//! Explicit user-selected imports only. Nothing in this module discovers account files.
use crate::model::*;
use rusqlite::{Connection, OpenFlags};
use std::{fs::File, io::Read, path::Path};
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_ROWS: usize = 20_000;

pub fn preview(path: &Path) -> Result<Backup, String> {
    let meta = path.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return Err("Choose a regular backup file under 64 MiB".into());
    }
    let mut header = [0; 16];
    let n = File::open(path)
        .map_err(|e| e.to_string())?
        .read(&mut header)
        .map_err(|e| e.to_string())?;
    let b = if n == 16 && &header == b"SQLite format 3\0" {
        t3_sqlite(path)?
    } else if header.starts_with(b"PK\x03\x04") {
        zip_backup(path)?
    } else {
        let s = std::fs::read(path).map_err(|e| e.to_string())?;
        json_backup(&s)?
    };
    validate(&b)?;
    Ok(b)
}
fn json_backup(bytes: &[u8]) -> Result<Backup, String> {
    let b:Backup=serde_json::from_slice(bytes).map_err(|_|"Not a supported spark-code backup. Choose a spark-code JSON/ZIP or a consistent T3 Code SQLite backup.".to_owned())?;
    if b.format != "spark-code" || b.version != 1 {
        return Err("Unsupported backup format/version; source left untouched".into());
    }
    Ok(b)
}
fn zip_backup(path: &Path) -> Result<Backup, String> {
    let mut z = zip::ZipArchive::new(File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if z.len() > 100 {
        return Err("Archive has too many entries".into());
    }
    let mut total = 0;
    let mut json = None;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.enclosed_name().is_none() || f.name().contains('\\') || f.name().contains(':') {
            return Err("Unsafe archive path".into());
        }
        if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("Archive symlinks are not supported".into());
        }
        total += f.size();
        if total > MAX_FILE || f.size() > MAX_FILE {
            return Err("Expanded archive exceeds 64 MiB".into());
        }
        if f.name() == "spark-code.json" {
            let mut v = Vec::new();
            f.by_ref()
                .take(MAX_FILE + 1)
                .read_to_end(&mut v)
                .map_err(|e| e.to_string())?;
            if v.len() as u64 > MAX_FILE {
                return Err("Archive entry exceeds limit".into());
            }
            json = Some(v);
        }
    }
    json_backup(&json.ok_or("ZIP must contain spark-code.json at its root")?)
}
fn validate(b: &Backup) -> Result<(), String> {
    if b.sessions.len() > MAX_ROWS || b.messages.len() > MAX_ROWS || b.projects.len() > 1000 {
        return Err("Backup has too many records; split it into smaller backups".into());
    }
    let mut bytes = 0;
    for m in &b.messages {
        bytes += m.text.len();
        if m.text.len() > MAX_MESSAGE_BYTES || bytes > MAX_FILE as usize {
            return Err("Backup transcript exceeds import limits".into());
        }
        if !b.sessions.iter().any(|s| s.id == m.session_id) {
            return Err("Backup contains an orphaned message".into());
        }
    }
    Ok(())
}
fn t3_sqlite(path: &Path) -> Result<Backup, String> {
    if Path::new(&format!("{}-wal", path.display())).exists()
        || Path::new(&format!("{}-shm", path.display())).exists()
    {
        return Err("Choose a standalone consistent SQLite backup, not a live T3 database with WAL/SHM files. Close T3 and create a SQLite .backup first.".into());
    }
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF;")
        .map_err(|e| e.to_string())?;
    // Fail closed on unknown schema instead of importing a partial/misinterpreted history.
    for (table, cols) in [
        (
            "projection_projects",
            vec!["project_id", "title", "workspace_root"],
        ),
        (
            "projection_threads",
            vec!["thread_id", "project_id", "title", "model_selection_json"],
        ),
        (
            "projection_thread_messages",
            vec!["message_id", "thread_id", "role", "text"],
        ),
    ] {
        let kind: String = db
            .query_row(
                "SELECT type FROM sqlite_master WHERE name=?1",
                [table],
                |r| r.get(0),
            )
            .map_err(|_| "Unsupported T3 schema: required projection tables missing".to_owned())?;
        if kind != "table" {
            return Err("Unsupported T3 schema".into());
        }
        let mut st = db
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|e| e.to_string())?;
        let names = st
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if !cols.iter().all(|c| names.iter().any(|n| n == c)) {
            return Err(format!(
                "Unsupported T3 schema in {table}; source unchanged"
            ));
        }
    }
    let mut projects = Vec::new();
    let mut sessions = Vec::new();
    let mut messages = Vec::new();
    let mut st = db
        .prepare("SELECT project_id,title,workspace_root FROM projection_projects LIMIT 1001")
        .map_err(|e| e.to_string())?;
    for r in st
        .query_map([], |r| {
            Ok(Project {
                id: format!("t3:{}", r.get::<_, String>(0)?),
                name: r.get(1)?,
                path: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
    {
        projects.push(r.map_err(|e| e.to_string())?);
    }
    let mut st=db.prepare("SELECT thread_id,project_id,title,model_selection_json FROM projection_threads LIMIT 20001").map_err(|e|e.to_string())?;
    for r in st
        .query_map([], |r| {
            let model = r.get::<_, Option<String>>(3)?.unwrap_or_default();
            let v: serde_json::Value = serde_json::from_str(&model).unwrap_or_default();
            let p = if v
                .get("provider")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v.contains("claude"))
            {
                Provider::Claude
            } else {
                Provider::Codex
            };
            Ok(Session {
                id: format!("t3:{}", r.get::<_, String>(0)?),
                project_id: format!("t3:{}", r.get::<_, String>(1)?),
                title: r.get(2)?,
                provider: p,
                model: v
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .into(),
                remote_id: None,
                updated: now(),
            })
        })
        .map_err(|e| e.to_string())?
    {
        sessions.push(r.map_err(|e| e.to_string())?);
    }
    let mut st=db.prepare("SELECT message_id,thread_id,role,text FROM projection_thread_messages ORDER BY rowid LIMIT 20001").map_err(|e|e.to_string())?;
    for r in st
        .query_map([], |r| {
            Ok(Message {
                id: format!("t3:{}", r.get::<_, String>(0)?),
                session_id: format!("t3:{}", r.get::<_, String>(1)?),
                role: r.get(2)?,
                text: r.get(3)?,
                created: now(),
            })
        })
        .map_err(|e| e.to_string())?
    {
        let m = r.map_err(|e| e.to_string())?;
        if m.text.len() > MAX_MESSAGE_BYTES {
            return Err("T3 message exceeds 512 KiB limit".into());
        }
        messages.push(m);
    }
    Ok(Backup {
        format: "spark-code".into(),
        version: 1,
        projects,
        sessions,
        messages,
    })
}
pub fn export(path: &Path, b: &Backup) -> Result<(), String> {
    validate(b)?;
    let bytes = serde_json::to_vec_pretty(b).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unknown_version() {
        assert!(
            json_backup(
                br#"{"format":"spark-code","version":9,"projects":[],"sessions":[],"messages":[]}"#
            )
            .is_err()
        );
    }
    #[test]
    fn rejects_zip_traversal() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("bad.zip");
        let mut z = zip::ZipWriter::new(File::create(&p).unwrap());
        z.start_file(
            "../spark-code.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        z.write_all(b"{}").unwrap();
        z.finish().unwrap();
        assert!(preview(&p).unwrap_err().contains("Unsafe"));
    }
    #[test]
    fn t3_schema_and_preview_do_not_write() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("t3.db");
        let db = Connection::open(&p).unwrap();
        db.execute_batch("CREATE TABLE projection_projects(project_id TEXT,title TEXT,workspace_root TEXT);CREATE TABLE projection_threads(thread_id TEXT,project_id TEXT,title TEXT,model_selection_json TEXT);CREATE TABLE projection_thread_messages(message_id TEXT,thread_id TEXT,role TEXT,text TEXT);INSERT INTO projection_projects VALUES('p','Test','/tmp');INSERT INTO projection_threads VALUES('s','p','Imported','{}');INSERT INTO projection_thread_messages VALUES('m','s','user','hello');").unwrap();
        drop(db);
        let before = std::fs::read(&p).unwrap();
        let b = preview(&p).unwrap();
        assert_eq!(b.sessions.len(), 1);
        assert_eq!(b.messages[0].text, "hello");
        assert_eq!(std::fs::read(p).unwrap(), before);
    }
}
