use crate::model::*;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
pub struct Store {
    db: Connection,
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("SPARK_CODE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("spark-code")
        })
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA cache_size=-1024; CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,name TEXT NOT NULL,path TEXT NOT NULL); CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,title TEXT NOT NULL,provider TEXT NOT NULL,model TEXT NOT NULL,remote_id TEXT,updated INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL REFERENCES sessions(id),role TEXT NOT NULL,text TEXT NOT NULL,created INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS message_session ON messages(session_id,created); CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL);").map_err(|e|e.to_string())?;
        Ok(Self { db })
    }
    pub fn projects(&self) -> Result<Vec<Project>, String> {
        let mut st = self
            .db
            .prepare("SELECT id,name,path FROM projects ORDER BY name")
            .map_err(|e| e.to_string())?;
        st.query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
    pub fn save_project(&self, p: &Project) -> Result<(), String> {
        self.db.execute("INSERT INTO projects VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,path=excluded.path",params![p.id,p.name,p.path]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn sessions(&self, query: &str) -> Result<Vec<Session>, String> {
        let mut st=self.db.prepare("SELECT DISTINCT s.id,s.project_id,s.title,s.provider,s.model,s.remote_id,s.updated FROM sessions s WHERE s.title LIKE ?1 ESCAPE '\\' OR EXISTS(SELECT 1 FROM messages m WHERE m.session_id=s.id AND m.text LIKE ?1 ESCAPE '\\') ORDER BY s.updated DESC LIMIT 200").map_err(|e|e.to_string())?;
        let q = format!(
            "%{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        st.query_map([q], |r| {
            Ok(Session {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                provider: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                model: r.get(4)?,
                remote_id: r.get(5)?,
                updated: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
    pub fn save_session(&self, s: &Session) -> Result<(), String> {
        self.db.execute("INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET title=excluded.title,provider=excluded.provider,model=excluded.model,remote_id=excluded.remote_id,updated=excluded.updated",params![s.id,s.project_id,s.title,serde_json::to_string(&s.provider).unwrap(),s.model,s.remote_id,s.updated]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn messages(&self, id: &str) -> Result<Vec<Message>, String> {
        self.messages_limit(id, VISIBLE_MESSAGES)
    }
    fn messages_limit(&self, id: &str, limit: usize) -> Result<Vec<Message>, String> {
        let mut st=self.db.prepare("SELECT id,session_id,role,text,created FROM (SELECT rowid,id,session_id,role,text,created FROM messages WHERE session_id=?1 ORDER BY rowid DESC LIMIT ?2) ORDER BY rowid").map_err(|e|e.to_string())?;
        st.query_map(params![id, limit as i64], |r| {
            Ok(Message {
                id: r.get(0)?,
                session_id: r.get(1)?,
                role: r.get(2)?,
                text: r.get(3)?,
                created: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
    pub fn save_message(&self, m: &Message) -> Result<(), String> {
        if m.text.len() > MAX_MESSAGE_BYTES {
            return Err("Message exceeds 512 KiB limit".into());
        }
        self.db.execute("INSERT INTO messages VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET text=excluded.text",params![m.id,m.session_id,m.role,m.text,m.created]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn settings(&self) -> Settings {
        self.db
            .query_row("SELECT json FROM settings WHERE id=1", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save_settings(&self, s: &Settings) -> Result<(), String> {
        self.db
            .execute(
                "INSERT OR REPLACE INTO settings VALUES(1,?1)",
                [serde_json::to_string(s).map_err(|e| e.to_string())?],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn backup(&self) -> Result<Backup, String> {
        let sessions = self.all_sessions()?;
        let mut messages = Vec::new();
        for s in &sessions {
            messages.extend(self.messages_limit(&s.id, usize::MAX / 2)?);
        }
        Ok(Backup {
            format: "spark-code".into(),
            version: 1,
            projects: self.projects()?,
            sessions,
            messages,
        })
    }
    fn all_sessions(&self) -> Result<Vec<Session>, String> {
        let mut st=self.db.prepare("SELECT id,project_id,title,provider,model,remote_id,updated FROM sessions ORDER BY updated DESC").map_err(|e|e.to_string())?;
        st.query_map([], |r| {
            Ok(Session {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                provider: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                model: r.get(4)?,
                remote_id: r.get(5)?,
                updated: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
    pub fn import(&mut self, b: &Backup, selected: &[String]) -> Result<usize, String> {
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        let mut n = 0;
        for p in &b.projects {
            tx.execute(
                "INSERT OR IGNORE INTO projects VALUES(?1,?2,?3)",
                params![p.id, p.name, p.path],
            )
            .map_err(|e| e.to_string())?;
        }
        for s in &b.sessions {
            if !selected.contains(&s.id) {
                continue;
            }
            n += tx
                .execute(
                    "INSERT OR IGNORE INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        s.id,
                        s.project_id,
                        s.title,
                        serde_json::to_string(&s.provider).unwrap(),
                        s.model,
                        s.remote_id,
                        s.updated
                    ],
                )
                .map_err(|e| e.to_string())?;
            for m in b.messages.iter().filter(|m| m.session_id == s.id) {
                if m.text.len() > MAX_MESSAGE_BYTES {
                    return Err("Imported message exceeds limit".into());
                }
                tx.execute(
                    "INSERT OR IGNORE INTO messages VALUES(?1,?2,?3,?4,?5)",
                    params![m.id, m.session_id, m.role, m.text, m.created],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(n)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_and_searches() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("data.db");
        let s = Store::open(&p).unwrap();
        let c = Session::new("p".into(), Provider::Codex, "".into());
        s.save_session(&c).unwrap();
        s.save_message(&Message::new(&c.id, "user", "find percent 100%".into()))
            .unwrap();
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(s.sessions("100%").unwrap().len(), 1);
        assert_eq!(s.sessions("absent").unwrap().len(), 0);
        assert_eq!(s.messages(&c.id).unwrap().len(), 1);
    }
    #[test]
    fn bounded_visible_history() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let c = Session::new("p".into(), Provider::Codex, "".into());
        s.save_session(&c).unwrap();
        for i in 0..130 {
            s.save_message(&Message::new(&c.id, "user", i.to_string()))
                .unwrap();
        }
        assert_eq!(s.messages(&c.id).unwrap().len(), 100);
        assert_eq!(s.backup().unwrap().messages.len(), 130);
    }
}
