//! Previewed file imports. Opt-in live database discovery and sync live in `local_import`.
use crate::model::*;
use rusqlite::{Connection, OpenFlags};
use std::{collections::HashSet, fs::File, io::Read, path::Path};
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
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_FILE {
            return Err("Backup exceeds 64 MiB".into());
        }
        json_backup(&bytes)?
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
    let mut total = 0u64;
    let mut json = None;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.enclosed_name().is_none() || f.name().contains('\\') || f.name().contains(':') {
            return Err("Unsafe archive path".into());
        }
        if f.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("Archive symlinks are not supported".into());
        }
        total = total
            .checked_add(f.size())
            .ok_or("Expanded archive exceeds limit")?;
        if total > MAX_FILE || f.size() > MAX_FILE {
            return Err("Expanded archive exceeds 64 MiB".into());
        }
        if f.name() == "spark-code.json" {
            if json.is_some() {
                return Err("Archive contains duplicate spark-code.json entries".into());
            }
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
pub(crate) fn validate(b: &Backup) -> Result<(), String> {
    if b.format != "spark-code" || b.version != 1 {
        return Err("Unsupported backup format/version; source left untouched".into());
    }
    if b.sessions.len() > MAX_ROWS || b.messages.len() > MAX_ROWS || b.projects.len() > 1000 {
        return Err("Backup has too many records; split it into smaller backups".into());
    }
    let mut projects = HashSet::new();
    let mut sessions = HashSet::new();
    let mut messages = HashSet::new();
    let mut bytes = 0;
    for p in &b.projects {
        if p.id.is_empty() || !projects.insert(p.id.as_str()) {
            return Err("Backup has empty or duplicate project IDs".into());
        }
        for field in [&p.id, &p.name, &p.path] {
            add_bytes(&mut bytes, field.len())?;
        }
    }
    for s in &b.sessions {
        if s.id.is_empty() || !sessions.insert(s.id.as_str()) {
            return Err("Backup has empty or duplicate conversation IDs".into());
        }
        if !s.project_id.is_empty() && !projects.contains(s.project_id.as_str()) {
            return Err("Backup contains an orphaned conversation".into());
        }
        for field in [&s.id, &s.project_id, &s.title, &s.model] {
            add_bytes(&mut bytes, field.len())?;
        }
        add_bytes(&mut bytes, s.remote_id.as_ref().map_or(0, String::len))?;
    }
    for m in &b.messages {
        if m.id.is_empty() || !messages.insert(m.id.as_str()) {
            return Err("Backup has empty or duplicate message IDs".into());
        }
        if m.text.len() > MAX_MESSAGE_BYTES {
            return Err("Backup message exceeds 512 KiB limit".into());
        }
        for field in [&m.id, &m.session_id, &m.role, &m.text] {
            add_bytes(&mut bytes, field.len())?;
        }
        if !sessions.contains(m.session_id.as_str()) {
            return Err("Backup contains an orphaned message".into());
        }
    }
    Ok(())
}
fn add_bytes(total: &mut usize, size: usize) -> Result<(), String> {
    *total = total
        .checked_add(size)
        .filter(|n| *n <= MAX_FILE as usize)
        .ok_or("Backup data exceeds 64 MiB import limit")?;
    Ok(())
}
// Inspect borrowed SQLite text before allocating owned strings. This also keeps
// large metadata fields and cumulative row content inside the import budget.
fn text_column(row: &rusqlite::Row<'_>, index: usize, total: &mut usize) -> Result<String, String> {
    let value = row.get_ref(index).map_err(|e| e.to_string())?;
    let text = match value {
        rusqlite::types::ValueRef::Null => "",
        _ => value.as_str().map_err(|_| "Unsupported T3 column type")?,
    };
    if text.len() > MAX_MESSAGE_BYTES {
        return Err("T3 field exceeds 512 KiB limit".into());
    }
    add_bytes(total, text.len())?;
    Ok(text.to_owned())
}
fn t3_id(row: &rusqlite::Row<'_>, index: usize, total: &mut usize) -> Result<String, String> {
    let id = text_column(row, index, total)?;
    if id.is_empty() {
        return Err("T3 contains an empty record identifier".into());
    }
    Ok(format!("t3:{id}"))
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
    // This supported projection shape is the schema contract. T3 migration
    // counters are not a portable format version and are never guessed here.
    let mut projects = Vec::new();
    let mut sessions = Vec::new();
    let mut messages = Vec::new();
    let mut bytes = 0;
    let mut st = db
        .prepare("SELECT project_id,title,workspace_root FROM projection_projects LIMIT 1001")
        .map_err(|e| e.to_string())?;
    let mut rows = st.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if projects.len() >= 1000 {
            return Err("Backup has too many projects".into());
        }
        projects.push(Project {
            id: t3_id(row, 0, &mut bytes)?,
            name: text_column(row, 1, &mut bytes)?,
            path: text_column(row, 2, &mut bytes)?,
        });
    }
    let mut st = db.prepare("SELECT thread_id,project_id,title,model_selection_json FROM projection_threads LIMIT 20001").map_err(|e| e.to_string())?;
    let mut rows = st.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if sessions.len() >= MAX_ROWS {
            return Err("Backup has too many conversations".into());
        }
        let id = t3_id(row, 0, &mut bytes)?;
        let project_id = t3_id(row, 1, &mut bytes)?;
        let title = text_column(row, 2, &mut bytes)?;
        let model = text_column(row, 3, &mut bytes)?;
        let v: serde_json::Value =
            serde_json::from_str(if model.is_empty() { "{}" } else { &model })
                .map_err(|_| "Unsupported T3 model selection JSON")?;
        let provider = match v.get("provider").and_then(|v| v.as_str()) {
            Some("claude" | "claude-code" | "claudeCode") => Provider::Claude,
            Some("codex" | "openai") | None => Provider::Codex,
            Some(_) => return Err("Unsupported T3 provider; source unchanged".into()),
        };
        sessions.push(Session {
            id,
            project_id,
            title,
            provider,
            model: v
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into(),
            remote_id: None,
            updated: now(),
        });
    }
    let mut st = db.prepare("SELECT message_id,thread_id,role,text FROM projection_thread_messages ORDER BY rowid LIMIT 20001").map_err(|e| e.to_string())?;
    let mut rows = st.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if messages.len() >= MAX_ROWS {
            return Err("Backup has too many messages".into());
        }
        messages.push(Message {
            id: t3_id(row, 0, &mut bytes)?,
            session_id: t3_id(row, 1, &mut bytes)?,
            role: text_column(row, 2, &mut bytes)?,
            text: text_column(row, 3, &mut bytes)?,
            created: now(),
        });
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
    if bytes.len() as u64 > MAX_FILE {
        return Err("Encoded backup exceeds 64 MiB; split it before exporting".into());
    }
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
    #[test]
    fn aggregate_budget_checks_overflow_before_allocation() {
        let mut bytes = MAX_FILE as usize - 2;
        add_bytes(&mut bytes, 2).unwrap();
        assert!(add_bytes(&mut bytes, 1).is_err());
        let mut bytes = usize::MAX;
        assert!(add_bytes(&mut bytes, 1).is_err());
    }
    #[test]
    fn sqlite_columns_reject_oversized_text_before_copying() {
        let db = Connection::open_in_memory().unwrap();
        let large = "x".repeat(MAX_MESSAGE_BYTES + 1);
        let mut st = db.prepare("SELECT ?1").unwrap();
        let mut rows = st.query([large]).unwrap();
        let mut bytes = 0;
        assert!(
            text_column(rows.next().unwrap().unwrap(), 0, &mut bytes)
                .unwrap_err()
                .contains("512 KiB")
        );
        assert_eq!(bytes, 0);
    }
    #[test]
    fn live_wal_and_unknown_projection_schema_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t3.db");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE unknown(id TEXT);").unwrap();
        drop(db);
        assert!(preview(&path).unwrap_err().contains("schema"));
        std::fs::write(dir.path().join("t3.db-wal"), b"test").unwrap();
        assert!(preview(&path).unwrap_err().contains("standalone"));
    }
}
