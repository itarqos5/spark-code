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
    pub codex_enabled: bool,
    pub claude_enabled: bool,
    pub provider: Provider,
    pub model: String,
    pub chat_text_size: u32,
    pub compact_layout: bool,
    pub show_timestamps: bool,
    pub auto_scroll: bool,
    pub sidebar_visible: bool,
    pub activity_visible: bool,
    pub auto_detect_cli: bool,
    pub auto_refresh_providers: bool,
    pub concurrency: usize,
    pub visible_messages: usize,
    pub stream_interval_ms: u64,
    pub antialiasing: bool,
}
impl Settings {
    /// Clamp persisted preferences before they reach layout, timers, or allocations.
    pub fn normalize(&mut self) {
        self.chat_text_size = self.chat_text_size.clamp(13, 19);
        self.concurrency = self.concurrency.clamp(1, 4);
        self.visible_messages = self.visible_messages.clamp(20, VISIBLE_MESSAGES);
        self.stream_interval_ms = self.stream_interval_ms.clamp(16, 200);
    }
    pub fn enabled(&self, provider: Provider) -> bool {
        match provider {
            Provider::Codex => self.codex_enabled,
            Provider::Claude => self.claude_enabled,
        }
    }
    pub fn set_enabled(&mut self, provider: Provider, enabled: bool) {
        match provider {
            Provider::Codex => self.codex_enabled = enabled,
            Provider::Claude => self.claude_enabled = enabled,
        }
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            light_theme: false,
            reduced_motion: false,
            codex_path: "codex".into(),
            claude_path: "claude".into(),
            codex_enabled: true,
            claude_enabled: true,
            provider: Provider::Codex,
            model: String::new(),
            chat_text_size: 15,
            compact_layout: false,
            show_timestamps: false,
            auto_scroll: true,
            sidebar_visible: true,
            activity_visible: false,
            auto_detect_cli: true,
            auto_refresh_providers: true,
            concurrency: 2,
            visible_messages: 50,
            stream_interval_ms: 33,
            antialiasing: false,
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
    #[test]
    fn invalid_performance_preferences_are_bounded() {
        let mut settings: Settings = serde_json::from_str(r#"{"chat_text_size":0,"concurrency":999,"visible_messages":999999,"stream_interval_ms":0}"#).unwrap();
        settings.normalize();
        assert_eq!(settings.chat_text_size, 13);
        assert_eq!(settings.concurrency, 4);
        assert_eq!(settings.visible_messages, VISIBLE_MESSAGES);
        assert_eq!(settings.stream_interval_ms, 16);
    }
    #[test]
    fn new_preferences_survive_serialization_and_legacy_defaults() {
        let legacy: Settings = serde_json::from_str("{}").unwrap();
        assert!(legacy.auto_scroll && legacy.auto_detect_cli && legacy.sidebar_visible);
        assert_eq!(legacy.visible_messages, 50);
        let settings = Settings {
            concurrency: 4,
            stream_interval_ms: 80,
            chat_text_size: 17,
            auto_detect_cli: false,
            antialiasing: true,
            ..legacy
        };
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.concurrency, 4);
        assert_eq!(restored.stream_interval_ms, 80);
        assert_eq!(restored.chat_text_size, 17);
        assert!(!restored.auto_detect_cli);
        assert!(restored.antialiasing);
    }
}
