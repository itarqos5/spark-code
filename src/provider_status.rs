//! Bounded, provider-attributed UI status. No credentials or hidden reasoning are stored.
use crate::model::{Provider, now};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessMode {
    ChatOnly,
    ReadOnly,
    #[default]
    Workspace,
    Full,
}
impl std::fmt::Display for AccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ChatOnly => "Chat only · no tools",
            Self::ReadOnly => "Read-only",
            Self::Workspace => "Project · ask approval",
            Self::Full => "Full access",
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Readiness {
    #[default]
    Unchecked,
    Checking,
    Ready,
    Missing,
    SignedOut,
    Broken,
}
impl std::fmt::Display for Readiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unchecked => "Not checked",
            Self::Checking => "Checking",
            Self::Ready => "Ready",
            Self::Missing => "Not installed",
            Self::SignedOut => "Sign in required",
            Self::Broken => "Needs attention",
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct UsageWindow {
    pub label: String,
    pub used_percent: f32,
    pub resets_at: Option<i64>,
}
#[derive(Clone, Debug, Default)]
pub struct ProviderSnapshot {
    pub readiness: Readiness,
    pub status: String,
    pub usage_windows: Vec<UsageWindow>,
    pub usage_note: String,
    pub checked_at: i64,
    pub supported_efforts: Vec<String>,
    pub model_efforts: Vec<(String, Vec<String>)>,
    pub supported_access: Vec<AccessMode>,
    pub tool_free_supported: bool,
}
impl ProviderSnapshot {
    pub fn ready(&self) -> bool {
        self.readiness == Readiness::Ready
    }
    pub fn is_stale(&self) -> bool {
        self.checked_at == 0 || now().saturating_sub(self.checked_at) > 300 || !self.ready()
    }
    pub fn efforts_for(&self, model: &str) -> &[String] {
        self.model_efforts
            .iter()
            .find(|(m, _)| m == model)
            .map(|(_, e)| e.as_slice())
            .unwrap_or(&[])
    }
    pub fn fail(&mut self, error: &str) {
        let lower = error.to_ascii_lowercase();
        self.readiness = if lower.contains("not found") || lower.contains("not installed") {
            Readiness::Missing
        } else if lower.contains("login")
            || lower.contains("sign in")
            || lower.contains("logged in")
        {
            Readiness::SignedOut
        } else {
            Readiness::Broken
        };
        self.status = bounded(error, 2048);
        // Retain last-known usage with its original timestamp; never turn a failure into 0%.
    }
}
#[derive(Default)]
pub struct ProviderStates {
    codex: ProviderSnapshot,
    claude: ProviderSnapshot,
}
impl ProviderStates {
    pub fn get(&self, provider: Provider) -> &ProviderSnapshot {
        match provider {
            Provider::Codex => &self.codex,
            Provider::Claude => &self.claude,
        }
    }
    pub fn get_mut(&mut self, provider: Provider) -> &mut ProviderSnapshot {
        match provider {
            Provider::Codex => &mut self.codex,
            Provider::Claude => &mut self.claude,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineKind {
    Status,
    ToolCall,
    ToolResult,
    Summary,
    Approval,
    Error,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineEntry {
    pub kind: TimelineKind,
    pub label: String,
    pub detail: String,
    pub elapsed_ms: u64,
}
impl TimelineEntry {
    pub fn new(kind: TimelineKind, label: &str, detail: &str) -> Self {
        Self {
            kind,
            label: bounded(label, 160),
            detail: bounded(detail, 8192),
            elapsed_ms: 0,
        }
    }
}
pub fn push_timeline(
    entries: &mut VecDeque<TimelineEntry>,
    mut entry: TimelineEntry,
    elapsed_ms: u64,
) {
    entry.label = bounded(&entry.label, 160);
    entry.detail = bounded(&entry.detail, 8192);
    entry.elapsed_ms = elapsed_ms;
    if entries.len() >= 100 {
        entries.pop_front();
    }
    entries.push_back(entry);
}
fn bounded(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.into()
    } else {
        format!("{}…", &s[..s.floor_char_boundary(max.saturating_sub(3))])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn histories_are_bounded_and_utf8_safe() {
        let mut t = VecDeque::new();
        for n in 0..120 {
            push_timeline(
                &mut t,
                TimelineEntry::new(TimelineKind::ToolResult, "tool", &"🙂".repeat(5000)),
                n,
            );
        }
        assert_eq!(t.len(), 100);
        assert_eq!(t.front().unwrap().elapsed_ms, 20);
        assert!(t.iter().all(|x| x.detail.len() <= 8192));
    }
    #[test]
    fn failure_retains_last_known_usage_and_timestamp() {
        let mut s = ProviderSnapshot {
            readiness: Readiness::Ready,
            checked_at: 42,
            usage_windows: vec![UsageWindow {
                label: "5h".into(),
                used_percent: 20.,
                resets_at: Some(100),
            }],
            ..Default::default()
        };
        s.fail("Please sign in");
        assert_eq!(s.readiness, Readiness::SignedOut);
        assert_eq!(s.checked_at, 42);
        assert_eq!(s.usage_windows[0].used_percent, 20.);
        assert!(s.is_stale());
    }
    #[test]
    fn status_is_provider_specific() {
        let mut s = ProviderStates::default();
        s.get_mut(Provider::Claude).readiness = Readiness::Ready;
        assert!(!s.get(Provider::Codex).ready());
    }
}
