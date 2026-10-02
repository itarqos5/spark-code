use crate::{
    model::*,
    provider::{self, Handle, ProviderCommand, ProviderEvent, RunConfig},
    store::{Store, data_dir},
};
use std::collections::{HashMap, VecDeque};

pub struct ActiveRun {
    pub handle: Handle,
    pub draft: Message,
    pub status: String,
    pub approval: VecDeque<(String, String)>,
    pub usage: String,
}
pub struct Engine {
    pub store: Store,
    pub settings: Settings,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub jobs: HashMap<String, ActiveRun>,
    pub notice: String,
    pub concurrency: usize,
}
impl Engine {
    pub fn open() -> Result<Self, String> {
        let store = Store::open(&data_dir().join("spark-code.db"))?;
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
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or("Conversation not found")?;
        let p = self
            .projects
            .iter()
            .find(|p| p.id == s.project_id)
            .ok_or("Project not found")?;
        for other in self.jobs.keys() {
            if self
                .sessions
                .iter()
                .find(|s| &s.id == other)
                .and_then(|s| self.projects.iter().find(|p| p.id == s.project_id))
                .is_some_and(|o| o.path == p.path)
            {
                return Err("An agent is already working in this folder. Use a separate project/worktree to prevent conflicting edits.".into());
            }
        }
        let s = self.sessions.iter_mut().find(|s| s.id == id).unwrap();
        let exe = match s.provider {
            Provider::Codex => &self.settings.codex_path,
            Provider::Claude => &self.settings.claude_path,
        };
        let config = RunConfig {
            provider: s.provider,
            executable: exe.clone(),
            cwd: p.path.clone(),
            model: s.model.clone(),
            remote_id: s.remote_id.clone(),
            prompt: prompt.clone(),
        };
        let handle = provider::start(config)?;
        if s.title == "New conversation" {
            s.title = prompt.chars().take(54).collect();
        }
        s.updated = now();
        self.store.save_session(s)?;
        self.store.save_message(&Message::new(id, "user", prompt))?;
        self.jobs.insert(
            id.into(),
            ActiveRun {
                handle,
                draft: Message::new(id, "assistant", String::new()),
                status: "Connecting".into(),
                approval: VecDeque::new(),
                usage: String::new(),
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
            if let Some((aid, _)) = j.approval.pop_front() {
                let _ = j
                    .handle
                    .commands
                    .try_send(ProviderCommand::Approve { id: aid, allow });
                j.status = if allow { "Running" } else { "Denied" }.into();
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
                                    let _ = self.store.save_session(s);
                                }
                                j.status = "Working".into();
                            }
                            ProviderEvent::Text(t) => {
                                let remain = MAX_MESSAGE_BYTES.saturating_sub(j.draft.text.len());
                                if remain > 0 {
                                    j.draft.text.extend(t.chars().scan(0, |n, c| {
                                        *n += c.len_utf8();
                                        if *n <= remain { Some(c) } else { None }
                                    }));
                                } else {
                                    j.handle.cancel();
                                    j.status = "Output limit reached".into();
                                }
                            }
                            ProviderEvent::Tool(t) => {
                                j.status = t.chars().take(240).collect();
                            }
                            ProviderEvent::Approval { id, description } => {
                                j.approval.push_back((id, description));
                                j.status = "Needs your approval".into();
                            }
                            ProviderEvent::Usage(t) => j.usage = t,
                            ProviderEvent::Models(_) => {}
                            ProviderEvent::Done => {
                                done.push(id.clone());
                                break;
                            }
                            ProviderEvent::Error(e) => {
                                self.notice = e.clone();
                                let _ = self.store.save_message(&Message::new(id, "notice", e));
                                done.push(id.clone());
                                break;
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done.push(id.clone());
                        break;
                    }
                }
            }
        }
        for id in done {
            if let Some(j) = self.jobs.remove(&id) {
                if !j.draft.text.is_empty() {
                    if let Err(e) = self.store.save_message(&j.draft) {
                        self.notice = e;
                    }
                }
                if !j.usage.is_empty() {
                    self.notice = j.usage;
                } else if self.notice.starts_with("Ready") {
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
