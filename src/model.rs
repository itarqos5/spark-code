use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const MAX_MESSAGE_BYTES: usize = 512 * 1024;
pub const VISIBLE_MESSAGES: usize = 100;
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Provider {
    #[default]
    Codex,
    Claude,
}
impl Provider {
    pub const ALL: [Self; 2] = [Self::Codex, Self::Claude];
    pub fn cli(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}
impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Codex => "Codex · ChatGPT",
            Self::Claude => "Claude Code",
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub provider: Provider,
    pub model: String,
    pub remote_id: Option<String>,
    pub updated: i64,
}
impl Session {
    pub fn new(project_id: String, provider: Provider, model: String) -> Self {
        Self {
            id: new_id(),
            project_id,
            title: "New conversation".into(),
            provider,
            model,
            remote_id: None,
            updated: now(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub text: String,
    pub created: i64,
}
impl Message {
    pub fn new(session_id: &str, role: &str, text: String) -> Self {
        Self {
            id: new_id(),
            session_id: session_id.into(),
            role: role.into(),
            text,
            created: now(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub light_theme: bool,
    pub reduced_motion: bool,
    pub codex_path: String,
    pub claude_path: String,
    pub provider: Provider,
    pub model: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            light_theme: false,
            reduced_motion: false,
            codex_path: "codex".into(),
            claude_path: "claude".into(),
            provider: Provider::Codex,
            model: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub messages: Vec<Message>,
}

#[cfg(test)]
mod appearance_tests {
    use super::*;
    #[test]
    fn old_settings_keep_provider_paths_when_appearance_fields_are_added() {
        let settings: Settings = serde_json::from_str(r#"{"codex_path":"C:/tools/codex.exe","claude_path":"C:/tools/claude.exe","provider":"Claude","model":"custom-model"}"#).unwrap();
        assert_eq!(settings.codex_path, "C:/tools/codex.exe");
        assert_eq!(settings.provider, Provider::Claude);
        assert!(!settings.light_theme);
        assert!(!settings.reduced_motion);
    }
    #[test]
    fn appearance_preferences_round_trip() {
        let settings = Settings {
            light_theme: true,
            reduced_motion: true,
            ..Settings::default()
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(restored.light_theme && restored.reduced_motion);
    }
}
