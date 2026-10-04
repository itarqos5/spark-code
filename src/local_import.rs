//! Opt-in local history readers. SQLite sources stay open read-only, including live WAL data.
use crate::{model::*, store::Store};
use rusqlite::{Connection, OpenFlags, Row};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

const THREADS: usize = 200;
const MESSAGES: usize = 100;
const BUDGET: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Codex,
    T3,
}
impl SourceKind {
    pub const ALL: [Self; 2] = [Self::Codex, Self::T3];
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::T3 => "T3 Code",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::T3 => "t3",
        }
    }
    pub fn enabled(self, s: &Settings) -> bool {
        match self {
            Self::Codex => s.history_codex,
            Self::T3 => s.history_t3,
        }
    }
    pub fn folder(self, s: &Settings) -> &str {
        match self {
            Self::Codex => &s.codex_history_dir,
            Self::T3 => &s.t3_history_dir,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Source {
    pub kind: SourceKind,
    pub path: PathBuf,
}
impl Source {
    pub fn available(&self) -> bool {
        self.path.is_file()
    }
    pub fn key(&self) -> String {
        // Stable, non-secret namespace across launches; this is not an authentication hash.
        // Codex's state_N filename changes during schema migrations; the source stays the same.
        let path = if self.kind == SourceKind::Codex {
            self.path.parent().unwrap_or(&self.path)
        } else {
            &self.path
        }
        .to_string_lossy();
        let normalized = if cfg!(windows) {
            path.to_lowercase()
        } else {
            path.into_owned()
        };
        let hash = normalized.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        format!("{}:{hash:016x}", self.kind.key())
    }
    fn id(&self, entity: &str, id: &str) -> String {
        format!("linked:{}:{entity}:{id}", self.key())
    }
}

#[derive(Clone, Debug)]
pub struct SyncResult {
    pub source: Source,
    pub result: Result<(usize, usize, usize), String>, // new conversations, window conversations, messages
}

pub fn discover(s: &Settings) -> Vec<Source> {
    let home = dirs::home_dir().unwrap_or_default();
    let codex_root = if s.codex_history_dir.is_empty() {
        std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"))
    } else {
        PathBuf::from(&s.codex_history_dir)
    };
    let codex = std::fs::read_dir(&codex_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let version = name
                .strip_prefix("state_")?
                .strip_suffix(".sqlite")?
                .parse::<u32>()
                .ok()?;
            Some((version, e.path()))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, p)| p)
        .unwrap_or_else(|| codex_root.join("state_5.sqlite"));
    let t3_root = if s.t3_history_dir.is_empty() {
        std::env::var_os("T3CODE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".t3"))
    } else {
        PathBuf::from(&s.t3_history_dir)
    };
    let t3 = [
        t3_root.join("userdata/state.sqlite"),
        t3_root.join("state.sqlite"),
    ]
    .into_iter()
    .find(|p| p.is_file())
    .unwrap_or_else(|| t3_root.join("userdata/state.sqlite"));
    vec![
        Source {
            kind: SourceKind::Codex,
            path: codex,
        },
        Source {
            kind: SourceKind::T3,
            path: t3,
        },
    ]
}

pub fn sync(sources: Vec<Source>, settings: &Settings, destination: &Path) -> Vec<SyncResult> {
    sources
        .into_iter()
        .filter(|s| s.kind.enabled(settings) && s.available())
        .map(|source| {
            let result = read(&source).and_then(|b| {
                let mut store = Store::open(destination)?;
                let added = store.sync_history(&source.key(), &b)?;
                Ok((added, b.sessions.len(), b.messages.len()))
            });
            SyncResult { source, result }
        })
        .collect()
}

pub fn read(source: &Source) -> Result<Backup, String> {
    let b = match source.kind {
        SourceKind::T3 => t3(source)?,
        SourceKind::Codex => codex(source)?,
    };
    crate::import::validate(&b)?;
    Ok(b)
}
fn empty() -> Backup {
    Backup {
        format: "spark-code".into(),
        version: 1,
        projects: vec![],
        sessions: vec![],
        messages: vec![],
    }
}
fn open(path: &Path) -> Result<Connection, String> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    // A deferred read transaction pins a consistent snapshot without closing the source app.
    db.execute_batch(
        "PRAGMA query_only=ON; PRAGMA trusted_schema=OFF; PRAGMA cache_size=-1024; BEGIN DEFERRED;",
    )
    .map_err(|e| e.to_string())?;
    Ok(db)
}
fn columns(db: &Connection, table: &str, required: &[&str]) -> Result<Vec<String>, String> {
    let kind: String = db
        .query_row(
            "SELECT type FROM sqlite_master WHERE name=?1",
            [table],
            |r| r.get(0),
        )
        .map_err(|_| format!("Unsupported history schema: missing {table}"))?;
    if kind != "table" {
        return Err("Unsupported history schema".into());
    }
    let mut st = db
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let names = st
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if !required.iter().all(|c| names.iter().any(|n| n == c)) {
        return Err(format!("Unsupported columns in {table}"));
    }
    Ok(names)
}
fn has(cols: &[String], col: &str) -> bool {
    cols.iter().any(|c| c == col)
}
fn text(row: &Row<'_>, index: usize, budget: &mut usize) -> Result<String, String> {
    let v = row.get_ref(index).map_err(|e| e.to_string())?;
    let s = if v == rusqlite::types::ValueRef::Null {
        ""
    } else {
        v.as_str().map_err(|_| "Unsupported history text column")?
    };
    if s.len() > 2 * 1024 * 1024 {
        return Err("History field exceeds 2 MiB".into());
    }
    *budget = budget
        .checked_add(s.len())
        .filter(|n| *n <= BUDGET)
        .ok_or("History window exceeds 64 MiB")?;
    Ok(s.to_owned())
}
fn timestamp(cols: &[String], col: &str) -> String {
    if has(cols, col) {
        format!("COALESCE(CAST(strftime('%s',{col}) AS INTEGER),0)")
    } else {
        "0".into()
    }
}

fn t3(source: &Source) -> Result<Backup, String> {
    let db = open(&source.path)?;
    columns(
        &db,
        "projection_projects",
        &["project_id", "title", "workspace_root"],
    )?;
    let tc = columns(
        &db,
        "projection_threads",
        &["thread_id", "project_id", "title", "model_selection_json"],
    )?;
    let mc = columns(
        &db,
        "projection_thread_messages",
        &["message_id", "thread_id", "role", "text"],
    )?;
    let mut b = empty();
    let mut budget = 0;
    let deleted = if has(&tc, "deleted_at") {
        "deleted_at IS NULL"
    } else {
        "1"
    };
    let updated = timestamp(&tc, "updated_at");
    let mut st = db.prepare(&format!("SELECT thread_id,project_id,title,model_selection_json,{updated} AS stamp FROM projection_threads WHERE {deleted} ORDER BY stamp DESC,rowid DESC LIMIT {THREADS}")).map_err(|e| e.to_string())?;
    let mut rows = st.query([]).map_err(|e| e.to_string())?;
    let mut projects = HashMap::new();
    while let Some(r) = rows.next().map_err(|e| e.to_string())? {
        let original = text(r, 0, &mut budget)?;
        let project = text(r, 1, &mut budget)?;
        let title = text(r, 2, &mut budget)?;
        let model: Value = serde_json::from_str(&text(r, 3, &mut budget)?)
            .map_err(|_| "Unsupported T3 model selection")?;
        let provider = if ["provider", "instanceId", "model"].iter().any(|key| {
            model[key]
                .as_str()
                .is_some_and(|s| s.to_lowercase().contains("claude"))
        }) {
            Provider::Claude
        } else {
            Provider::Codex
        };
        if !projects.contains_key(&project) {
            let p = db
                .query_row(
                    "SELECT title,workspace_root FROM projection_projects WHERE project_id=?1",
                    [&project],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .map_err(|e| e.to_string())?;
            b.projects.push(Project {
                id: source.id("p", &project),
                name: p.0,
                path: p.1,
            });
            projects.insert(project.clone(), ());
        }
        let id = source.id("s", &original);
        b.sessions.push(Session {
            id: id.clone(),
            project_id: source.id("p", &project),
            title,
            provider,
            model: model["model"].as_str().unwrap_or_default().into(),
            remote_id: None,
            updated: r.get(4).map_err(|e| e.to_string())?,
        });
        let created = timestamp(&mc, "created_at");
        let done = if has(&mc, "is_streaming") {
            "AND is_streaming=0"
        } else {
            ""
        };
        let mut messages = db.prepare(&format!("SELECT message_id,role,text,stamp FROM (SELECT rowid,message_id,role,text,{created} AS stamp FROM projection_thread_messages WHERE thread_id=?1 AND role IN ('user','assistant') {done} ORDER BY rowid DESC LIMIT {MESSAGES}) ORDER BY rowid")).map_err(|e|e.to_string())?;
        let mut rs = messages.query([&original]).map_err(|e| e.to_string())?;
        while let Some(r) = rs.next().map_err(|e| e.to_string())? {
            let mid = text(r, 0, &mut budget)?;
            b.messages.push(Message {
                id: source.id("m", &format!("{original}:{mid}")),
                session_id: id.clone(),
                role: text(r, 1, &mut budget)?,
                text: text(r, 2, &mut budget)?,
                created: r.get(3).map_err(|e| e.to_string())?,
            });
        }
    }
    Ok(b)
}

fn codex(source: &Source) -> Result<Backup, String> {
    let db = open(&source.path)?;
    let tc = columns(
        &db,
        "threads",
        &["id", "cwd", "title", "updated_at", "rollout_path"],
    )?;
    let root = source.path.parent().ok_or("Invalid Codex history folder")?;
    let history_path = root.join("thread_history_1.sqlite");
    let history = if history_path.is_file() {
        Some(open(&history_path)?)
    } else {
        None
    };
    if let Some(h) = &history {
        columns(
            h,
            "thread_items",
            &[
                "thread_id",
                "item_id",
                "item_type",
                "item_json",
                "created_at_ms",
                "rollout_ordinal",
            ],
        )?;
    }
    let model = if has(&tc, "model") {
        "COALESCE(model,'')"
    } else {
        "''"
    };
    let mut visible = if has(&tc, "archived") {
        "archived=0"
    } else {
        "1"
    }
    .to_owned();
    if has(&tc, "source") {
        visible.push_str(" AND instr(COALESCE(source,''),'subagent')=0");
    }
    let mut st = db.prepare(&format!("SELECT id,cwd,title,{model},updated_at,rollout_path FROM threads WHERE {visible} ORDER BY updated_at DESC LIMIT {THREADS}")).map_err(|e|e.to_string())?;
    let mut rows = st.query([]).map_err(|e| e.to_string())?;
    let mut b = empty();
    let mut budget = 0;
    let mut projects = HashMap::new();
    while let Some(r) = rows.next().map_err(|e| e.to_string())? {
        let original = text(r, 0, &mut budget)?;
        let cwd = text(r, 1, &mut budget)?;
        let project_id = source.id("p", &cwd);
        if !projects.contains_key(&cwd) {
            b.projects.push(Project {
                id: project_id.clone(),
                name: Path::new(&cwd)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Codex conversations".into()),
                path: cwd.clone(),
            });
            projects.insert(cwd, ());
        }
        let id = source.id("s", &original);
        b.sessions.push(Session {
            id: id.clone(),
            project_id,
            title: text(r, 2, &mut budget)?,
            provider: Provider::Codex,
            model: text(r, 3, &mut budget)?,
            remote_id: None,
            updated: r.get(4).map_err(|e| e.to_string())?,
        });
        let before = b.messages.len();
        if let Some(h) = &history {
            let mut st = h.prepare(&format!("SELECT item_id,item_json,created_at_ms FROM (SELECT item_id,item_json,created_at_ms,rollout_ordinal FROM thread_items WHERE thread_id=?1 AND item_type IN ('userMessage','agentMessage') ORDER BY rollout_ordinal DESC LIMIT {MESSAGES}) ORDER BY rollout_ordinal")).map_err(|e|e.to_string())?;
            let mut rs = st.query([&original]).map_err(|e| e.to_string())?;
            while let Some(r) = rs.next().map_err(|e| e.to_string())? {
                let mid = text(r, 0, &mut budget)?;
                let item: Value = serde_json::from_str(&text(r, 1, &mut budget)?)
                    .map_err(|_| "Unsupported Codex history item")?;
                if let Some((role, text)) = codex_item(&item) {
                    b.messages.push(Message {
                        id: source.id("m", &format!("{original}:{mid}")),
                        session_id: id.clone(),
                        role: role.into(),
                        text,
                        created: r.get::<_, i64>(2).map_err(|e| e.to_string())? / 1000,
                    });
                }
            }
        }
        // Older CLI installations persist transcripts in rollout JSONL instead of the history DB.
        if b.messages.len() == before {
            let path = PathBuf::from(text(r, 5, &mut budget)?);
            if path.is_file() {
                let canonical = path.canonicalize().map_err(|e| e.to_string())?;
                if canonical.starts_with(root.canonicalize().map_err(|e| e.to_string())?) {
                    b.messages.extend(rollout(
                        &db,
                        source,
                        &original,
                        &id,
                        &canonical,
                        &mut budget,
                    )?);
                }
            }
        }
    }
    Ok(b)
}
fn codex_item(item: &Value) -> Option<(&'static str, String)> {
    match item["type"].as_str()? {
        "userMessage" => Some((
            "user",
            item["content"]
                .as_array()?
                .iter()
                .filter_map(|v| v["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )),
        "agentMessage" => Some(("assistant", item["text"].as_str()?.into())),
        _ => None,
    }
}
fn rollout(
    db: &Connection,
    source: &Source,
    original: &str,
    id: &str,
    path: &Path,
    budget: &mut usize,
) -> Result<Vec<Message>, String> {
    use std::io::{BufRead, BufReader, Seek, SeekFrom};
    // Do not slurp potentially huge tool logs. Lines and total scan size are bounded.
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    let mut offset = length.saturating_sub(BUDGET as u64);
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file.take(BUDGET as u64));
    if offset > 0 {
        let mut partial = vec![];
        offset += reader
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut partial)
            .map_err(|e| e.to_string())? as u64;
        if partial.len() > 2 * 1024 * 1024 {
            return Err("Codex rollout line exceeds 2 MiB".into());
        }
    }
    let mut messages = std::collections::VecDeque::new();
    loop {
        let mut bytes = vec![];
        let n = reader
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if n > 2 * 1024 * 1024 {
            return Err("Codex rollout line exceeds 2 MiB".into());
        }
        let item_offset = offset;
        offset += n as u64;
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let p = &v["payload"];
        if v["type"] != "response_item" || p["type"] != "message" {
            continue;
        }
        let Some(role @ ("user" | "assistant")) = p["role"].as_str() else {
            continue;
        };
        let text = p["content"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|v| v["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        *budget = budget
            .checked_add(text.len())
            .filter(|n| *n <= BUDGET)
            .ok_or("History window exceeds 64 MiB")?;
        messages.push_back(Message {
            id: source.id("m", &format!("{original}:rollout:{item_offset}")),
            session_id: id.into(),
            role: role.into(),
            text,
            created: db
                .query_row(
                    "SELECT COALESCE(unixepoch(?1),0)",
                    [v["timestamp"].as_str().unwrap_or("")],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?,
        });
        if messages.len() > MESSAGES {
            messages.pop_front();
        }
    }
    Ok(messages.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    #[test]
    fn older_codex_rollouts_keep_recent_messages_and_stable_ids() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state_5.sqlite");
        let rollout = dir.path().join("rollout.jsonl");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT,cwd TEXT,title TEXT,updated_at INTEGER,rollout_path TEXT);").unwrap();
        db.execute(
            "INSERT INTO threads VALUES('s','/project','Chat',42,?1)",
            [rollout.to_string_lossy().as_ref()],
        )
        .unwrap();
        let mut file = std::fs::File::create(&rollout).unwrap();
        for i in 0..101 {
            writeln!(file,"{}",serde_json::json!({"timestamp":"1970-01-01T00:00:42Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":format!("message {i}")}]}})).unwrap();
        }
        drop(file);
        let source = Source {
            kind: SourceKind::Codex,
            path,
        };
        let first = read(&source).unwrap();
        let second = read(&source).unwrap();
        assert_eq!(first.messages.len(), 100);
        assert_eq!(first.messages[0].text, "message 1");
        assert_eq!(first.messages[0].created, 42);
        assert_eq!(first.messages[0].id, second.messages[0].id);
        let migrated = Source {
            kind: SourceKind::Codex,
            path: dir.path().join("state_6.sqlite"),
        };
        assert_eq!(source.key(), migrated.key());
    }
    #[test]
    fn live_t3_wal_sync_is_repeatable_and_preserves_local_replies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.sqlite");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE projection_projects(project_id TEXT,title TEXT,workspace_root TEXT); CREATE TABLE projection_threads(thread_id TEXT,project_id TEXT,title TEXT,model_selection_json TEXT); CREATE TABLE projection_thread_messages(message_id TEXT,thread_id TEXT,role TEXT,text TEXT,is_streaming INTEGER); INSERT INTO projection_projects VALUES('p','Project','/project'); INSERT INTO projection_threads VALUES('s','p','Chat','{}'); INSERT INTO projection_thread_messages VALUES('m','s','assistant','first',0); INSERT INTO projection_thread_messages VALUES('draft','s','assistant','partial',1);").unwrap();
        let source = Source {
            kind: SourceKind::T3,
            path,
        };
        let mut target = Store::open(&dir.path().join("spark.db")).unwrap();
        let b = read(&source).unwrap();
        assert_eq!(b.messages.len(), 1);
        assert_eq!(target.sync_history(&source.key(), &b).unwrap(), 1);
        target
            .save_message(&Message::new(
                &b.sessions[0].id,
                "user",
                "local reply".into(),
            ))
            .unwrap();
        db.execute(
            "UPDATE projection_thread_messages SET text='updated' WHERE message_id='m'",
            [],
        )
        .unwrap();
        assert_eq!(
            target
                .sync_history(&source.key(), &read(&source).unwrap())
                .unwrap(),
            0
        );
        let messages = target.messages(&b.sessions[0].id).unwrap();
        assert!(messages.iter().any(|m| m.text == "updated"));
        assert!(messages.iter().any(|m| m.text == "local reply"));
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM projection_thread_messages", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }
    #[test]
    fn codex_database_reads_only_visible_message_items() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state_5.sqlite");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE threads(id TEXT,cwd TEXT,title TEXT,updated_at INTEGER,rollout_path TEXT); INSERT INTO threads VALUES('s','/project','Chat',42,'');").unwrap();
        let history = Connection::open(dir.path().join("thread_history_1.sqlite")).unwrap();
        history.execute_batch("CREATE TABLE thread_items(thread_id TEXT,item_id TEXT,item_type TEXT,item_json TEXT,created_at_ms INTEGER,rollout_ordinal INTEGER);").unwrap();
        for (id, kind, json) in [
            (
                "u",
                "userMessage",
                r#"{"type":"userMessage","content":[{"type":"text","text":"question"}]}"#,
            ),
            (
                "a",
                "agentMessage",
                r#"{"type":"agentMessage","text":"answer"}"#,
            ),
            ("r", "reasoning", r#"{"type":"reasoning","text":"hidden"}"#),
        ] {
            history
                .execute(
                    "INSERT INTO thread_items VALUES('s',?1,?2,?3,42000,?4)",
                    params![id, kind, json, if id == "u" { 1 } else { 2 }],
                )
                .unwrap();
        }
        let b = read(&Source {
            kind: SourceKind::Codex,
            path,
        })
        .unwrap();
        assert_eq!(b.messages.len(), 2);
        assert_eq!(b.messages[0].created, 42);
    }
}
