use crate::{
    model::*,
    provider::{self, Handle, ProviderCommand, ProviderEvent, RunConfig},
    store::{Store, data_dir},
};
use std::{
    collections::{HashMap, VecDeque, hash_map::DefaultHasher},
    fs::{File, OpenOptions},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::mpsc::TrySendError,
};
const MAX_APPROVALS: usize = 32;

pub struct ActiveRun {
    pub handle: Handle,
    pub draft: Message,
    pub status: String,
    pub approval: VecDeque<(String, String)>,
    pub usage: String,
    failed: bool,
    canonical_path: PathBuf,
    // Fields drop in declaration order: provider Handle cleanup must finish first.
    _project_lock: File,
}
pub struct Engine {
    pub store: Store,
    pub settings: Settings,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub jobs: HashMap<String, ActiveRun>,
    pub notice: String,
    pub concurrency: usize,
    pub models: Vec<String>,
    lock_dir: PathBuf,
}
impl Engine {
    pub fn open() -> Result<Self, String> {
        Self::open_at(&data_dir().join("spark-code.db"))
    }
    fn open_at(path: &Path) -> Result<Self, String> {
        let store = Store::open(path)?;
        let settings = store.settings();
        let projects = store.projects()?;
        let sessions = store.sessions("")?;
        Ok(Self {
            store,
            settings,
            projects,
            sessions,
            jobs: HashMap::new(),
            notice: "Ready · providers start only when you send".into(),
            concurrency: 2,
            models: Vec::new(),
            lock_dir: path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .join("locks"),
        })
    }
    pub fn refresh(&mut self, query: &str) -> Result<(), String> {
        self.projects = self.store.projects()?;
        self.sessions = self.store.sessions(query)?;
        Ok(())
    }
    pub fn add_project(&mut self, path: String) -> Result<String, String> {
        let p = std::path::Path::new(&path);
        if !p.is_dir() {
            return Err("Choose an existing project folder".into());
        }
        let path = p
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        if let Some(p) = self.projects.iter().find(|p| p.path == path) {
            return Ok(p.id.clone());
        }
        let p = Project {
            id: new_id(),
            name: std::path::Path::new(&path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path,
        };
        self.store.save_project(&p)?;
        let id = p.id.clone();
        self.projects.push(p);
        Ok(id)
    }
    pub fn new_session(&mut self, pid: String) -> Result<String, String> {
        if !self.projects.iter().any(|p| p.id == pid) {
            return Err("Add or select a project folder first".into());
        }
        let s = Session::new(pid, self.settings.provider, self.settings.model.clone());
        self.store.save_session(&s)?;
        let id = s.id.clone();
        self.sessions.insert(0, s);
        Ok(id)
    }
    pub fn send(&mut self, id: &str, prompt: String) -> Result<(), String> {
        if prompt.trim().is_empty() {
            return Ok(());
        }
        if prompt.len() > 64 * 1024 {
            return Err("Prompt is too long (64 KiB maximum)".into());
        }
        if self.jobs.contains_key(id) {
            return Err("This conversation is already running".into());
        }
        if self.jobs.len() >= self.concurrency {
            return Err(format!(
                "All {} agent slots are busy; wait or stop one",
                self.concurrency
            ));
        }
        let s = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or("Conversation not found")?;
        let p = self
            .projects
            .iter()
            .find(|p| p.id == s.project_id)
            .ok_or("Project not found")?;
        let canonical = canonical_project(&p.path)?;
        if self
            .jobs
            .values()
            .any(|run| run.canonical_path == canonical)
        {
            return Err("An agent is already working in this folder. Use a separate project/worktree to prevent conflicting edits.".into());
        }
        let mut s = self.sessions.iter().find(|s| s.id == id).unwrap().clone();
        let exe = match s.provider {
            Provider::Codex => &self.settings.codex_path,
            Provider::Claude => &self.settings.claude_path,
        };
        let config = RunConfig {
            provider: s.provider,
            executable: exe.clone(),
            cwd: canonical.to_string_lossy().into_owned(),
            model: s.model.clone(),
            remote_id: s.remote_id.clone(),
            prompt: prompt.clone(),
        };
        if s.title == "New conversation" {
            s.title = prompt.chars().take(54).collect();
        }
        s.updated = now();
        let project_lock = lock_project(&self.lock_dir, &canonical)?;
        self.store
            .save_turn(&s, &Message::new(id, "user", prompt))?;
        *self
            .sessions
            .iter_mut()
            .find(|local| local.id == id)
            .unwrap() = s;
        let handle = provider::start(config)?;
        self.jobs.insert(
            id.into(),
            ActiveRun {
                handle,
                draft: Message::new(id, "assistant", String::new()),
                status: "Connecting".into(),
                approval: VecDeque::new(),
                usage: String::new(),
                failed: false,
                canonical_path: canonical,
                _project_lock: project_lock,
            },
        );
        Ok(())
    }
    pub fn cancel(&mut self, id: &str) {
        if let Some(j) = self.jobs.get_mut(id) {
            j.handle.cancel();
            j.status = "Stopping…".into();
        }
    }
    pub fn approve(&mut self, id: &str, allow: bool) {
        if let Some(j) = self.jobs.get_mut(id) {
            if let Some((aid, _)) = j.approval.front() {
                match j.handle.commands.try_send(ProviderCommand::Approve {
                    id: aid.clone(),
                    allow,
                }) {
                    Ok(()) => {
                        j.approval.pop_front();
                        j.status = if !j.approval.is_empty() {
                            "Needs your approval"
                        } else if allow {
                            "Running"
                        } else {
                            "Denied"
                        }
                        .into();
                    }
                    Err(TrySendError::Full(_)) => {
                        self.notice = "Approval queue is busy; please try again".into()
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        j.handle.cancel();
                        self.notice = "Provider disconnected before receiving your approval".into();
                    }
                }
            }
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let mut done = Vec::new();
        for (id, j) in &mut self.jobs {
            for _ in 0..128 {
                match j.handle.events.try_recv() {
                    Ok(ev) => {
                        changed = true;
                        match ev {
                            ProviderEvent::Started { remote_id } => {
                                if let Some(s) = self.sessions.iter_mut().find(|s| &s.id == id) {
                                    s.remote_id = Some(remote_id);
                                    if let Err(error) = self.store.save_session(s) {
                                        self.notice =
                                            format!("Could not save provider session: {error}");
                                        j.failed = true;
                                        j.handle.cancel();
                                    }
                                }
                                j.status = "Working".into();
                            }
                            ProviderEvent::Text(t) => {
                                let remain = MAX_MESSAGE_BYTES.saturating_sub(j.draft.text.len());
                                let oversized = t.len() > remain;
                                if remain > 0 {
                                    j.draft.text.extend(t.chars().scan(0, |n, c| {
                                        *n += c.len_utf8();
                                        if *n <= remain { Some(c) } else { None }
                                    }));
                                }
                                if oversized {
                                    j.handle.cancel();
                                    j.status = "Output limit reached".into();
                                    j.failed = true;
                                    self.notice = "Output limit reached; stopping with a bounded partial transcript".into();
                                }
                            }
                            ProviderEvent::Tool(t) => {
                                j.status = t.chars().take(240).collect();
                            }
                            ProviderEvent::Approval { id, description } => {
                                if let Err(error) = queue_approval(&mut j.approval, id, description)
                                {
                                    self.notice = error;
                                    j.failed = true;
                                    j.handle.cancel();
                                    j.status = "Stopping: approval limit reached".into();
                                } else {
                                    j.status = "Needs your approval".into();
                                }
                            }
                            ProviderEvent::Usage(t) => j.usage = t,
                            ProviderEvent::Models(models) => self.models = models,
                            ProviderEvent::Done => {
                                done.push(id.clone());
                                break;
                            }
                            ProviderEvent::Error(e) => {
                                j.failed = true;
                                self.notice = e.clone();
                                if let Err(error) =
                                    self.store.save_message(&Message::new(id, "notice", e))
                                {
                                    self.notice
                                        .push_str(&format!(" · Could not save error: {error}"));
                                }
                                done.push(id.clone());
                                break;
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        changed = true;
                        done.push(id.clone());
                        break;
                    }
                }
            }
        }
        for id in done {
            if let Some(j) = self.jobs.remove(&id) {
                let mut saved = true;
                if !j.draft.text.is_empty() {
                    if let Err(e) = self.store.save_message(&j.draft) {
                        self.notice = format!("Could not save transcript: {e}");
                        saved = false;
                    }
                }
                if saved
                    && !j.failed
                    && !j.usage.is_empty()
                    && !self.notice.contains("Could not save")
                {
                    self.notice = j.usage;
                } else if saved && !j.failed && self.notice.starts_with("Ready") {
                    self.notice = "Turn finished · transcript saved locally".into();
                }
            }
        }
        changed
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        for j in self.jobs.values() {
            j.handle.cancel();
        }
    }
}

// Lock files are retained after release. Removing them can let another process
// lock a new inode while an older inode is still held by an active worker.
fn lock_project(lock_dir: &Path, canonical: &Path) -> Result<File, String> {
    std::fs::create_dir_all(lock_dir)
        .map_err(|e| format!("Cannot create project lock directory: {e}"))?;
    let mut key = DefaultHasher::new();
    canonical.hash(&mut key);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_dir.join(format!("{:016x}.lock", key.finish())))
        .map_err(|e| format!("Cannot open project lock: {e}"))?;
    file.try_lock().map_err(|e| {
        format!(
            "This folder is locked by another Spark Code window, or its lock is unavailable: {e}"
        )
    })?;
    Ok(file)
}

fn canonical_project(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path)
        .canonicalize()
        .map_err(|_| "Project folder is unavailable; select an existing folder")?;
    if !path.is_dir() {
        return Err("Project path is not a folder".into());
    }
    Ok(path)
}
fn queue_approval(
    queue: &mut VecDeque<(String, String)>,
    id: String,
    description: String,
) -> Result<(), String> {
    if queue.iter().any(|(existing, _)| existing == &id) {
        return Ok(());
    }
    if queue.len() >= MAX_APPROVALS {
        return Err("Too many pending approvals; provider stopped safely".into());
    }
    queue.push_back((id, description));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Engine) {
        let dir = tempfile::tempdir().unwrap();
        let mut e = Engine::open_at(&dir.path().join("test.db")).unwrap();
        e.settings.codex_path = dir
            .path()
            .join("not-installed-test-provider")
            .to_string_lossy()
            .into_owned();
        (dir, e)
    }
    fn project_session(e: &mut Engine, path: &Path) -> String {
        let p = e.add_project(path.to_string_lossy().into_owned()).unwrap();
        e.new_session(p).unwrap()
    }
    #[test]
    fn default_two_slots_and_same_canonical_folder_is_exclusive() {
        let (dir, mut e) = fixture();
        assert_eq!(e.concurrency, 2);
        let a = project_session(&mut e, dir.path());
        let alias = Project {
            id: "alias".into(),
            name: "Alias".into(),
            path: dir.path().join(".").to_string_lossy().into_owned(),
        };
        e.store.save_project(&alias).unwrap();
        e.projects.push(alias);
        let b = e.new_session("alias".into()).unwrap();
        e.send(&a, "first".into()).unwrap();
        e.refresh("New conversation").unwrap();
        assert!(
            e.send(&b, "collision".into())
                .unwrap_err()
                .contains("already working")
        );
        let second = dir.path().join("second");
        std::fs::create_dir(&second).unwrap();
        let c = project_session(&mut e, &second);
        e.send(&c, "second".into()).unwrap();
        assert_eq!(e.jobs.len(), 2);
        assert!(e.send(&b, "third".into()).unwrap_err().contains("slots"));
        assert_eq!(e.store.messages(&b).unwrap().len(), 0);
    }
    #[test]
    fn approvals_are_deduplicated_and_bounded() {
        let mut q = VecDeque::new();
        for n in 0..MAX_APPROVALS {
            queue_approval(&mut q, n.to_string(), "review".into()).unwrap();
        }
        queue_approval(&mut q, "0".into(), "duplicate".into()).unwrap();
        assert_eq!(q.len(), MAX_APPROVALS);
        assert!(queue_approval(&mut q, "overflow".into(), "review".into()).is_err());
        assert_eq!(q.front().unwrap().1, "review");
    }
    #[test]
    fn failed_turn_save_never_starts_provider_or_changes_local_title() {
        let (dir, mut e) = fixture();
        let id = project_session(&mut e, dir.path());
        let db = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
        db.execute_batch("CREATE TRIGGER fail_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL,'test disk failure'); END;").unwrap();
        assert!(e.send(&id, "do not execute".into()).is_err());
        assert!(e.jobs.is_empty());
        assert_eq!(e.sessions[0].title, "New conversation");
        assert_eq!(e.store.sessions("").unwrap()[0].title, "New conversation");
        assert!(e.store.messages(&id).unwrap().is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn cancelling_one_run_leaves_the_other_running() {
        use std::{
            os::unix::fs::PermissionsExt,
            time::{Duration, Instant},
        };
        let (dir, mut e) = fixture();
        let executable = dir.path().join("mock-provider");
        std::fs::write(
            &executable,
            "#!/usr/bin/python3\nimport sys\nfor line in sys.stdin: pass\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        e.settings.codex_path = executable.to_string_lossy().into_owned();
        let a = project_session(&mut e, dir.path());
        let second = dir.path().join("second");
        std::fs::create_dir(&second).unwrap();
        let b = project_session(&mut e, &second);
        e.send(&a, "first".into()).unwrap();
        e.send(&b, "second".into()).unwrap();
        e.cancel(&a);
        let deadline = Instant::now() + Duration::from_secs(6);
        while e.jobs.contains_key(&a) && Instant::now() < deadline {
            e.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!e.jobs.contains_key(&a));
        assert!(e.jobs.contains_key(&b));
        assert_ne!(e.jobs[&b].status, "Stopping…");
        e.cancel(&b);
        let deadline = Instant::now() + Duration::from_secs(6);
        while !e.jobs.is_empty() && Instant::now() < deadline {
            e.poll();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(e.jobs.is_empty());
    }
    #[test]
    fn final_transcript_save_failure_is_not_hidden_by_usage() {
        let (dir, mut e) = fixture();
        let id = project_session(&mut e, dir.path());
        e.send(&id, "hello".into()).unwrap();
        e.jobs.get_mut(&id).unwrap().draft.text = "partial result".into();
        e.jobs.get_mut(&id).unwrap().usage = "10 tokens".into();
        let db = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
        db.execute_batch("CREATE TRIGGER fail_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL,'test disk failure'); END;").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !e.jobs.is_empty() && std::time::Instant::now() < deadline {
            e.poll();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(e.jobs.is_empty());
        assert!(
            e.notice.contains("Could not save transcript"),
            "{}",
            e.notice
        );
        assert_eq!(e.store.messages(&id).unwrap().len(), 1);
    }
    #[test]
    fn separate_frontends_lock_same_folder_but_allow_other_folders_and_release() {
        let (dir, mut first) = fixture();
        let a = project_session(&mut first, dir.path());
        let mut second = Engine::open_at(&dir.path().join("test.db")).unwrap();
        second.settings.codex_path = first.settings.codex_path.clone();
        let b = project_session(&mut second, &dir.path().join("."));
        first.send(&a, "first window".into()).unwrap();
        assert!(
            second
                .send(&b, "blocked".into())
                .unwrap_err()
                .contains("locked")
        );
        assert!(second.store.messages(&b).unwrap().is_empty());
        let other = dir.path().join("other");
        std::fs::create_dir(&other).unwrap();
        let c = project_session(&mut second, &other);
        second.send(&c, "independent folder".into()).unwrap();
        assert_eq!(second.jobs.len(), 1);
        drop(first.jobs.remove(&a));
        second.send(&b, "lock released".into()).unwrap();
        assert_eq!(second.jobs.len(), 2);
    }
    #[test]
    fn failed_turn_save_releases_cross_frontend_folder_lock() {
        let (dir, mut first) = fixture();
        let a = project_session(&mut first, dir.path());
        let mut second = Engine::open_at(&dir.path().join("test.db")).unwrap();
        second.settings.codex_path = first.settings.codex_path.clone();
        let b = project_session(&mut second, dir.path());
        let db = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
        db.execute_batch("CREATE TRIGGER fail_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL,'test failure'); END;").unwrap();
        assert!(first.send(&a, "fail before execution".into()).is_err());
        db.execute_batch("DROP TRIGGER fail_message").unwrap();
        second.send(&b, "lock was released".into()).unwrap();
    }
}
