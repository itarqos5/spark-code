//! Subscription-backed CLI bridges. No credentials are read, copied, or persisted here.
//! Wire references (reviewed 2026-10-02):
//! https://developers.openai.com/codex/app-server
//! https://github.com/openai/codex/tree/main/codex-rs/app-server-protocol/schema/typescript/v2
//! https://code.claude.com/docs/en/cli-reference
//! https://code.claude.com/docs/en/headless
//! https://github.com/anthropics/claude-agent-sdk-python/tree/main/src/claude_agent_sdk/_internal
//! The Claude wire format is implemented against the unmodified CLI, not the SDK.

use crate::model::{Backup, MAX_MESSAGE_BYTES, Message, Project, Provider, Session};
use crate::provider_status::{AccessMode, TimelineEntry, TimelineKind, UsageWindow};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

const MAX_LINE: usize = 1024 * 1024;
const MAX_EVENT: usize = 16 * 1024;
const POLL: Duration = Duration::from_millis(25);
const GRACE: Duration = Duration::from_secs(2);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(45);
const CANCELLED: &str = "Run cancelled";

#[derive(Clone, Debug)]
pub struct RunConfig {
    pub provider: Provider,
    pub executable: String,
    pub cwd: String,
    pub model: String,
    pub remote_id: Option<String>,
    pub prompt: String,
    pub effort: Option<String>,
    pub access: AccessMode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProviderEvent {
    Started { remote_id: String },
    Text(String),
    Tool(String),
    Approval { id: String, description: String },
    Usage(String),
    UsageSnapshot(Vec<UsageWindow>),
    Timeline(TimelineEntry),
    Models(Vec<String>),
    Done,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum ProviderCommand {
    Approve { id: String, allow: bool },
    Cancel,
}

pub struct Handle {
    pub events: Receiver<ProviderEvent>,
    pub commands: SyncSender<ProviderCommand>,
    cancelled: Arc<AtomicBool>,
    finished: Receiver<()>,
}
impl Handle {
    /// Nonblocking cancellation, including while the output queue is backpressured.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.commands.try_send(ProviderCommand::Cancel);
    }

    /// Bounded shutdown barrier for application exit. Cancel all handles first
    /// so their cleanup runs concurrently, then wait. Normal UI polling must
    /// not call this; already-completed workers return immediately.
    pub fn wait_for_shutdown(&self) -> bool {
        self.cancel();
        match self.finished.recv_timeout(Duration::from_secs(3)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => false,
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.wait_for_shutdown();
    }
}

/// Returns immediately; process discovery, authentication checks, and I/O run off the UI thread.
pub fn start(config: RunConfig) -> Result<Handle, String> {
    validate(&config)?;
    let (event_tx, events) = mpsc::sync_channel(256);
    let (commands, command_rx) = mpsc::sync_channel(16);
    let (finished_tx, finished) = mpsc::sync_channel(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    thread::Builder::new()
        .name("spark-provider".into())
        .spawn(move || {
            let sink = EventSink {
                tx: event_tx,
                cancelled: signal.clone(),
            };
            let result = match config.provider {
                Provider::Codex => run_codex(&config, &sink, &command_rx, &signal),
                Provider::Claude => run_claude(&config, &sink, &command_rx, &signal),
            };
            if let Err(error) = result
                && error != CANCELLED
            {
                let _ = sink.tx.try_send(ProviderEvent::Error(error));
            }
            let _ = sink.tx.try_send(ProviderEvent::Done);
            // run_* has returned, so every owned Transport/AuthChild has already
            // closed or killed and reaped its CLI before completion is signalled.
            let _ = finished_tx.try_send(());
        })
        .map_err(|_| "Could not start provider worker".to_string())?;
    Ok(Handle {
        events,
        commands,
        cancelled,
        finished,
    })
}

fn validate(config: &RunConfig) -> Result<(), String> {
    if config.executable.trim().is_empty() || config.executable.contains(['\0', '\n', '\r']) {
        return Err("Choose an installed CLI executable in Settings".into());
    }
    if config.cwd.is_empty() || config.cwd.contains('\0') {
        return Err("Choose a project folder".into());
    }
    if config.provider == Provider::Codex && config.access == AccessMode::ChatOnly {
        return Err("Codex tool-free mode is unavailable: this official protocol has no verified all-tools-off mode. Use Read-only general chat or select a project scope".into());
    }
    if config.provider == Provider::Claude && config.access == AccessMode::Full {
        return Err("Claude full access bypasses its approval prompts and is not enabled in this client; choose project approval mode".into());
    }
    if config.effort.as_ref().is_some_and(|e| {
        !matches!(
            e.as_str(),
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
        )
    }) {
        return Err("Unsupported reasoning effort".into());
    }
    if config.prompt.trim().is_empty() {
        return Err("Enter a message first".into());
    }
    if config.prompt.len() > MAX_MESSAGE_BYTES {
        return Err("Message exceeds the 512 KiB limit".into());
    }
    if config.model.len() > 256
        || config
            .remote_id
            .as_ref()
            .is_some_and(|s| s.len() > 256 || s.contains(['\0', '\n', '\r', '/', '\\']))
    {
        return Err("Invalid model or session identifier".into());
    }
    Ok(())
}

struct EventSink {
    tx: SyncSender<ProviderEvent>,
    cancelled: Arc<AtomicBool>,
}
impl EventSink {
    fn emit(&self, mut event: ProviderEvent) -> Result<(), String> {
        let start = Instant::now();
        loop {
            match self.tx.try_send(event) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Disconnected(_)) => return Err(CANCELLED.into()),
                Err(TrySendError::Full(value)) => event = value,
            }
            if self.cancelled.load(Ordering::Acquire) {
                return Err(CANCELLED.into());
            }
            if start.elapsed() > Duration::from_secs(2) {
                return Err("Output queue is full; the provider was stopped safely. Retry after the UI catches up".into());
            }
            thread::sleep(POLL);
        }
    }
    fn text(&self, text: &str) -> Result<(), String> {
        for part in chunks(text, MAX_EVENT) {
            self.emit(ProviderEvent::Text(part.to_owned()))?;
        }
        Ok(())
    }
    fn tool(&self, text: &str) -> Result<(), String> {
        self.timeline(TimelineKind::Status, "Provider activity", text)
    }
    fn timeline(&self, kind: TimelineKind, label: &str, detail: &str) -> Result<(), String> {
        self.emit(ProviderEvent::Timeline(TimelineEntry::new(
            kind,
            &redact(label),
            &redact(detail),
        )))
    }
    fn limits(&self, value: &Value) -> Result<(), String> {
        self.emit(ProviderEvent::Usage(rate_usage(value)))?;
        self.emit(ProviderEvent::UsageSnapshot(rate_windows(value)))
    }
}

#[derive(Debug)]
enum Wire {
    Json(Value),
    Eof,
    Error(&'static str),
}

/// Child ownership is per invocation. Never discovers, kills, or attaches to other processes.
struct Transport {
    child: Child,
    incoming: Receiver<Wire>,
    outgoing: Option<SyncSender<Value>>,
}
impl Transport {
    fn spawn(executable: &str, cwd: &str, args: &[String]) -> Result<Self, String> {
        let mut command = cli_command(executable)?;
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Leave CLI-owned subscription storage to the CLI. Never forward API billing
        // overrides or externally supplied OAuth tokens to this subscription-only app.
        for key in [
            "OPENAI_API_KEY",
            "CODEX_API_KEY",
            "OPENAI_BASE_URL",
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "CLAUDE_CODE_USE_FOUNDRY",
        ] {
            command.env_remove(key);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW; native executable only
        }
        let mut child = command.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "CLI or project folder not found. Install the official CLI yourself and select its executable in Settings".into(),
            std::io::ErrorKind::PermissionDenied => "CLI or project folder permission denied".into(),
            _ => "Could not launch the CLI. Check its executable and project folder".to_string(),
        })?;
        let stdout = child.stdout.take().ok_or("CLI stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("CLI stderr unavailable")?;
        let mut stdin = child.stdin.take().ok_or("CLI stdin unavailable")?;
        let (raw_tx, incoming) = mpsc::sync_channel(8);
        let output_tx = raw_tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_bounded_line(&mut reader, MAX_LINE) {
                    Ok(Some(line)) if line.iter().all(u8::is_ascii_whitespace) => continue,
                    Ok(Some(line)) => {
                        let packet = match serde_json::from_slice::<Value>(&line) {
                            Ok(value) if value.is_object() => Wire::Json(value),
                            _ => Wire::Error(
                                "CLI emitted malformed JSON; update the official CLI and retry",
                            ),
                        };
                        let invalid = matches!(packet, Wire::Error(_));
                        if output_tx.send(packet).is_err() || invalid {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = output_tx.send(Wire::Eof);
                        break;
                    }
                    Err(_) => {
                        let _ = output_tx.send(Wire::Error(
                            "CLI output exceeded 1 MiB per line or could not be read",
                        ));
                        break;
                    }
                }
            }
        });
        // Drain but never display or retain stderr: authentication failures may contain
        // headers, tokens, or private filesystem paths. Surface actionable safe errors.
        thread::spawn(move || {
            let mut stderr = stderr;
            let mut buf = [0_u8; 8192];
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
            }
        });
        let (outgoing, writer_rx) = mpsc::sync_channel::<Value>(16);
        thread::spawn(move || {
            for value in writer_rx {
                let mut bytes = match serde_json::to_vec(&value) {
                    Ok(b) => b,
                    Err(_) => break,
                };
                if bytes.len() > MAX_LINE {
                    let _ = raw_tx.try_send(Wire::Error("Provider request exceeds 1 MiB"));
                    break;
                }
                bytes.push(b'\n');
                if stdin.write_all(&bytes).and_then(|_| stdin.flush()).is_err() {
                    let _ = raw_tx.try_send(Wire::Error("CLI input pipe closed"));
                    break;
                }
            }
        });
        Ok(Self {
            child,
            incoming,
            outgoing: Some(outgoing),
        })
    }
    fn send(&self, value: Value) -> Result<(), String> {
        self.outgoing
            .as_ref()
            .ok_or("CLI input closed")?
            .try_send(value)
            .map_err(|_| "CLI input is busy or closed; run stopped safely".into())
    }
    fn next(&self) -> Result<Option<Value>, String> {
        match self.incoming.recv_timeout(POLL) {
            Ok(Wire::Json(v)) => Ok(Some(v)),
            Ok(Wire::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => Err("CLI exited before completing the request. Check the official CLI login and version in a terminal".into()),
            Ok(Wire::Error(e)) => Err(e.into()),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        }
    }
    fn close(&mut self) {
        self.outgoing.take();
        let deadline = Instant::now() + GRACE;
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(POLL);
        }
        kill_process(&mut self.child);
        let _ = self.child.wait();
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.outgoing.take();
        if self.child.try_wait().ok().flatten().is_none() {
            kill_process(&mut self.child);
        }
        let _ = self.child.wait();
    }
}

/// How a configured CLI path is launched. Windows installs commonly expose the official
/// CLIs as npm `.cmd`/`.ps1` shims next to (or instead of) a native `.exe`.
#[cfg(any(windows, test))]
#[derive(Debug, PartialEq, Eq)]
enum Launcher {
    Native,
    Batch,
    PowerShell,
}
#[cfg(any(windows, test))]
fn launcher_for(path: &str) -> Result<Launcher, String> {
    let lower = path.to_ascii_lowercase();
    let script = is_batch_path(path)
        || [".cmd", ".bat", ".ps1"]
            .iter()
            .any(|ext| lower.contains(ext));
    // Windows ignores trailing dots/spaces and resolves `name.cmd:stream`, so these
    // spellings would slip past an extension check. Only plain script paths are accepted.
    let trimmed = lower.trim_end_matches(['.', ' ']);
    let suspicious = trimmed.len() != lower.len()
        || path.char_indices().any(|(i, c)| c == ':' && i != 1)
        || path.contains(['\0', '\n', '\r', '"', '%']);
    if script && suspicious {
        return Err("Unsupported CLI path. Select the plain .exe, .cmd or .ps1 file".into());
    }
    Ok(if trimmed.ends_with(".cmd") || trimmed.ends_with(".bat") {
        Launcher::Batch
    } else if trimmed.ends_with(".ps1") {
        Launcher::PowerShell
    } else {
        Launcher::Native
    })
}

fn cli_command(executable: &str) -> Result<Command, String> {
    #[cfg(windows)]
    {
        match launcher_for(executable)? {
            // std quotes batch-file arguments for cmd.exe and refuses ones it cannot make
            // safe, so no shell command string is ever built here.
            Launcher::Batch => return Ok(Command::new(executable)),
            Launcher::PowerShell => {
                let mut command = Command::new("powershell.exe");
                command
                    .args([
                        "-NoLogo",
                        "-NoProfile",
                        "-NonInteractive",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-File",
                    ])
                    .arg(executable);
                return Ok(command);
            }
            Launcher::Native => {}
        }
        if Path::new(executable).extension().is_none() {
            return Ok(Command::new(format!("{executable}.exe")));
        }
    }
    Ok(Command::new(executable))
}
#[cfg(any(windows, test))]
fn is_batch_path(path: &str) -> bool {
    path.split(['/', '\\', ':']).any(|p| {
        let p = p.trim_end_matches(['.', ' ']).to_ascii_lowercase();
        p.ends_with(".cmd") || p.ends_with(".bat")
    })
}

/// Stops one child we spawned plus the processes it started. npm shims run the real CLI as
/// a grandchild, so killing only the shim would leave the CLI running.
fn kill_process(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .status();
    }
    let _ = child.kill();
}

fn read_bounded_line<R: BufRead>(reader: &mut R, limit: usize) -> std::io::Result<Option<Vec<u8>>> {
    let mut result = Vec::new();
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok((!result.is_empty()).then_some(result));
        }
        let n = buf
            .iter()
            .position(|b| *b == b'\n')
            .map_or(buf.len(), |i| i + 1);
        if result.len() + n > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "line too long",
            ));
        }
        let done = buf[n - 1] == b'\n';
        result.extend_from_slice(&buf[..n]);
        reader.consume(n);
        if done {
            return Ok(Some(result));
        }
    }
}

fn rpc(method: &str, id: u64, params: Value) -> Value {
    json!({"method":method,"id":id,"params":params})
}
fn initialization() -> Value {
    rpc(
        "initialize",
        0,
        json!({"clientInfo":{"name":"spark_code","title":"Spark Code","version":env!("CARGO_PKG_VERSION")}}),
    )
}
fn codex_thread(config: &RunConfig) -> Value {
    // Enum spellings follow the generated protocol schema, not legacy docs examples.
    let mut params = json!({"cwd":config.cwd,"modelProvider":"openai","approvalPolicy":"untrusted","approvalsReviewer":"user","sandbox":"workspace-write"});
    params["sandbox"] = match config.access {
        AccessMode::ReadOnly | AccessMode::ChatOnly => "read-only",
        AccessMode::Workspace => "workspace-write",
        AccessMode::Full => "danger-full-access",
    }
    .into();

    if !config.model.is_empty() {
        params["model"] = config.model.clone().into();
    }
    let method = if let Some(id) = &config.remote_id {
        params["threadId"] = id.clone().into();
        "thread/resume"
    } else {
        "thread/start"
    };
    rpc(method, 4, params)
}
fn codex_turn(config: &RunConfig, id: &str) -> Value {
    let mut params = json!({"threadId":id,"cwd":config.cwd,"input":[{"type":"text","text":config.prompt,"text_elements":[]}],
        "approvalPolicy":"untrusted","approvalsReviewer":"user","sandboxPolicy":{"type":"workspaceWrite","writableRoots":[config.cwd],"networkAccess":false,"excludeTmpdirEnvVar":true,"excludeSlashTmp":true}});
    if !config.model.is_empty() {
        params["model"] = config.model.clone().into();
    }
    match config.access {
        AccessMode::ReadOnly | AccessMode::ChatOnly => {
            params["sandboxPolicy"] = json!({"type":"readOnly","networkAccess":false});
        }
        AccessMode::Full => {
            params["sandboxPolicy"] = json!({"type":"dangerFullAccess"});
        }
        AccessMode::Workspace => {}
    }
    if let Some(effort) = &config.effort {
        params["effort"] = effort.clone().into();
    }
    params["summary"] = "concise".into();
    rpc("turn/start", 5, params)
}

#[derive(Clone)]
enum Approval {
    Codex {
        wire_id: Value,
        permissions: Option<Value>,
    },
    Claude {
        wire_id: String,
        input: Value,
    },
}
fn approval_response(approval: Approval, allow: bool) -> Value {
    match approval {
        Approval::Codex {
            wire_id,
            permissions: None,
        } => json!({"id":wire_id,"result":{"decision":if allow {"accept"} else {"decline"}}}),
        Approval::Codex {
            wire_id,
            permissions: Some(permissions),
        } => {
            json!({"id":wire_id,"result":{"permissions":if allow {permissions} else {json!({})},"scope":"turn"}})
        }
        Approval::Claude { wire_id, input } => {
            json!({"type":"control_response","response":{"subtype":"success","request_id":wire_id,
            "response":if allow {json!({"behavior":"allow","updatedInput":input})} else {json!({"behavior":"deny","message":"User declined this action"})}}})
        }
    }
}
fn commands(
    rx: &Receiver<ProviderCommand>,
    signal: &AtomicBool,
    pending: &mut HashMap<String, Approval>,
    transport: &Transport,
) -> Result<bool, String> {
    let mut cancel = signal.load(Ordering::Acquire);
    while let Ok(command) = rx.try_recv() {
        match command {
            ProviderCommand::Cancel => cancel = true,
            ProviderCommand::Approve { id, allow } => {
                if let Some(approval) = pending.remove(&id) {
                    transport.send(approval_response(approval, allow))?;
                }
            }
        }
    }
    Ok(cancel)
}
fn safe_error(value: &Value) -> String {
    let text = value.to_string().to_ascii_lowercase();
    if text.contains("unauthorized") || text.contains("auth") || text.contains("401") {
        "Provider authentication failed. Sign in through the official CLI in a terminal, then retry"
            .into()
    } else if text.contains("ratelimit")
        || text.contains("rate_limit")
        || text.contains("usage_limit")
        || text.contains("429")
    {
        "The provider reports a usage or rate limit. Check your subscription usage and retry after the reset".into()
    } else if text.contains("model") {
        "The provider rejected this model or configuration. Choose an available model, or leave Model blank".into()
    } else {
        "The provider reported an error. Run its official CLI in a terminal for private diagnostics; no raw credentials or error payloads are stored by Spark Code".into()
    }
}

fn run_codex(
    config: &RunConfig,
    sink: &EventSink,
    command_rx: &Receiver<ProviderCommand>,
    signal: &AtomicBool,
) -> Result<(), String> {
    let mut transport = Transport::spawn(&config.executable, &config.cwd, &["app-server".into()])?;
    transport.send(initialization())?;
    let mut pending = HashMap::new();
    let mut thread_id = String::new();
    let mut turn_id = String::new();
    let mut streamed = HashSet::new();
    let mut items: HashMap<String, String> = HashMap::new();
    let mut phase_start = Instant::now();
    let mut running = false;
    let mut cancelling: Option<Instant> = None;
    loop {
        if commands(command_rx, signal, &mut pending, &transport)? && cancelling.is_none() {
            if !thread_id.is_empty() && !turn_id.is_empty() {
                let _ = transport.send(rpc(
                    "turn/interrupt",
                    99,
                    json!({"threadId":thread_id,"turnId":turn_id}),
                ));
            }
            for (_, approval) in pending.drain() {
                let _ = transport.send(approval_response(approval, false));
            }
            cancelling = Some(Instant::now());
        }
        if cancelling.is_some_and(|t| t.elapsed() >= GRACE) {
            return Err(CANCELLED.into());
        }
        if !running && cancelling.is_none() && phase_start.elapsed() > HANDSHAKE_TIMEOUT {
            return Err("Codex did not finish initialization. Update the official CLI, check login, and retry".into());
        }
        let value = match transport.next() {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(_) if cancelling.is_some() => return Err(CANCELLED.into()),
            Err(e) => return Err(e),
        };
        if let Some(method) = value["method"].as_str() {
            let p = &value["params"];
            if let Some(wire_id) = value.get("id") {
                if cancelling.is_some() {
                    let _ = transport.send(
                        json!({"id":wire_id,"error":{"code":-32000,"message":"Run cancelled"}}),
                    );
                    continue;
                }
                let approval = match method {
                    "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                        Some(Approval::Codex {
                            wire_id: wire_id.clone(),
                            permissions: None,
                        })
                    }
                    "item/permissions/requestApproval" => Some(Approval::Codex {
                        wire_id: wire_id.clone(),
                        permissions: Some(p["permissions"].clone()),
                    }),
                    _ => None,
                };
                if let Some(approval) = approval {
                    if config.access == AccessMode::ChatOnly {
                        transport.send(approval_response(approval, false))?;
                        sink.tool(
                            "Additional permissions denied by the selected read-only access mode",
                        )?;
                        continue;
                    }
                    if p["threadId"].as_str() != Some(thread_id.as_str())
                        || (!turn_id.is_empty() && p["turnId"].as_str() != Some(turn_id.as_str()))
                        || pending.len() >= 32
                    {
                        transport.send(approval_response(approval, false))?;
                        continue;
                    }
                    if method == "item/fileChange/requestApproval"
                        && !p["itemId"]
                            .as_str()
                            .is_some_and(|id| items.contains_key(id))
                    {
                        transport.send(approval_response(approval, false))?;
                        sink.tool("File approval denied: the provider did not supply a displayable change preview")?;
                        continue;
                    }
                    let id = format!("codex:{}", wire_id);
                    let item = p["itemId"]
                        .as_str()
                        .and_then(|i| items.get(i))
                        .cloned()
                        .unwrap_or_default();
                    let description = format!("{method}\n{}\n{item}", display_json(p));
                    // Never approve details the UI could not display in full.
                    if description.len() > MAX_EVENT {
                        transport.send(approval_response(approval, false))?;
                        sink.tool("Approval denied: details exceeded the display limit. Review the action in the official CLI")?;
                        continue;
                    }
                    pending.insert(id.clone(), approval);
                    sink.emit(ProviderEvent::Approval { id, description })?;
                } else {
                    // Unsupported interactive requests fail closed rather than hang or auto-approve.
                    transport.send(json!({"id":wire_id,"error":{"code":-32601,"message":"Spark Code does not support this interactive request; use the official CLI"}}))?;
                    sink.tool(
                        "A provider interaction requires the official CLI; no action was approved",
                    )?;
                }
                continue;
            }
            if p["threadId"]
                .as_str()
                .is_some_and(|id| !thread_id.is_empty() && id != thread_id)
            {
                continue;
            }
            match method {
                "turn/started" => {
                    if let Some(id) = p["turn"]["id"].as_str() {
                        turn_id = id.into();
                        running = true;
                        if cancelling.is_some() {
                            let _ = transport.send(rpc(
                                "turn/interrupt",
                                99,
                                json!({"threadId":thread_id,"turnId":turn_id}),
                            ));
                        }
                    }
                }
                "item/agentMessage/delta" if cancelling.is_none() => {
                    if let Some(id) = p["itemId"].as_str()
                        && streamed.len() < 4096
                    {
                        streamed.insert(id.to_owned());
                    }
                    if let Some(text) = p["delta"].as_str() {
                        sink.text(text)?;
                    }
                }
                "item/started" | "item/completed" => {
                    let item = &p["item"];
                    let kind = item["type"].as_str().unwrap_or("");
                    if kind == "agentMessage" && method == "item/completed" {
                        if !streamed.contains(item["id"].as_str().unwrap_or(""))
                            && let Some(text) = item["text"].as_str()
                        {
                            sink.text(text)?;
                        }
                    } else if kind == "reasoning" && method == "item/completed" {
                        if let Some(entry) = codex_summary(item) {
                            sink.emit(ProviderEvent::Timeline(entry))?;
                        }
                    } else if matches!(
                        kind,
                        "commandExecution"
                            | "fileChange"
                            | "mcpToolCall"
                            | "dynamicToolCall"
                            | "functionCallOutput"
                            | "plan"
                            | "webSearch"
                            | "imageView"
                            | "imageGeneration"
                            | "contextCompaction"
                            | "enteredReviewMode"
                            | "exitedReviewMode"
                    ) {
                        let preview = display_json(item);
                        if items.len() < 128
                            && let Some(id) = item["id"].as_str()
                        {
                            items.insert(id.into(), truncate(&preview, MAX_EVENT));
                        }
                        sink.timeline(
                            if method == "item/started" {
                                TimelineKind::ToolCall
                            } else {
                                TimelineKind::ToolResult
                            },
                            kind,
                            &preview,
                        )?;
                    }
                }
                "item/commandExecution/outputDelta" => {
                    if let Some(text) = p["delta"].as_str() {
                        sink.timeline(TimelineKind::ToolResult, "Command output", text)?;
                    }
                }
                "turn/diff/updated" => {
                    if let Some(diff) = p["diff"].as_str() {
                        sink.timeline(TimelineKind::ToolResult, "File changes", diff)?;
                    }
                }
                "thread/tokenUsage/updated" => sink.emit(ProviderEvent::Usage(token_usage(p)))?,
                "account/rateLimits/updated" => sink.limits(p)?,
                "serverRequest/resolved" => {
                    pending.remove(&format!("codex:{}", p["requestId"]));
                }
                "error" => {
                    if p["willRetry"] == true {
                        sink.tool("Codex reported a recoverable error and is retrying")?;
                    } else {
                        return Err(safe_error(p));
                    }
                }
                "turn/completed" => {
                    if p["turn"]["status"] == "failed" {
                        sink.emit(ProviderEvent::Error(safe_error(&p["turn"]["error"])))?;
                    }
                    pending.clear();
                    transport.close();
                    return Ok(());
                }
                _ => {}
            }
            continue;
        }
        let Some(id) = value["id"].as_u64() else {
            continue;
        };
        if value.get("error").is_some() {
            if matches!(id, 2 | 3 | 99) {
                if id != 99 {
                    sink.emit(ProviderEvent::Usage(
                        "This CLI version did not provide model or usage information".into(),
                    ))?;
                }
                continue;
            }
            return Err(safe_error(&value["error"]));
        }
        if cancelling.is_some() {
            continue;
        }
        let result = &value["result"];
        match id {
            0 => {
                transport.send(json!({"method":"initialized","params":{}}))?;
                transport.send(rpc("account/read", 1, json!({"refreshToken":false})))?;
                phase_start = Instant::now();
            }
            1 => {
                if result["account"]["type"] != "chatgpt" {
                    return Err("Codex requires a ChatGPT subscription login. Run `codex login` in a terminal and choose ChatGPT. API-key and external-token modes are not used".into());
                }
                let plan = result["account"]["planType"]
                    .as_str()
                    .unwrap_or("connected");
                sink.emit(ProviderEvent::Usage(format!(
                    "ChatGPT subscription: {}",
                    redact(&truncate(plan, 128))
                )))?;
                transport.send(rpc("model/list", 2, json!({"limit":100})))?;
                transport.send(rpc("account/rateLimits/read", 3, json!({})))?;
                transport.send(codex_thread(config))?;
                phase_start = Instant::now();
            }
            2 => sink.emit(ProviderEvent::Models(model_names(result)))?,
            3 => sink.limits(result)?,
            4 => {
                thread_id = result["thread"]["id"]
                    .as_str()
                    .ok_or("Codex returned no thread identifier")?
                    .into();
                sink.emit(ProviderEvent::Started {
                    remote_id: thread_id.clone(),
                })?;
                transport.send(codex_turn(config, &thread_id))?;
                phase_start = Instant::now();
            }
            5 => {
                turn_id = result["turn"]["id"]
                    .as_str()
                    .ok_or("Codex returned no turn identifier")?
                    .into();
                running = true;
            }
            _ => {}
        }
    }
}

fn claude_args(config: &RunConfig) -> Vec<String> {
    let mut args = vec![
        "--print",
        "--output-format",
        "stream-json",
        "--input-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-mode",
        "default",
        "--permission-prompt-tool",
        "stdio",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let mode = match config.access {
        AccessMode::Full => "default",
        AccessMode::ReadOnly => "plan",
        AccessMode::ChatOnly => "dontAsk",
        AccessMode::Workspace => "default",
    };
    if let Some(pos) = args.iter().position(|a| a == "--permission-mode") {
        args[pos + 1] = mode.into();
    }
    // Subscription-preserving safe mode suppresses local customizations. Managed
    // administrator hooks/policy still apply: this is a model-tool boundary, not
    // an OS sandbox. `--bare` would remove subscription authentication.
    if matches!(config.access, AccessMode::ChatOnly | AccessMode::ReadOnly) {
        args.extend(
            [
                "--safe-mode",
                "--setting-sources",
                "",
                "--settings",
                "{\"disableAllHooks\":true,\"autoMemoryEnabled\":false}",
                "--disable-slash-commands",
            ]
            .into_iter()
            .map(str::to_owned),
        );
    }
    if config.access == AccessMode::ChatOnly {
        args.extend(
            [
                "--tools",
                "",
                "--disallowedTools",
                "*",
                "--strict-mcp-config",
                "--mcp-config",
                "{\"mcpServers\":{}}",
            ]
            .into_iter()
            .map(str::to_owned),
        );
    } else if config.access == AccessMode::ReadOnly {
        args.extend(
            [
                "--tools",
                "Read,Glob,Grep",
                "--disallowedTools",
                "mcp__*",
                "--strict-mcp-config",
                "--mcp-config",
                "{\"mcpServers\":{}}",
            ]
            .into_iter()
            .map(str::to_owned),
        );
    }
    if let Some(effort) = &config.effort {
        args.push(format!("--effort={effort}"));
    }
    // Equals form prevents a dash-leading model/session value becoming a new flag.
    if !config.model.is_empty() {
        args.push(format!("--model={}", config.model));
    }
    if let Some(id) = &config.remote_id {
        args.push(format!("--resume={id}"));
    }
    args
}
fn claude_control(id: &str, request: Value) -> Value {
    json!({"type":"control_request","request_id":id,"request":request})
}

fn claude_auth(
    config: &RunConfig,
    signal: &AtomicBool,
    commands: &Receiver<ProviderCommand>,
) -> Result<(), String> {
    // `auth status` may be pretty-printed JSON, so capture a bounded stream in a
    // dedicated reader, never Command::output() with an unbounded allocation.
    let mut cmd = cli_command(&config.executable)?;
    cmd.args(["auth", "status"])
        .current_dir(&config.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        cmd.env_remove(key);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let child = cmd.spawn().map_err(|_| "Claude Code was not found. Install its official native CLI yourself and choose the executable in Settings".to_string())?;
    let mut owned = AuthChild(child);
    let stdout = owned
        .0
        .stdout
        .take()
        .ok_or("Claude auth status stdout unavailable")?;
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take((MAX_LINE + 1) as u64).read_to_end(&mut bytes);
        let _ = tx.send((result.is_ok(), bytes));
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let bytes = loop {
        if signal.load(Ordering::Acquire)
            || matches!(commands.try_recv(), Ok(ProviderCommand::Cancel))
        {
            return Err(CANCELLED.into());
        }
        match rx.recv_timeout(POLL) {
            Ok((true, b)) if b.len() <= MAX_LINE => break b,
            Ok(_) => return Err("Claude auth status was unreadable or oversized".into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Claude auth status failed".into());
            }
            _ => {}
        }
        if Instant::now() > deadline {
            return Err(
                "Claude auth status timed out. Check the official CLI in a terminal".into(),
            );
        }
    };
    let result: Value = serde_json::from_slice(&bytes).map_err(|_| "This Claude CLI does not expose a recognized auth status. Update the official CLI and retry".to_string())?;
    if result["loggedIn"] != true || result["authMethod"] != "claude.ai" {
        return Err("Claude requires its own subscription login. Run `claude auth login` in a terminal. API keys, API-key helpers, external OAuth tokens, and third-party billing are not used".into());
    }
    Ok(())
}
struct AuthChild(Child);
impl Drop for AuthChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            kill_process(&mut self.0);
        }
        let _ = self.0.wait();
    }
}

fn run_claude(
    config: &RunConfig,
    sink: &EventSink,
    command_rx: &Receiver<ProviderCommand>,
    signal: &AtomicBool,
) -> Result<(), String> {
    claude_auth(config, signal, command_rx)?;
    sink.emit(ProviderEvent::Usage(
        "Claude subscription connected; quota percentages are not provided by the CLI stream"
            .into(),
    ))?;
    let mut transport = Transport::spawn(&config.executable, &config.cwd, &claude_args(config))?;
    transport.send(claude_control(
        "spark-init",
        json!({"subtype":"initialize","hooks":null}),
    ))?;
    let mut pending = HashMap::new();
    let mut initialized = false;
    let mut seen_session = false;
    let mut streamed_message = false;
    let mut got_text = false;
    let start = Instant::now();
    let mut cancelling: Option<Instant> = None;
    loop {
        if commands(command_rx, signal, &mut pending, &transport)? && cancelling.is_none() {
            for (_, approval) in pending.drain() {
                let _ = transport.send(approval_response(approval, false));
            }
            let _ = transport.send(claude_control("spark-stop", json!({"subtype":"interrupt"})));
            cancelling = Some(Instant::now());
        }
        if cancelling.is_some_and(|t| t.elapsed() >= GRACE) {
            return Err(CANCELLED.into());
        }
        if !initialized && start.elapsed() > HANDSHAKE_TIMEOUT {
            return Err(
                "Claude did not initialize its stream. Update the official CLI and retry".into(),
            );
        }
        let value = match transport.next() {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(_) if cancelling.is_some() => return Err(CANCELLED.into()),
            Err(e) => return Err(e),
        };
        if !seen_session
            && let Some(id) = value["session_id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 256)
        {
            sink.emit(ProviderEvent::Started {
                remote_id: id.into(),
            })?;
            seen_session = true;
        }
        match value["type"].as_str().unwrap_or("") {
            "control_response" if value["response"]["request_id"] == "spark-init" => {
                if value["response"]["subtype"] == "error" {
                    return Err("Claude does not support the required CLI control protocol. Update the official CLI".into());
                }
                if cancelling.is_none() {
                    let models = value["response"]["response"]["models"]
                        .as_array()
                        .map(|m| {
                            m.iter()
                                .filter_map(|v| {
                                    v["value"]
                                        .as_str()
                                        .or_else(|| v["id"].as_str())
                                        .filter(|s| s.len() <= 256)
                                        .map(str::to_owned)
                                })
                                .take(100)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if !models.is_empty() {
                        sink.emit(ProviderEvent::Models(models))?;
                    }
                    transport.send(json!({"type":"user","session_id":config.remote_id.as_deref().unwrap_or(""),"parent_tool_use_id":null,"message":{"role":"user","content":config.prompt}}))?;
                    initialized = true;
                }
            }
            "control_request" => {
                let Some(id) = value["request_id"].as_str() else {
                    continue;
                };
                let request = &value["request"];
                if request["subtype"] == "can_use_tool" {
                    let approval = Approval::Claude {
                        wire_id: id.into(),
                        input: request["input"].clone(),
                    };
                    let description = format!(
                        "{}\n{}",
                        request["tool_name"].as_str().unwrap_or("Tool action"),
                        display_json(request)
                    );
                    if config.access == AccessMode::ChatOnly
                        || cancelling.is_some()
                        || pending.len() >= 32
                        || description.len() > MAX_EVENT
                    {
                        transport.send(approval_response(approval, false))?;
                        sink.tool("Tool approval denied because the request was cancelled, oversized, or exceeded the pending limit")?;
                    } else {
                        let key = format!("claude:{id}");
                        pending.insert(key.clone(), approval);
                        sink.emit(ProviderEvent::Approval {
                            id: key,
                            description,
                        })?;
                    }
                } else {
                    transport.send(json!({"type":"control_response","response":{"subtype":"error","request_id":id,"error":"Unsupported interactive request; use the official Claude CLI"}}))?;
                }
            }
            "control_cancel_request" => {
                if let Some(id) = value["request_id"].as_str() {
                    pending.remove(&format!("claude:{id}"));
                }
            }
            "stream_event" if value["parent_tool_use_id"].is_null() => {
                let event = &value["event"];
                if event["type"] == "message_start" {
                    streamed_message = false;
                }
                if event["delta"]["type"] == "text_delta"
                    && let Some(text) = event["delta"]["text"].as_str()
                {
                    sink.text(text)?;
                    streamed_message = true;
                    got_text = true;
                }
            }
            "assistant" => {
                if let Some(parts) = value["message"]["content"].as_array() {
                    for part in parts {
                        match part["type"].as_str() {
                            Some("text")
                                if !streamed_message && value["parent_tool_use_id"].is_null() =>
                            {
                                if let Some(text) = part["text"].as_str() {
                                    sink.text(text)?;
                                    got_text = true;
                                }
                            }
                            Some("tool_use") => sink.timeline(
                                TimelineKind::ToolCall,
                                part["name"].as_str().unwrap_or("Tool"),
                                &display_json(&part["input"]),
                            )?,
                            _ => {}
                        }
                    }
                }
                if let Some(error) = value.get("error") {
                    sink.emit(ProviderEvent::Error(safe_error(error)))?;
                }
            }
            "user" => {
                if let Some(parts) = value["message"]["content"].as_array() {
                    for part in parts {
                        if let Some(entry) = claude_tool_result(part) {
                            sink.emit(ProviderEvent::Timeline(entry))?;
                        }
                    }
                }
            }
            "system" if value["subtype"] == "permission_denied" => {
                sink.tool("Claude denied an action under its permission policy")?
            }
            "result" => {
                if value["is_error"] == true {
                    sink.emit(ProviderEvent::Error(safe_error(&value)))?;
                } else if !got_text && let Some(text) = value["result"].as_str() {
                    sink.text(text)?;
                }
                sink.emit(ProviderEvent::Usage(claude_usage(&value)))?;
                if value["permission_denials"]
                    .as_array()
                    .is_some_and(|d| !d.is_empty())
                {
                    sink.tool("One or more tool actions were denied; see the official CLI for permission configuration")?;
                }
                pending.clear();
                transport.close();
                return Ok(());
            }
            _ => {}
        }
    }
}

// Only the documented, provider-exposed summary is visible. `content` and
// reasoning/textDelta (raw reasoning), Claude thinking and signature fields are ignored.
fn codex_summary(item: &Value) -> Option<TimelineEntry> {
    if item["type"] != "reasoning" {
        return None;
    }
    let summary = item["summary"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .take(16)
        .collect::<Vec<_>>()
        .join("\n");
    if summary.is_empty() {
        None
    } else {
        Some(TimelineEntry::new(
            TimelineKind::Summary,
            "Provider reasoning summary",
            &redact(&truncate(&summary, 2048)),
        ))
    }
}
fn claude_tool_result(part: &Value) -> Option<TimelineEntry> {
    if part["type"] != "tool_result" {
        return None;
    }
    let label = if part["is_error"] == true {
        "Tool failed"
    } else {
        "Tool result"
    };
    Some(TimelineEntry::new(
        TimelineKind::ToolResult,
        label,
        &display_json(&part["content"]),
    ))
}
fn rate_windows(v: &Value) -> Vec<UsageWindow> {
    let limits = &v["rateLimits"];
    ["primary", "secondary"]
        .into_iter()
        .filter_map(|name| {
            let w = &limits[name];
            let percent = w["usedPercent"].as_f64()?;
            if !percent.is_finite() || percent < 0. {
                return None;
            }
            let label = match w["windowDurationMins"].as_u64() {
                Some(300) => "5-hour window".into(),
                Some(10080) => "Weekly window".into(),
                Some(m) if m % 60 == 0 => format!("{}-hour window", m / 60),
                Some(m) => format!("{m}-minute window"),
                None => format!("{name} window"),
            };
            Some(UsageWindow {
                label,
                used_percent: percent.min(100.) as f32,
                resets_at: w["resetsAt"].as_i64(),
            })
        })
        .collect()
}
fn codex_efforts(v: &Value) -> Vec<(String, Vec<String>)> {
    v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["hidden"] != true)
        .take(100)
        .filter_map(|m| {
            let name = m["model"].as_str().or_else(|| m["id"].as_str())?;
            if name.len() > 256 {
                return None;
            }
            let efforts = m["supportedReasoningEfforts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| e["reasoningEffort"].as_str())
                .filter(|e| {
                    matches!(
                        *e,
                        "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
                    )
                })
                .take(8)
                .map(str::to_owned)
                .collect();
            Some((name.into(), efforts))
        })
        .collect()
}
fn token_usage(v: &Value) -> String {
    let usage = &v["tokenUsage"]["total"];
    format!(
        "Session tokens: input {}, output {}",
        usage["inputTokens"].as_u64().unwrap_or(0),
        usage["outputTokens"].as_u64().unwrap_or(0)
    )
}
fn claude_usage(v: &Value) -> String {
    format!(
        "Turn tokens: input {}, output {} (usage reported by Claude CLI; not a quota percentage)",
        v["usage"]["input_tokens"].as_u64().unwrap_or(0),
        v["usage"]["output_tokens"].as_u64().unwrap_or(0)
    )
}
fn rate_usage(v: &Value) -> String {
    let limits = &v["rateLimits"];
    let mut windows = Vec::new();
    for name in ["primary", "secondary"] {
        let w = &limits[name];
        if let Some(percent) = w["usedPercent"].as_f64() {
            let reset = w["resetsAt"]
                .as_i64()
                .map(|v| format!(", reset Unix {v}"))
                .unwrap_or_default();
            windows.push(format!("{name}: {percent:.0}% used{reset}"));
        }
    }
    if windows.is_empty() {
        "Subscription rate limits unavailable from this CLI/account".into()
    } else {
        windows.join(" · ")
    }
}
fn model_names(v: &Value) -> Vec<String> {
    v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|v| v["hidden"] != true)
                .filter_map(|v| {
                    v["model"]
                        .as_str()
                        .or_else(|| v["id"].as_str())
                        .filter(|s| s.len() <= 256)
                        .map(str::to_owned)
                })
                .take(100)
                .collect()
        })
        .unwrap_or_default()
}
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.into();
    }
    let end = s.floor_char_boundary(max.saturating_sub(32));
    format!("{}\n[display truncated]", &s[..end])
}
fn chunks(s: &str, max: usize) -> Vec<&str> {
    let mut rest = s;
    let mut out = Vec::new();
    while rest.len() > max {
        let n = rest.floor_char_boundary(max);
        out.push(&rest[..n]);
        rest = &rest[n..];
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}
fn redact(text: &str) -> String {
    let mut result = String::new();
    let mut redact_next = false;
    for word in text.split_inclusive(char::is_whitespace) {
        let lower = word.to_ascii_lowercase();
        let secret = redact_next
            || lower.contains("sk-ant-")
            || lower.contains("sk-proj-")
            || lower.starts_with("sk-")
            || lower.starts_with("eyj")
            || ["api_key=", "token=", "password=", "authorization="]
                .iter()
                .any(|p| lower.contains(p));
        redact_next = lower.trim_end() == "bearer";
        if secret {
            result.push_str("[redacted]");
            if word.ends_with(char::is_whitespace) {
                result.push(' ');
            }
        } else {
            result.push_str(word);
        }
    }
    result
}
fn display_json(value: &Value) -> String {
    fn clean(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .map(|(k, v)| {
                        let lower = k.to_ascii_lowercase();
                        let sensitive = [
                            "token",
                            "secret",
                            "password",
                            "apikey",
                            "api_key",
                            "authorization",
                            "credential",
                        ]
                        .iter()
                        .any(|p| lower.contains(p));
                        (
                            k.clone(),
                            if sensitive {
                                Value::String("[redacted]".into())
                            } else {
                                clean(v)
                            },
                        )
                    })
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(clean).collect()),
            Value::String(s) => Value::String(redact(s)),
            _ => v.clone(),
        }
    }
    serde_json::to_string_pretty(&clean(value)).unwrap_or_default()
}

/// Explicit, read-only history import for one project. Run this on a background
/// thread only after an import click; normal startup never calls thread/list/read.
/// Existing provider files are never accessed by this application.
pub fn import_codex(executable: String, cwd: String) -> Result<Backup, String> {
    if !Path::new(&cwd).is_dir() {
        return Err("Select an existing project folder first".into());
    }
    let cwd = Path::new(&cwd)
        .canonicalize()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();
    let mut transport = Transport::spawn(&executable, &cwd, &["app-server".into()])?;
    let mut seq = 0;
    transport.send(initialization())?;
    import_response(&transport, seq)?;
    transport.send(json!({"method":"initialized","params":{}}))?;
    let project = Project {
        id: format!("codex-project:{cwd}"),
        name: Path::new(&cwd)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imported project".into()),
        path: cwd.clone(),
    };
    let mut backup = Backup {
        format: "spark-code".into(),
        version: 1,
        projects: vec![project.clone()],
        sessions: vec![],
        messages: vec![],
    };
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    let mut cursors = HashSet::new();
    let mut bytes = 0;
    loop {
        seq += 1;
        transport.send(rpc(
            "thread/list",
            seq,
            json!({"cwd":cwd,"limit":50,"cursor":cursor,"archived":false,"sourceKinds":["cli","vscode","exec","appServer","unknown"]}),
        ))?;
        let page = import_response(&transport, seq)?;
        let data = page["data"]
            .as_array()
            .ok_or("Codex returned an invalid thread list")?;
        for record in data {
            let Some(id) = record["id"].as_str() else {
                continue;
            };
            if !seen.insert(id.to_owned()) {
                continue;
            }
            if seen.len() > 200 {
                return Err("This project has more than 200 Codex sessions; use a smaller explicit export instead".into());
            }
            seq += 1;
            transport.send(rpc(
                "thread/read",
                seq,
                json!({"threadId":id,"includeTurns":true}),
            ))?;
            let read = import_response(&transport, seq)?;
            let t = &read["thread"];
            let mut session = Session::new(
                project.id.clone(),
                Provider::Codex,
                t["model"].as_str().unwrap_or("").into(),
            );
            session.id = format!("codex:{id}");
            session.remote_id = Some(id.into());
            session.title = truncate(
                t["name"]
                    .as_str()
                    .or_else(|| t["preview"].as_str())
                    .unwrap_or("Imported Codex conversation"),
                160,
            );
            session.updated = t["updatedAt"].as_i64().unwrap_or(session.updated);
            if let Some(turns) = t["turns"].as_array() {
                for (turn_index, turn) in turns.iter().enumerate() {
                    if let Some(items) = turn["items"].as_array() {
                        for (item_index, item) in items.iter().enumerate() {
                            let (role, text) = match item["type"].as_str() {
                                Some("agentMessage") => {
                                    ("assistant", item["text"].as_str().unwrap_or("").to_owned())
                                }
                                Some("userMessage") => (
                                    "user",
                                    item["content"]
                                        .as_array()
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|p| p["text"].as_str())
                                                .collect::<Vec<_>>()
                                                .join("\n")
                                        })
                                        .unwrap_or_default(),
                                ),
                                _ => continue,
                            };
                            if text.is_empty() {
                                continue;
                            }
                            bytes += text.len();
                            if text.len() > MAX_MESSAGE_BYTES
                                || bytes > 16 * 1024 * 1024
                                || backup.messages.len() >= 10000
                            {
                                return Err("Codex import exceeded safe history limits; use a smaller explicit export instead".into());
                            }
                            let mut message = Message::new(&session.id, role, text);
                            let item_key = item["id"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| format!("{turn_index}:{item_index}"));
                            message.id = format!("codex:{id}:{item_key}");
                            backup.messages.push(message);
                        }
                    }
                }
            }
            backup.sessions.push(session);
        }
        cursor = page["nextCursor"].as_str().map(str::to_owned);
        if let Some(c) = &cursor {
            if !cursors.insert(c.clone()) {
                return Err("Codex repeated a pagination cursor; import stopped".into());
            }
        } else {
            break;
        }
    }
    transport.close();
    Ok(backup)
}
fn import_response(transport: &Transport, id: u64) -> Result<Value, String> {
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(v) = transport.next()? {
            if v.get("method").is_some() {
                if let Some(id) = v.get("id") {
                    transport.send(json!({"id":id,"error":{"code":-32601,"message":"Read-only import cannot approve actions"}}))?;
                }
            } else if v["id"].as_u64() == Some(id) {
                if v.get("error").is_some() {
                    return Err(safe_error(&v["error"]));
                }
                return Ok(v["result"].clone());
            }
        }
    }
    Err("Codex history request timed out; no partial import was applied".into())
}

#[derive(Clone, Debug)]
pub struct ProviderInfo {
    pub version: String,
    pub models: Vec<String>,
    pub usage: String,
    pub status: String,
    pub usage_windows: Vec<UsageWindow>,
    pub supported_efforts: Vec<String>,
    pub model_efforts: Vec<(String, Vec<String>)>,
    pub supported_access: Vec<AccessMode>,
    pub tool_free_supported: bool,
}

fn login_arguments(provider: Provider) -> &'static [&'static str] {
    match provider {
        Provider::Codex => &["login"],
        Provider::Claude => &["auth", "login"],
    }
}

/// Opens the official CLI's own login flow after an explicit user click.
/// Success means the console/terminal was launched, not that sign-in completed.
/// No credentials are supplied, inspected, copied, or saved by Spark Code.
pub fn launch_login(provider: Provider, executable: String) -> Result<(), String> {
    if executable.trim().is_empty()
        || executable.starts_with('-')
        || executable.contains(['\0', '\n', '\r'])
    {
        return Err("Choose an installed official CLI executable first".into());
    }
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = cli_command(&executable)?;
        command
            .args(login_arguments(provider))
            .creation_flags(0x00000010); // CREATE_NEW_CONSOLE
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("x-terminal-emulator");
        command
            .arg("-e")
            .arg(&executable)
            .args(login_arguments(provider));
        command
    };
    for key in [
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
    ] {
        command.env_remove(key);
    }
    // Deliberately do not retain/kill this child: the separate console belongs to
    // the user, who completes and closes their own official login flow.
    command.spawn().map_err(|_| "Could not open a login console. Run `codex login` or `claude auth login` in your terminal, then refresh accounts".to_string())?;
    Ok(())
}

/// Finds the official CLI on PATH (`where` on Windows, `which` elsewhere) and returns the
/// best full path: a native binary first, then `.cmd`/`.bat`, then `.ps1` shims.
pub fn detect_cli(provider: Provider) -> Option<String> {
    let name = provider.cli();
    let mut found: Vec<String> = Vec::new();
    #[cfg(windows)]
    let queries = [name.to_owned(), format!("{name}.ps1")];
    #[cfg(not(windows))]
    let queries = [name.to_owned()];
    for query in queries {
        #[cfg(windows)]
        let mut command = {
            use std::os::windows::process::CommandExt;
            let mut command = Command::new("where.exe");
            command.creation_flags(0x08000000);
            command
        };
        #[cfg(not(windows))]
        let mut command = Command::new("which");
        let Ok(output) = command
            .arg(&query)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        found.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.trim().to_owned())
                .filter(|line| !line.is_empty() && Path::new(line).is_file()),
        );
    }
    #[cfg(windows)]
    if provider == Provider::Codex {
        // npm exposes shell shims on PATH. Prefer the official native binary
        // shipped alongside that installation to avoid an extra shell per turn.
        let native: Vec<String> = found
            .iter()
            .filter_map(|shim| bundled_codex_binary(Path::new(shim)))
            .collect();
        found.extend(native);
    }
    let rank = |path: &str| match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("exe") => Some(0),
        Some("cmd") | Some("bat") => Some(1),
        Some("ps1") => Some(2),
        // Extensionless files are POSIX scripts/binaries on Unix; npm's extensionless
        // Windows shim is a sh script that cannot be launched natively.
        None if cfg!(not(windows)) => Some(0),
        _ => None,
    };
    found
        .into_iter()
        .filter_map(|path| rank(&path).map(|r| (r, path)))
        .min_by_key(|(r, _)| *r)
        .map(|(_, path)| path)
}

#[cfg(any(windows, test))]
fn bundled_codex_binary(shim: &Path) -> Option<String> {
    let root = shim.parent()?;
    let paths = [
        "node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
        "node_modules/@openai/codex/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
    ];
    paths
        .into_iter()
        .map(|relative| root.join(relative))
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

/// Explicit account/model refresh. This is blocking: call it from a UI worker.
/// No prompt, inference, login, or history request is sent.
pub fn probe(provider: Provider, executable: String, cwd: String) -> Result<ProviderInfo, String> {
    let mut info = probe_account(provider, executable.clone(), cwd)?;
    info.version = cli_version(&executable);
    Ok(info)
}

/// Best-effort `--version` of the selected CLI, reduced to its version number.
fn cli_version(executable: &str) -> String {
    let Ok(mut command) = cli_command(executable) else {
        return String::new();
    };
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(output) = command.output() else {
        return String::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find(|word| word.starts_with(|c: char| c.is_ascii_digit()))
        .map(|word| truncate(word, 32))
        .unwrap_or_default()
}

fn probe_account(
    provider: Provider,
    executable: String,
    cwd: String,
) -> Result<ProviderInfo, String> {
    if executable.trim().is_empty() || executable.contains(['\0', '\n', '\r']) {
        return Err("Choose an installed official CLI executable first".into());
    }
    if !Path::new(&cwd).is_dir() {
        return Err("Choose an existing project folder first".into());
    }
    match provider {
        Provider::Codex => {
            let mut transport = Transport::spawn(&executable, &cwd, &["app-server".into()])?;
            transport.send(initialization())?;
            import_response(&transport, 0)?;
            transport.send(json!({"method":"initialized","params":{}}))?;
            transport.send(rpc("account/read", 1, json!({"refreshToken":false})))?;
            let account = import_response(&transport, 1)?;
            if account["account"]["type"] != "chatgpt" {
                return Err("Sign in to the official Codex CLI with ChatGPT. API-key and external-token modes are not used".into());
            }
            let plan = account["account"]["planType"]
                .as_str()
                .unwrap_or("connected");
            let status = format!(
                "ChatGPT subscription connected ({})",
                redact(&truncate(plan, 128))
            );
            transport.send(rpc("model/list", 2, json!({"limit":100})))?;
            let catalog = import_response(&transport, 2)?;
            let models = model_names(&catalog);
            let model_efforts = codex_efforts(&catalog);
            let mut supported_efforts = Vec::new();
            for (_, efforts) in &model_efforts {
                for effort in efforts {
                    if !supported_efforts.contains(effort) {
                        supported_efforts.push(effort.clone());
                    }
                }
            }
            transport.send(rpc("account/rateLimits/read", 3, json!({})))?;
            let limits = import_response(&transport, 3);
            let usage_windows = limits.as_ref().map(rate_windows).unwrap_or_default();
            let usage = limits.map(|v| rate_usage(&v)).unwrap_or_else(|_| {
                "Subscription limits are unavailable from this CLI/account".into()
            });
            transport.close();
            Ok(ProviderInfo {
                version: String::new(),
                models,
                usage,
                status,
                usage_windows,
                supported_efforts,
                model_efforts,
                supported_access: vec![
                    AccessMode::ReadOnly,
                    AccessMode::Workspace,
                    AccessMode::Full,
                ],
                tool_free_supported: false,
            })
        }
        Provider::Claude => {
            let config = RunConfig {
                provider,
                executable,
                cwd,
                model: String::new(),
                remote_id: None,
                prompt: String::new(),
                effort: None,
                access: AccessMode::ChatOnly,
            };
            let (_sender, receiver) = mpsc::sync_channel(1);
            claude_auth(&config, &AtomicBool::new(false), &receiver)?;
            let mut transport =
                Transport::spawn(&config.executable, &config.cwd, &claude_args(&config))?;
            transport.send(claude_control(
                "spark-probe",
                json!({"subtype":"initialize","hooks":null}),
            ))?;
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut models = Vec::new();
            let mut handshake_ok = false;
            while Instant::now() < deadline {
                let value = match transport.next() {
                    Ok(Some(v)) => v,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                if value["type"] == "control_response"
                    && value["response"]["request_id"] == "spark-probe"
                {
                    if value["response"]["subtype"] == "success" {
                        handshake_ok = true;
                        models = value["response"]["response"]["models"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| {
                                        v["value"].as_str().or_else(|| v["id"].as_str())
                                    })
                                    .filter(|s| s.len() <= 256)
                                    .take(100)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                    break;
                }
                if value["type"] == "control_request" {
                    transport.send(json!({"type":"control_response","response":{"subtype":"error",
                        "request_id":value["request_id"],"error":"Read-only account refresh cannot approve actions"}}))?;
                }
            }
            transport.close();
            if !handshake_ok {
                return Err("Claude CLI could not verify the required safe stream protocol. Update the official CLI and refresh".into());
            }
            Ok(ProviderInfo {
                version: String::new(),
                models,
                usage: "Subscription quota and reset unavailable from the official Claude CLI stream; token counts are not quota percentages".into(),
                status: "Claude subscription connected".into(),
                usage_windows:vec![],
                supported_efforts:vec![],
                model_efforts:vec![],
                supported_access:vec![AccessMode::ChatOnly,AccessMode::ReadOnly,AccessMode::Workspace],
                tool_free_supported:true,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    static MOCK_IO: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn config(provider: Provider) -> RunConfig {
        RunConfig {
            provider,
            executable: provider.cli().into(),
            cwd: "/tmp".into(),
            model: String::new(),
            remote_id: None,
            prompt: "Hello; $(no shell)\nnext".into(),
            effort: None,
            access: AccessMode::Workspace,
        }
    }
    #[test]
    fn bounded_lines_reject_oversize() {
        assert!(read_bounded_line(&mut std::io::Cursor::new(vec![b'x'; 10]), 9).is_err());
        assert_eq!(
            read_bounded_line(&mut std::io::Cursor::new(b"abc\nrest"), 4)
                .unwrap()
                .unwrap(),
            b"abc\n"
        );
    }
    #[test]
    fn codex_schema_and_safe_permissions() {
        let c = config(Provider::Codex);
        let t = codex_thread(&c);
        assert_eq!(t["params"]["sandbox"], "workspace-write");
        assert_eq!(t["params"]["approvalPolicy"], "untrusted");
        let turn = codex_turn(&c, "thread");
        assert_eq!(turn["params"]["sandboxPolicy"]["networkAccess"], false);
        assert_eq!(turn["params"]["input"][0]["text"], c.prompt);
    }
    #[test]
    fn approvals_preserve_wire_id_and_never_grant_session() {
        let a = Approval::Codex {
            wire_id: json!(42),
            permissions: None,
        };
        assert_eq!(
            approval_response(a, true),
            json!({"id":42,"result":{"decision":"accept"}})
        );
        let a = Approval::Claude {
            wire_id: "r".into(),
            input: json!({"command":"echo ok"}),
        };
        let v = approval_response(a, false);
        assert_eq!(v["response"]["response"]["behavior"], "deny");
        let a = Approval::Codex {
            wire_id: json!("r"),
            permissions: Some(json!({"network":true})),
        };
        assert_eq!(
            approval_response(a, false)["result"]["permissions"],
            json!({})
        );
    }
    #[test]
    fn flags_cannot_be_injected() {
        let mut c = config(Provider::Claude);
        c.model = "--dangerously-skip-permissions".into();
        c.remote_id = Some("--dangerously-skip-permissions".into());
        let args = claude_args(&c);
        assert!(!args.contains(&"--dangerously-skip-permissions".into()));
        assert!(args.contains(&"--resume=--dangerously-skip-permissions".into()));
        assert!(!args.contains(&c.prompt));
    }
    #[test]
    fn windows_script_variants_are_classified_safely() {
        for p in [
            "C:\\bin\\claude.CMD. ",
            "a.cmd:stream",
            "C:\\x.bat\\...\\..",
            "C:\\x.ps1 ",
            "C:\\x.cmd\"&calc",
        ] {
            assert!(launcher_for(p).is_err(), "{p}");
        }
        assert_eq!(launcher_for("C:\\bin\\claude.cmd"), Ok(Launcher::Batch));
        assert_eq!(launcher_for("C:\\bin\\codex.PS1"), Ok(Launcher::PowerShell));
        assert_eq!(launcher_for("C:\\bin\\claude.exe"), Ok(Launcher::Native));
        assert_eq!(launcher_for("claude"), Ok(Launcher::Native));
    }
    #[test]
    fn login_uses_only_official_subscription_login_arguments() {
        assert_eq!(login_arguments(Provider::Codex), &["login"]);
        assert_eq!(login_arguments(Provider::Claude), &["auth", "login"]);
        // Input validation only: this test never spawns a login process.
        assert!(launch_login(Provider::Claude, "\n".into()).is_err());
        assert!(launch_login(Provider::Codex, "--api-key".into()).is_err());
    }
    #[test]
    fn npm_codex_detection_finds_its_native_binary_without_reading_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("codex.cmd");
        assert!(bundled_codex_binary(&shim).is_none());
        let binary = dir.path().join("node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe");
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, []).unwrap();
        assert_eq!(bundled_codex_binary(&shim).as_deref(), binary.to_str());
    }

    #[test]
    fn secrets_are_not_in_diagnostics() {
        let raw = json!({"accessToken":"super-secret","nested":{"password":"dontshow"},"command":"curl -H Bearer sk-ant-abc"});
        let text = display_json(&raw);
        assert!(!text.contains("super-secret"));
        assert!(!text.contains("dontshow"));
        assert!(!text.contains("sk-ant-abc"));
        assert!(!safe_error(&raw).contains("super-secret"));
    }
    #[test]
    fn unicode_bounds_hold() {
        let s = "💡".repeat(10000);
        for p in chunks(&s, 100) {
            assert!(p.len() <= 100);
        }
        assert_eq!(chunks(&s, 100).join(""), s);
        assert!(truncate(&s, 100).len() <= 100);
    }
    #[test]
    fn usage_windows_use_only_official_numbers_and_resets() {
        let windows = rate_windows(
            &json!({"rateLimits":{"primary":{"usedPercent":25.5,"resetsAt":1780000000,"windowDurationMins":300},"secondary":{"usedPercent":80,"windowDurationMins":10080}}}),
        );
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].used_percent, 25.5);
        assert_eq!(windows[0].resets_at, Some(1780000000));
        assert_eq!(windows[0].label, "5-hour window");
        assert_eq!(windows[1].label, "Weekly window");
        assert!(windows[1].resets_at.is_none());
        assert!(rate_windows(&json!({"usage":{"input_tokens":1000}})).is_empty());
        assert!(rate_windows(&json!({"rateLimits":{"primary":{"usedPercent":-1}}})).is_empty());
    }
    #[test]
    fn reasoning_summary_never_displays_raw_content() {
        let e=codex_summary(&json!({"type":"reasoning","summary":["Brief provider summary"],"content":["PRIVATE RAW REASONING"]})).unwrap();
        assert_eq!(e.kind, TimelineKind::Summary);
        assert_eq!(e.detail, "Brief provider summary");
        assert!(!e.detail.contains("PRIVATE"));
        assert!(codex_summary(&json!({"type":"reasoning","content":["PRIVATE"]})).is_none());
        assert!(claude_tool_result(&json!({"type":"thinking","thinking":"PRIVATE"})).is_none());
        assert!(
            claude_tool_result(&json!({"type":"signature_delta","signature":"PRIVATE"})).is_none()
        );
    }
    #[test]
    fn claude_tool_result_preserves_actual_result_and_redacts_keys() {
        let e=claude_tool_result(&json!({"type":"tool_result","content":{"stdout":"tests passed","api_key":"sk-private"},"is_error":false})).unwrap();
        assert_eq!(e.kind, TimelineKind::ToolResult);
        assert!(e.detail.contains("tests passed"));
        assert!(!e.detail.contains("sk-private"));
    }
    #[test]
    fn codex_effort_is_model_specific_and_protocol_checked() {
        let efforts = codex_efforts(
            &json!({"data":[{"model":"small","supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"high"},{"reasoningEffort":"fictional"}]},{"model":"hidden","hidden":true,"supportedReasoningEfforts":[{"reasoningEffort":"max"}]}]}),
        );
        assert_eq!(
            efforts,
            vec![("small".into(), vec!["low".into(), "high".into()])]
        );
        let mut c = config(Provider::Codex);
        c.effort = Some("high".into());
        c.access = AccessMode::ReadOnly;
        let wire = codex_turn(&c, "thread");
        assert_eq!(wire["params"]["effort"], "high");
        assert_eq!(
            wire["params"]["sandboxPolicy"],
            json!({"type":"readOnly","networkAccess":false})
        );
        assert_eq!(wire["params"]["approvalPolicy"], "untrusted");
    }
    #[test]
    fn claude_chat_only_disables_builtins_mcp_and_customizations_without_bare() {
        let mut c = config(Provider::Claude);
        c.access = AccessMode::ChatOnly;
        let args = claude_args(&c);
        for (flag, value) in [
            ("--tools", ""),
            ("--disallowedTools", "*"),
            ("--mcp-config", "{\"mcpServers\":{}}"),
            ("--permission-mode", "dontAsk"),
            ("--setting-sources", ""),
        ] {
            let i = args.iter().position(|a| a == flag).unwrap();
            assert_eq!(args[i + 1], value);
        }
        assert!(args.contains(&"--safe-mode".to_string()));
        assert!(!args.contains(&"--bare".to_string()));
        assert!(!args.contains(&c.prompt));
        c.provider = Provider::Codex;
        assert!(validate(&c).unwrap_err().contains("no verified"));
        c.provider = Provider::Claude;
        c.remote_id = Some("/private/history.jsonl".into());
        assert!(validate(&c).is_err());
    }
    #[test]
    fn models_and_usage_are_real_protocol_fields() {
        assert_eq!(
            model_names(
                &json!({"data":[{"id":"x","model":"official-model"},{"model":"hidden","hidden":true}]})
            ),
            vec!["official-model"]
        );
        assert!(
            rate_usage(&json!({"rateLimits":{"primary":{"usedPercent":20,"resetsAt":100}}}))
                .contains("20%")
        );
    }

    #[cfg(unix)]
    fn mock_cli(mode: &str) -> (tempfile::TempDir, String) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mock-cli");
        let program = r#"#!/usr/bin/env python3
import json, sys, os
MODE = '__MODE__'
open('mock.pid','w').write(str(os.getpid()))
def out(value):
    data = json.dumps(value) + '\n'
    # Split writes to prove framing is independent of read boundaries.
    split = max(1, len(data)//2)
    sys.stdout.write(data[:split]); sys.stdout.flush()
    sys.stdout.write(data[split:]); sys.stdout.flush()
def response(i, v): out({'id':i,'result':v})
def event(method, params): out({'method':method,'params':params})
if sys.argv[1:] == ['auth','status']:
    print(json.dumps({'loggedIn':True,'authMethod':'api_key' if MODE == 'api' else 'claude.ai'}, indent=2))
    sys.exit(0)
for line in sys.stdin:
    v=json.loads(line)
    with open('wire.log','a') as f: f.write(json.dumps(v)+'\n')
    method=v.get('method')
    if MODE == 'malformed':
        print('{not valid json sk-ant-secret}',flush=True); sys.exit(0)
    if method == 'initialize': response(v['id'],{'userAgent':'mock'})
    elif method == 'account/read': response(v['id'],{'account':{'type':'apiKey' if MODE == 'api' else 'chatgpt','planType':'plus'}})
    elif method == 'model/list': response(v['id'],{'data':[{'model':'mock-model'}],'nextCursor':None})
    elif method == 'account/rateLimits/read': response(v['id'],{'rateLimits':{'primary':{'usedPercent':12}}})
    elif method in ('thread/start','thread/resume'):
        assert v['params']['approvalPolicy']=='untrusted'
        assert v['params']['sandbox']=='workspace-write'
        response(v['id'],{'thread':{'id':'thread-mock'}})
    elif method == 'turn/start':
        assert v['params']['sandboxPolicy']['networkAccess'] is False
        response(v['id'],{'turn':{'id':'turn-mock'}})
        event('turn/started',{'threadId':'thread-mock','turn':{'id':'turn-mock'}})
        event('item/agentMessage/delta',{'threadId':'thread-mock','itemId':'msg','delta':'Hello from mock'})
        if MODE not in ('cancel', 'cancel-stuck'):
            event('item/started',{'threadId':'thread-mock','item':{'type':'commandExecution','id':'cmd','command':'echo mock'}})
            event('item/completed',{'threadId':'thread-mock','item':{'type':'reasoning','id':'reason','summary':['Provider brief summary'],'content':['PRIVATE RAW REASONING']}})
            event('item/commandExecution/requestApproval',{'threadId':'thread-mock','turnId':'turn-mock','itemId':'cmd','command':'echo mock'}) if False else None
            out({'id':47,'method':'item/commandExecution/requestApproval','params':{'threadId':'thread-mock','turnId':'turn-mock','itemId':'cmd','command':'echo mock'}})
    elif method == 'turn/interrupt':
        open('interrupted','w').write('yes')
        if MODE == 'cancel-stuck': continue
        response(v['id'],{})
        event('turn/completed',{'threadId':'thread-mock','turn':{'id':'turn-mock','status':'interrupted'}})
    elif v.get('id') == 47:
        open('decision','w').write(v['result']['decision'])
        event('item/completed',{'threadId':'thread-mock','item':{'id':'msg','type':'agentMessage','text':'Hello from mock'}})
        event('item/completed',{'threadId':'thread-mock','item':{'type':'commandExecution','id':'cmd','command':'echo mock','aggregatedOutput':'actual tool output','exitCode':0}})
        event('turn/completed',{'threadId':'thread-mock','turn':{'id':'turn-mock','status':'completed'}})
    elif method == 'thread/list': response(v['id'],{'data':[{'id':'historic'}],'nextCursor':None})
    elif method == 'thread/read': response(v['id'],{'thread':{'id':'historic','name':'Saved test','turns':[{'items':[{'type':'userMessage','content':[{'type':'text','text':'Question'}]},{'type':'agentMessage','text':'Answer'}]}]}})
    elif v.get('type') == 'control_request':
        req=v['request']; rid=v['request_id']
        out({'type':'control_response','response':{'subtype':'success','request_id':rid,'response':{'models':[{'value':'mock-claude'}]}}})
        if req['subtype']=='interrupt': out({'type':'result','session_id':'claude-mock','is_error':False,'result':'','usage':{}})
    elif v.get('type') == 'user':
        out({'type':'system','subtype':'init','session_id':'claude-mock'})
        out({'type':'stream_event','parent_tool_use_id':None,'event':{'type':'message_start'}})
        out({'type':'stream_event','parent_tool_use_id':None,'event':{'delta':{'type':'text_delta','text':'Hello Claude'}}})
        out({'type':'assistant','parent_tool_use_id':None,'message':{'content':[{'type':'text','text':'Hello Claude'},{'type':'tool_use','id':'tool-1','name':'Bash','input':{'command':'echo mock'}},{'type':'thinking','thinking':'PRIVATE RAW REASONING'}]}})
        out({'type':'user','message':{'content':[{'type':'tool_result','tool_use_id':'tool-1','content':'actual tool output'}]}})
        out({'type':'control_request','request_id':'permission-1','request':{'subtype':'can_use_tool','tool_name':'Bash','input':{'command':'echo test'}}})
    elif v.get('type')=='control_response':
        open('decision','w').write(v['response']['response']['behavior'])
        out({'type':'result','session_id':'claude-mock','is_error':False,'result':'Hello Claude','usage':{'input_tokens':10,'output_tokens':3}})
"#.replace("__MODE__", mode);
        std::fs::write(&path, program).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        (dir, path.to_string_lossy().into_owned())
    }
    #[cfg(unix)]
    fn mock_config(provider: Provider, dir: &tempfile::TempDir, executable: String) -> RunConfig {
        RunConfig {
            executable,
            cwd: dir.path().to_string_lossy().into_owned(),
            ..config(provider)
        }
    }
    #[cfg(unix)]
    fn until_done(handle: &Handle, allow: bool) -> Vec<ProviderEvent> {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut events = Vec::new();
        loop {
            let ev = handle
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("worker should finish");
            if let ProviderEvent::Approval { id, .. } = &ev {
                handle
                    .commands
                    .try_send(ProviderCommand::Approve {
                        id: id.clone(),
                        allow,
                    })
                    .unwrap();
            }
            let done = matches!(ev, ProviderEvent::Done);
            events.push(ev);
            if done {
                break;
            }
        }
        events
    }
    #[cfg(unix)]
    #[test]
    fn actual_streams_produce_structured_timeline_without_hidden_reasoning() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        for provider in Provider::ALL {
            let (dir, exe) = mock_cli("normal");
            let handle = start(mock_config(provider, &dir, exe)).unwrap();
            let events = until_done(&handle, true);
            assert!(!events.iter().any(|e| matches!(e, ProviderEvent::Error(_))));
            assert!(events.iter().any(|e|matches!(e,ProviderEvent::Timeline(t) if t.kind==TimelineKind::ToolCall && t.detail.contains("echo mock"))));
            assert!(events.iter().any(|e|matches!(e,ProviderEvent::Timeline(t) if t.kind==TimelineKind::ToolResult && t.detail.contains("actual tool output"))));
            assert!(!format!("{events:?}").contains("PRIVATE RAW REASONING"));
            if provider == Provider::Codex {
                assert!(events.iter().any(|e|matches!(e,ProviderEvent::Timeline(t) if t.kind==TimelineKind::Summary && t.detail=="Provider brief summary")));
                assert!(events.iter().any(|e|matches!(e,ProviderEvent::UsageSnapshot(w) if w.len()==1 && w[0].used_percent==12.)));
            }
        }
    }
    #[test]
    fn expanded_codex_filesystem_scope_keeps_user_approvals() {
        let mut c = config(Provider::Codex);
        c.access = AccessMode::Full;
        let thread = codex_thread(&c);
        let turn = codex_turn(&c, "thread");
        assert_eq!(thread["params"]["sandbox"], "danger-full-access");
        assert_eq!(turn["params"]["sandboxPolicy"]["type"], "dangerFullAccess");
        assert_eq!(turn["params"]["approvalPolicy"], "untrusted");
        assert_eq!(turn["params"]["approvalsReviewer"], "user");
        c.provider = Provider::Claude;
        assert!(validate(&c).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn codex_stdio_handshake_approval_stream_and_cleanup() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, exe) = mock_cli("normal");
        let handle = start(mock_config(Provider::Codex, &dir, exe)).unwrap();
        let events = until_done(&handle, true);
        assert!(
            !events.iter().any(|e| matches!(e, ProviderEvent::Error(_))),
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|e| if let ProviderEvent::Text(s) = e {
                    Some(s.as_str())
                } else {
                    None
                })
                .collect::<String>(),
            "Hello from mock"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("decision")).unwrap(),
            "accept"
        );
        let wire = std::fs::read_to_string(dir.path().join("wire.log")).unwrap();
        assert!(!wire.contains("thread/list"));
        assert!(!wire.contains("thread/read"));
        assert!(
            events
                .iter()
                .any(|e| matches!(e,ProviderEvent::Started{remote_id} if remote_id=="thread-mock"))
        );
    }
    #[cfg(unix)]
    #[test]
    fn claude_stdio_auth_approval_and_deduplication() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, exe) = mock_cli("normal");
        let handle = start(mock_config(Provider::Claude, &dir, exe)).unwrap();
        let events = until_done(&handle, false);
        assert!(
            !events.iter().any(|e| matches!(e, ProviderEvent::Error(_))),
            "{events:?}"
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|e| if let ProviderEvent::Text(s) = e {
                    Some(s.as_str())
                } else {
                    None
                })
                .collect::<String>(),
            "Hello Claude"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("decision")).unwrap(),
            "deny"
        );
    }
    #[cfg(unix)]
    #[test]
    fn api_billing_modes_are_refused_before_prompt() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        for provider in [Provider::Codex, Provider::Claude] {
            let (dir, exe) = mock_cli("api");
            let handle = start(mock_config(provider, &dir, exe)).unwrap();
            let events = until_done(&handle, false);
            assert!(events.iter().any(|e| matches!(e, ProviderEvent::Error(_))));
            let wire = std::fs::read_to_string(dir.path().join("wire.log")).unwrap_or_default();
            assert!(!wire.contains("turn/start"));
            assert!(!wire.contains("\"type\": \"user\""));
        }
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_interrupts_only_its_owned_child() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (dir1, exe1) = mock_cli("cancel");
        let (dir2, exe2) = mock_cli("normal");
        let first = start(mock_config(Provider::Codex, &dir1, exe1)).unwrap();
        let second = start(mock_config(Provider::Codex, &dir2, exe2)).unwrap();
        loop {
            if matches!(
                first.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                ProviderEvent::Text(_)
            ) {
                break;
            }
        }
        first.cancel();
        let a = until_done(&first, false);
        assert!(
            !a.iter().any(|e| matches!(e, ProviderEvent::Error(_))),
            "{a:?}"
        );
        assert!(dir1.path().join("interrupted").exists());
        let b = until_done(&second, true);
        assert!(
            !b.iter().any(|e| matches!(e, ProviderEvent::Error(_))),
            "{b:?}"
        );
        assert_eq!(
            std::fs::read_to_string(dir2.path().join("decision")).unwrap(),
            "accept"
        );
    }
    #[cfg(unix)]
    #[test]
    fn explicit_import_reads_history_without_resuming_or_inference() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, exe) = mock_cli("normal");
        let backup = import_codex(exe.clone(), dir.path().to_string_lossy().into_owned()).unwrap();
        assert_eq!(backup.sessions.len(), 1);
        assert_eq!(backup.messages.len(), 2);
        let again = import_codex(exe, dir.path().join(".").to_string_lossy().into_owned()).unwrap();
        assert_eq!(backup.projects[0].id, again.projects[0].id);
        assert_eq!(backup.sessions[0].id, again.sessions[0].id);
        assert_eq!(backup.messages[0].id, again.messages[0].id);
        let mut store = crate::store::Store::open(Path::new(":memory:")).unwrap();
        let selected = vec![backup.sessions[0].id.clone()];
        assert_eq!(store.import(&backup, &selected).unwrap(), 1);
        assert_eq!(store.import(&again, &selected).unwrap(), 0);
        assert_eq!(store.backup().unwrap().messages.len(), 2);
        let wire = std::fs::read_to_string(dir.path().join("wire.log")).unwrap();
        assert!(wire.contains("thread/list"));
        assert!(wire.contains("thread/read"));
        assert!(!wire.contains("thread/resume"));
        assert!(!wire.contains("turn/start"));
    }
    #[cfg(unix)]
    #[test]
    fn malformed_output_fails_without_disclosing_payload() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, exe) = mock_cli("malformed");
        let handle = start(mock_config(Provider::Codex, &dir, exe)).unwrap();
        let events = until_done(&handle, false);
        assert!(events.iter().any(|e| matches!(e, ProviderEvent::Error(_))));
        assert!(!format!("{events:?}").contains("sk-ant-secret"));
    }
    #[cfg(unix)]
    #[test]
    fn unresponsive_child_is_killed_after_grace_period() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (dir, exe) = mock_cli("cancel-stuck");
        let handle = start(mock_config(Provider::Codex, &dir, exe)).unwrap();
        loop {
            if matches!(
                handle.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                ProviderEvent::Text(_)
            ) {
                break;
            }
        }
        let started = Instant::now();
        handle.cancel();
        let events = until_done(&handle, false);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(dir.path().join("interrupted").exists());
        assert!(!events.iter().any(|e| matches!(e, ProviderEvent::Error(_))));
    }

    #[cfg(unix)]
    #[test]
    fn account_probes_never_send_prompts_or_history_requests() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        for provider in [Provider::Codex, Provider::Claude] {
            let (dir, exe) = mock_cli("normal");
            let info = probe(provider, exe, dir.path().to_string_lossy().into_owned()).unwrap();
            assert!(!info.models.is_empty());
            assert!(info.status.contains("connected"));
            let wire = std::fs::read_to_string(dir.path().join("wire.log")).unwrap();
            assert!(!wire.contains("thread/list"));
            assert!(!wire.contains("thread/read"));
            assert!(!wire.contains("thread/start"));
            assert!(!wire.contains("turn/start"));
            assert!(!wire.contains("\"type\": \"user\""));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dropping_active_handle_reaps_only_its_child_before_returning() {
        let _guard = MOCK_IO.lock().unwrap_or_else(|e| e.into_inner());
        let (first_dir, first_exe) = mock_cli("cancel-stuck");
        let (second_dir, second_exe) = mock_cli("normal");
        let first = start(mock_config(Provider::Codex, &first_dir, first_exe)).unwrap();
        let second = start(mock_config(Provider::Codex, &second_dir, second_exe)).unwrap();
        loop {
            if matches!(
                first.events.recv_timeout(Duration::from_secs(5)).unwrap(),
                ProviderEvent::Text(_)
            ) {
                break;
            }
        }
        let pid = std::fs::read_to_string(first_dir.path().join("mock.pid")).unwrap();
        assert!(Path::new(&format!("/proc/{pid}")).exists());
        let started = Instant::now();
        drop(first);
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "owned child must be reaped before Drop returns"
        );
        let events = until_done(&second, true);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, ProviderEvent::Error(_)))
        );
        assert_eq!(
            std::fs::read_to_string(second_dir.path().join("decision")).unwrap(),
            "accept"
        );
    }
}
