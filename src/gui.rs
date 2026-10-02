use crate::appearance::Colors;
use iced::{
    Color, Element, Length, Subscription, Task, Theme,
    widget::{Space, container, markdown, text_editor},
};
use spark_code::{
    engine::Engine,
    import,
    model::*,
    provider,
    provider_status::{AccessMode, Readiness},
};
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
    time::{Duration, Instant},
};
#[path = "ui.rs"]
mod ui;

pub fn run() -> iced::Result {
    set_app_id();
    iced::application(App::new, App::update, App::view)
        .title("Spark Code")
        .font(include_bytes!("../assets/fonts/InterVariable.ttf").as_slice())
        .default_font(iced::Font::with_name("Inter Variable"))
        .theme(App::theme)
        .subscription(App::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(1280., 840.),
            min_size: Some(iced::Size::new(920., 640.)),
            decorations: false,
            icon: load_icon(),
            ..Default::default()
        })
        .run()
}
fn load_icon() -> Option<iced::window::Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../assets/branding/spark-code.png"
    )));
    let mut reader = decoder.read_info().ok()?;
    let mut bytes = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut bytes).ok()?;
    bytes.truncate(info.buffer_size());
    iced::window::icon::from_rgba(bytes, info.width, info.height).ok()
}
fn set_app_id() {
    #[cfg(windows)]
    {
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(id: *const u16) -> i32;
        }
        let id: Vec<u16> = "SparkCode.Desktop".encode_utf16().chain(Some(0)).collect();
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
        }
    }
}
struct App {
    engine: Result<Engine, String>,
    selected: Option<String>,
    project: Option<String>,
    messages: Vec<Message>,
    prompt: String,
    editor: text_editor::Content,
    probe_receiver: Option<mpsc::Receiver<(Provider, Result<provider::ProviderInfo, String>)>>,
    search: String,
    settings: bool,
    importing: bool,
    preview: Option<Backup>,
    chosen: HashSet<String>,
    receiver: Option<mpsc::Receiver<Result<Backup, String>>>,
    notice: String,
    model: String,
    current_provider: Provider,
    effort: String,
    access: String,
    full_access_confirmation: bool,
    ime_composing: bool,
    last_ime_event: Option<Instant>,
    show_agents: bool,
    sidebar_visible: bool,
    rendered: HashMap<String, (usize, markdown::Content)>,
    theme_mix: f32,
    theme_from: f32,
    theme_target: f32,
    theme_started: Option<Instant>,
    motion_started: Option<Instant>,
    capture_started: Option<Instant>,
    capture_path: Option<String>,
}
#[derive(Debug, Clone)]
enum Msg {
    Tick,
    GeneralChat,
    ComposerSubmit,
    ImeUpdate(bool),
    ImeCommit,
    Effort(String),
    AccessRequested(String),
    ConfirmFullAccess,
    CancelFullAccess,
    Resize(iced::window::Direction),
    SystemMenu,
    RequestCapture,
    DragWindow,
    Minimize,
    Maximize,
    CloseWindow,
    ToggleTheme,
    ReducedMotion(bool),
    ToggleAgents,
    ToggleSidebar,
    OpenLink(String),
    Capture(iced::window::Screenshot),
    Edit(text_editor::Action),
    Probe(Provider),
    Login(Provider),
    UseHistory,
    Search(String),
    Select(String),
    Project(String),
    AddProject,
    New,
    Send,
    Settings,
    Provider(Provider),
    Model(String),
    CodexPath(String),
    ClaudePath(String),
    SaveSettings,
    Cancel(String),
    Approve(String, bool),
    ImportFile,
    ImportCodex,
    ToggleImport(String, bool),
    ConfirmImport,
    CloseImport,
    Export,
    Copy(String),
    OpenDocs(Provider),
    Concurrency(String),
}
impl App {
    fn new() -> Self {
        let engine = Engine::open();
        let project: Option<String> = None;
        let model = engine
            .as_ref()
            .map(|e| e.settings.model.clone())
            .unwrap_or_default();
        let current_provider = engine
            .as_ref()
            .map(|e| e.settings.provider)
            .unwrap_or_default();
        let theme_mix = if engine.as_ref().is_ok_and(|e| e.settings.light_theme) {
            1.
        } else {
            0.
        };
        let capture_path = std::env::var("SPARK_CODE_CAPTURE").ok();
        Self {
            current_provider,
            effort: String::new(),
            access: if project.is_some() {
                AccessMode::Workspace
            } else {
                AccessMode::ReadOnly
            }
            .to_string(),
            full_access_confirmation: false,
            ime_composing: false,
            last_ime_event: None,
            show_agents: false,
            sidebar_visible: true,
            rendered: HashMap::new(),
            theme_mix,
            theme_from: theme_mix,
            theme_target: theme_mix,
            theme_started: None,
            motion_started: None,
            capture_started: capture_path.as_ref().map(|_| Instant::now()),
            capture_path,
            engine,
            selected: None,
            project,
            messages: Vec::new(),
            prompt: String::new(),
            editor: text_editor::Content::new(),
            probe_receiver: None,
            search: String::new(),
            settings: false,
            importing: false,
            preview: None,
            chosen: HashSet::new(),
            receiver: None,
            notice: String::new(),
            model,
        }
    }
    fn subscription(&self) -> Subscription<Msg> {
        let timer = if self.engine.as_ref().is_ok_and(|e| !e.jobs.is_empty())
            || self.receiver.is_some()
            || self.probe_receiver.is_some()
            || self.theme_started.is_some()
            || self.motion_started.is_some()
            || self.capture_started.is_some()
        {
            iced::time::every(Duration::from_millis(
                if self.theme_started.is_some() || self.motion_started.is_some() {
                    16
                } else {
                    80
                },
            ))
            .map(|_| Msg::Tick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            timer,
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::InputMethod(iced::advanced::input_method::Event::Preedit(
                    value,
                    _,
                )) => Some(Msg::ImeUpdate(!value.is_empty())),
                iced::Event::InputMethod(iced::advanced::input_method::Event::Commit(_)) => {
                    Some(Msg::ImeCommit)
                }
                iced::Event::InputMethod(iced::advanced::input_method::Event::Closed) => {
                    Some(Msg::ImeUpdate(false))
                }
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) => {
                    if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                        Some(Msg::CancelFullAccess)
                    } else if modifiers.alt()
                        && key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Space)
                    {
                        Some(Msg::SystemMenu)
                    } else if modifiers.control()
                        && key == iced::keyboard::Key::Character("n".into())
                    {
                        Some(Msg::New)
                    } else if modifiers.control()
                        && key == iced::keyboard::Key::Character(",".into())
                    {
                        Some(Msg::Settings)
                    } else if modifiers.control()
                        && modifiers.shift()
                        && matches!(key.as_ref(), iced::keyboard::Key::Character("t" | "T"))
                    {
                        Some(Msg::ToggleTheme)
                    } else if modifiers.control()
                        && modifiers.shift()
                        && matches!(key.as_ref(), iced::keyboard::Key::Character("s" | "S"))
                    {
                        Some(Msg::RequestCapture)
                    } else {
                        None
                    }
                }
                _ => None,
            }),
        ])
    }
    fn load(&mut self) {
        if let (Ok(e), Some(id)) = (&self.engine, &self.selected) {
            self.messages = e.store.messages(id).unwrap_or_default();
            if let Some(s) = e.sessions.iter().find(|s| &s.id == id) {
                self.project = (!s.project_id.is_empty()).then(|| s.project_id.clone());
                self.current_provider = s.provider;
                self.model = s.model.clone();
            }
        }
    }
    fn update(&mut self, mut msg: Msg) -> Task<Msg> {
        if matches!(msg, Msg::ComposerSubmit) {
            if submission_blocked_by_ime(
                self.ime_composing,
                self.last_ime_event.map(|at| at.elapsed()),
            ) {
                return Task::none();
            }
            msg = Msg::Send;
        }
        if self.full_access_confirmation
            && !matches!(
                msg,
                Msg::ConfirmFullAccess
                    | Msg::CancelFullAccess
                    | Msg::Tick
                    | Msg::Capture(_)
                    | Msg::RequestCapture
                    | Msg::CloseWindow
                    | Msg::ImeUpdate(_)
                    | Msg::ImeCommit
            )
        {
            return Task::none();
        }
        if matches!(msg, Msg::Send) && !self.can_send() {
            self.notice = self.send_hint();
            return Task::none();
        }
        match &msg {
            Msg::Copy(s) => return iced::clipboard::write(s.clone()),
            Msg::Resize(direction) => {
                let direction = *direction;
                return iced::window::latest()
                    .and_then(move |id| iced::window::drag_resize(id, direction));
            }
            Msg::SystemMenu => {
                crate::window_chrome::show_system_menu();
                return Task::none();
            }
            Msg::RequestCapture => {
                return iced::window::latest()
                    .and_then(iced::window::screenshot)
                    .map(Msg::Capture);
            }
            Msg::DragWindow => return iced::window::latest().and_then(iced::window::drag),
            Msg::Minimize => {
                return iced::window::latest().and_then(|id| iced::window::minimize(id, true));
            }
            Msg::Maximize => return iced::window::latest().and_then(iced::window::toggle_maximize),
            Msg::CloseWindow => return iced::window::latest().and_then(iced::window::close),
            Msg::Capture(shot) => {
                if let Some(path) = &self.capture_path {
                    if let Err(err) = save_capture(
                        &capture_destination(
                            path,
                            self.engine.as_ref().is_ok_and(|e| e.settings.light_theme),
                            self.settings,
                            !self.messages.is_empty(),
                        ),
                        shot,
                    ) {
                        self.notice = format!("Preview capture failed: {err}");
                    }
                }
                return Task::none();
            }
            Msg::Tick
                if self
                    .capture_started
                    .is_some_and(|t| t.elapsed() > Duration::from_millis(900)) =>
            {
                self.capture_started = None;
                return iced::window::latest()
                    .and_then(iced::window::screenshot)
                    .map(Msg::Capture);
            }
            _ => {}
        }
        if matches!(msg, Msg::Settings | Msg::Select(_) | Msg::New)
            && self
                .engine
                .as_ref()
                .is_ok_and(|e| !e.settings.reduced_motion)
        {
            self.motion_started = Some(Instant::now());
        }
        let Ok(e) = &mut self.engine else {
            return Task::none();
        };
        let result: Result<(), String> = (|| {
            match msg {
                Msg::Tick => {
                    if let Some(start) = self.theme_started {
                        let t = (start.elapsed().as_secs_f32() / 0.2).min(1.);
                        let eased = t * t * (3. - 2. * t);
                        self.theme_mix =
                            self.theme_from + (self.theme_target - self.theme_from) * eased;
                        if t >= 1. {
                            self.theme_started = None;
                        }
                    }
                    if self
                        .motion_started
                        .is_some_and(|t| t.elapsed() > Duration::from_millis(180))
                    {
                        self.motion_started = None;
                    }
                    if let Some(rx) = &self.probe_receiver {
                        if let Ok((source_provider, result)) = rx.try_recv() {
                            self.probe_receiver = None;
                            match result {
                                Ok(info) => {
                                    self.notice = format!("{} · {}", info.status, info.usage);
                                    let first = info.models.first().cloned();
                                    e.probe_succeeded(source_provider, info);
                                    if source_provider == self.current_provider
                                        && self.model.is_empty()
                                    {
                                        if let Some(model) = first {
                                            self.model = model.clone();
                                            e.settings.model = model;
                                            e.store.save_settings(&e.settings)?;
                                        }
                                    }
                                }
                                Err(err) => {
                                    e.provider_failed(source_provider, &err);
                                    self.notice = err;
                                }
                            }
                        }
                    }
                    if e.poll() {
                        self.messages = self
                            .selected
                            .as_ref()
                            .map(|id| e.store.messages(id).unwrap_or_default())
                            .unwrap_or_default();
                    }
                    if let Some(rx) = &self.receiver {
                        if let Ok(r) = rx.try_recv() {
                            self.importing = false;
                            self.receiver = None;
                            match r {
                                Ok(b) => {
                                    self.chosen = b.sessions.iter().map(|s| s.id.clone()).collect();
                                    self.preview = Some(b);
                                }
                                Err(err) => self.notice = err,
                            }
                        }
                    }
                }
                Msg::Edit(action) => {
                    self.editor.perform(action);
                    self.prompt = self.editor.text();
                }
                Msg::Probe(p) => {
                    if self.probe_receiver.is_none() {
                        let exe = match p {
                            Provider::Codex => e.settings.codex_path.clone(),
                            Provider::Claude => e.settings.claude_path.clone(),
                        };
                        let cwd = self
                            .project
                            .as_ref()
                            .and_then(|id| e.projects.iter().find(|p| &p.id == id))
                            .map(|p| p.path.clone())
                            .unwrap_or_else(|| ".".into());
                        let (tx, rx) = mpsc::channel();
                        self.probe_receiver = Some(rx);
                        e.providers.get_mut(p).readiness = Readiness::Checking;
                        self.notice = "Checking official CLI connection…".into();
                        std::thread::spawn(move || {
                            let _ = tx.send((p, provider::probe(p, exe, cwd)));
                        });
                    }
                }
                Msg::Login(p) => {
                    let exe = match p {
                        Provider::Codex => e.settings.codex_path.clone(),
                        Provider::Claude => e.settings.claude_path.clone(),
                    };
                    provider::launch_login(p, exe)?;
                    e.providers.get_mut(p).readiness = Readiness::Unchecked;
                    self.notice="Complete sign-in in the official CLI window, then click Refresh. No account history is imported.".into();
                }
                Msg::UseHistory => {
                    let mut context =
                        String::from("Selected imported conversation context (reference only):\n");
                    for m in self
                        .messages
                        .iter()
                        .rev()
                        .take(8)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                    {
                        let remaining = 16000usize.saturating_sub(context.len());
                        if remaining == 0 {
                            break;
                        }
                        let part: String = m.text.chars().take(remaining / 4).collect();
                        context.push_str(&format!("\n{}: {}\n", m.role, part));
                    }
                    context.push_str("\nMy request:\n");
                    self.prompt = context;
                    self.editor = text_editor::Content::with_text(&self.prompt);
                    self.notice="Recent imported text added to your draft. Review it before sending to the selected provider.".into();
                }
                Msg::Search(s) => {
                    self.search = s;
                    e.refresh(&self.search)?;
                }
                Msg::Select(id) => {
                    if let Some(session) = e.sessions.iter().find(|s| s.id == id) {
                        self.current_provider = session.provider;
                        self.project =
                            (!session.project_id.is_empty()).then(|| session.project_id.clone());
                        self.model = session.model.clone();
                    }
                    self.selected = Some(id);
                    self.settings = false;
                }
                Msg::Project(id) => {
                    self.project = Some(id);
                    self.access = AccessMode::Workspace.to_string();
                    self.selected = None;
                    self.messages.clear();
                }
                Msg::AddProject => {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Choose a project folder")
                        .pick_folder()
                    {
                        self.project = Some(e.add_project(p.to_string_lossy().into_owned())?);
                        self.selected = None;
                        self.messages.clear();
                        self.access = AccessMode::Workspace.to_string();
                    }
                }
                Msg::New => {
                    e.settings.provider = self.current_provider;
                    e.settings.model = self.model.clone();
                    self.selected = Some(if let Some(p) = self.project.clone() {
                        e.new_session(p)?
                    } else {
                        e.new_general_chat()?
                    });
                    self.settings = false;
                    self.prompt.clear();
                    self.editor = text_editor::Content::new();
                }
                Msg::Send => {
                    if self.selected.is_none() {
                        e.settings.provider = self.current_provider;
                        e.settings.model = self.model.clone();
                        self.selected = Some(if let Some(p) = self.project.clone() {
                            e.new_session(p)?
                        } else {
                            e.new_general_chat()?
                        });
                    }
                    let id = self.selected.clone().unwrap();
                    let mode = parse_access(&self.access)
                        .filter(|mode| self.project.is_some() || *mode != AccessMode::Workspace)
                        .unwrap_or(if self.project.is_none() {
                            AccessMode::ReadOnly
                        } else {
                            AccessMode::Workspace
                        });
                    e.send_with_options(
                        &id,
                        self.prompt.clone(),
                        (!self.effort.is_empty()).then(|| self.effort.clone()),
                        mode,
                    )?;
                    self.prompt.clear();
                    self.editor = text_editor::Content::new();
                }
                Msg::GeneralChat => {
                    self.project = None;
                    self.selected = None;
                    self.messages.clear();
                    self.settings = false;
                    self.access = AccessMode::ReadOnly.to_string();
                }
                Msg::ImeUpdate(composing) => {
                    self.ime_composing = composing;
                    self.last_ime_event = Some(Instant::now());
                }
                Msg::ImeCommit => {
                    self.ime_composing = false;
                    self.last_ime_event = Some(Instant::now());
                }
                Msg::Effort(value) => {
                    if value.is_empty()
                        || e.providers
                            .get(self.current_provider)
                            .efforts_for(&self.model)
                            .contains(&value)
                    {
                        self.effort = value;
                    }
                }
                Msg::AccessRequested(value) => {
                    let mode = parse_access(&value).ok_or("Unsupported access setting")?;
                    if !e
                        .providers
                        .get(self.current_provider)
                        .supported_access
                        .contains(&mode)
                    {
                        return Err("This provider does not expose that access mode".into());
                    }
                    if mode == AccessMode::Full {
                        self.full_access_confirmation = true;
                    } else {
                        self.access = value;
                    }
                }
                Msg::ConfirmFullAccess => {
                    if self.full_access_confirmation {
                        self.access = AccessMode::Full.to_string();
                        self.full_access_confirmation = false;
                    }
                }
                Msg::CancelFullAccess => self.full_access_confirmation = false,
                Msg::Settings => self.settings = !self.settings,
                Msg::ToggleTheme => {
                    e.settings.light_theme = !e.settings.light_theme;
                    self.theme_from = self.theme_mix;
                    self.theme_target = if e.settings.light_theme { 1. } else { 0. };
                    if e.settings.reduced_motion {
                        self.theme_mix = self.theme_target;
                    } else {
                        self.theme_started = Some(Instant::now());
                    }
                    e.store.save_settings(&e.settings)?;
                }
                Msg::ReducedMotion(value) => {
                    e.settings.reduced_motion = value;
                    e.store.save_settings(&e.settings)?;
                    if value {
                        self.theme_started = None;
                        self.motion_started = None;
                        self.theme_mix = self.theme_target;
                    }
                }
                Msg::ToggleAgents => self.show_agents = !self.show_agents,
                Msg::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
                Msg::OpenLink(url) => {
                    if url.starts_with("https://") || url.starts_with("http://") {
                        open_url(&url)?;
                    } else {
                        self.notice = "Only web links can be opened from chat".into();
                    }
                }
                Msg::Provider(p) => {
                    if self
                        .selected
                        .as_ref()
                        .is_some_and(|id| e.jobs.contains_key(id))
                    {
                        return Err("Stop this turn before switching provider".into());
                    }
                    self.current_provider = p;
                    self.effort.clear();
                    self.access = if self.project.is_some() {
                        AccessMode::Workspace
                    } else {
                        AccessMode::ReadOnly
                    }
                    .to_string();
                    e.settings.provider = p;
                    e.settings.model.clear();
                    self.model = e.models.get(p).first().cloned().unwrap_or_default();
                    e.settings.model = self.model.clone();
                    e.store.save_settings(&e.settings)?;
                    if let Some(id) = &self.selected {
                        if e.jobs.contains_key(id) {
                            return Err("Stop this turn before switching provider".into());
                        }
                        if let Some(s) = e.sessions.iter_mut().find(|s| &s.id == id) {
                            if s.provider != p {
                                self.selected = None;
                                self.notice="Provider changed. Your next message starts a separate conversation.".into();
                            }
                        }
                    }
                }
                Msg::Model(s) => {
                    self.model = s.clone();
                    e.settings.model = s.clone();
                    e.store.save_settings(&e.settings)?;
                    if let Some(id) = &self.selected {
                        if let Some(ses) = e.sessions.iter_mut().find(|s| &s.id == id) {
                            ses.model = s;
                            e.store.save_session(ses)?;
                        }
                    }
                }
                Msg::CodexPath(s) => {
                    e.settings.codex_path = s;
                    e.providers.get_mut(Provider::Codex).readiness = Readiness::Unchecked;
                }
                Msg::ClaudePath(s) => {
                    e.settings.claude_path = s;
                    e.providers.get_mut(Provider::Claude).readiness = Readiness::Unchecked;
                }
                Msg::SaveSettings => {
                    e.store.save_settings(&e.settings)?;
                    self.notice="Settings saved. Sign in through the official CLI before your first message.".into();
                }
                Msg::Concurrency(s) => {
                    e.concurrency = s.parse::<usize>().unwrap_or(2).clamp(1, 4);
                }
                Msg::Cancel(id) => e.cancel(&id),
                Msg::Approve(id, a) => e.approve(&id, a),
                Msg::ImportFile => {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Choose a spark-code export or consistent T3 Code backup")
                        .add_filter("Supported backups", &["json", "zip", "sqlite", "db"])
                        .pick_file()
                    {
                        let (tx, rx) = mpsc::channel();
                        self.receiver = Some(rx);
                        self.importing = true;
                        std::thread::spawn(move || {
                            let _ = tx.send(import::preview(&p));
                        });
                    }
                }
                Msg::ImportCodex => {
                    let cwd = self
                        .project
                        .as_ref()
                        .and_then(|id| e.projects.iter().find(|p| &p.id == id))
                        .map(|p| p.path.clone())
                        .ok_or("Select a project folder before listing its Codex conversations")?;
                    let exe = e.settings.codex_path.clone();
                    let (tx, rx) = mpsc::channel();
                    self.receiver = Some(rx);
                    self.importing = true;
                    std::thread::spawn(move || {
                        let _ = tx.send(provider::import_codex(exe, cwd));
                    });
                }
                Msg::ToggleImport(id, on) => {
                    if on {
                        self.chosen.insert(id);
                    } else {
                        self.chosen.remove(&id);
                    }
                }
                Msg::ConfirmImport => {
                    if let Some(b) = &self.preview {
                        let n = e
                            .store
                            .import(b, &self.chosen.iter().cloned().collect::<Vec<_>>())?;
                        self.notice = format!(
                            "Imported {n} conversations. Source files were left unchanged."
                        );
                        e.refresh(&self.search)?;
                        self.preview = None;
                    }
                }
                Msg::CloseImport => {
                    self.preview = None;
                    self.chosen.clear();
                }
                Msg::Export => {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Export local spark-code history")
                        .set_file_name("spark-code.json")
                        .save_file()
                    {
                        import::export(&p, &e.store.backup()?)?;
                        self.notice="Export saved. It contains local messages and project paths, but no stored account credentials.".into();
                    }
                }
                Msg::OpenDocs(p) => {
                    let url = match p {
                        Provider::Codex => "https://developers.openai.com/codex/cli/",
                        Provider::Claude => "https://code.claude.com/docs/en/setup",
                    };
                    open_url(url)?;
                }
                Msg::Copy(_)
                | Msg::DragWindow
                | Msg::Minimize
                | Msg::Maximize
                | Msg::CloseWindow
                | Msg::Capture(_)
                | Msg::Resize(_)
                | Msg::SystemMenu
                | Msg::RequestCapture
                | Msg::ComposerSubmit => {}
            }
            Ok(())
        })();
        if let Err(err) = result {
            self.notice = err;
        }
        self.load();
        self.cache_markdown();
        Task::none()
    }
    fn view(&self) -> Element<'_, Msg> {
        with_resize_edges(ui::view(self))
    }
    fn selected_provider(&self) -> Provider {
        self.current_provider
    }
    fn model_name(&self) -> String {
        if self.model.is_empty() {
            "Choose model".into()
        } else {
            self.model.clone()
        }
    }
    fn provider_ready(&self, p: Provider) -> bool {
        self.engine
            .as_ref()
            .is_ok_and(|e| e.providers.get(p).ready())
    }
    fn provider_hint(&self, p: Provider) -> String {
        let Ok(e) = &self.engine else {
            return "Local storage unavailable".into();
        };
        let state = e.providers.get(p);
        match state.readiness{Readiness::Ready=>"Connected with your subscription".into(),Readiness::Checking=>"Checking the official CLI connection…".into(),Readiness::Unchecked=>"Open Settings and Refresh this provider. Install its official CLI or sign in if needed.".into(),_=>format!("{} Open Settings to install, sign in, or troubleshoot, then Refresh.",state.status)}
    }
    fn usage_fraction(&self, p: Provider) -> Option<f32> {
        self.engine
            .as_ref()
            .ok()?
            .providers
            .get(p)
            .usage_windows
            .first()
            .map(|w| (w.used_percent / 100.).clamp(0., 1.))
    }
    fn usage_label(&self, p: Provider) -> String {
        let Ok(e) = &self.engine else {
            return "Unavailable".into();
        };
        let s = e.providers.get(p);
        if s.usage_windows.is_empty() {
            if s.checked_at == 0 {
                return s.readiness.to_string();
            }
            return if s.usage_note.is_empty() {
                "Usage unavailable".into()
            } else {
                s.usage_note.clone()
            };
        }
        let labels = s
            .usage_windows
            .iter()
            .map(|w| {
                let reset = w
                    .resets_at
                    .map(|at| format!(" · {}", reset_label(at)))
                    .unwrap_or_default();
                format!("{}: {:.0}% used{}", w.label, w.used_percent, reset)
            })
            .collect::<Vec<_>>()
            .join(" · ");
        if s.is_stale() {
            format!("{labels} · last known")
        } else {
            labels
        }
    }
    fn supported_efforts(&self) -> Vec<String> {
        self.engine
            .as_ref()
            .map(|e| {
                e.providers
                    .get(self.current_provider)
                    .efforts_for(&self.model)
                    .to_vec()
            })
            .unwrap_or_default()
    }
    fn access_choices(&self) -> Vec<String> {
        self.engine
            .as_ref()
            .map(|e| {
                e.providers
                    .get(self.current_provider)
                    .supported_access
                    .iter()
                    .filter(|m| self.project.is_some() || **m != AccessMode::Workspace)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
    fn send_hint(&self) -> String {
        if !self.provider_ready(self.current_provider) {
            return self.provider_hint(self.current_provider);
        }
        let Ok(e) = &self.engine else {
            return "Storage unavailable".into();
        };
        if self.model.is_empty() || !e.models.get(self.current_provider).contains(&self.model) {
            return "Choose an available model after refreshing the provider in Settings".into();
        }
        if self.ime_composing {
            return "Finish composing your text before sending".into();
        }
        if self.prompt.trim().is_empty() {
            return "Write a message first".into();
        }
        String::new()
    }
    fn can_send(&self) -> bool {
        self.send_hint().is_empty()
            && !self.full_access_confirmation
            && self
                .selected
                .as_ref()
                .is_none_or(|id| self.engine.as_ref().is_ok_and(|e| !e.jobs.contains_key(id)))
    }

    fn colors(&self) -> Colors {
        Colors::at(self.theme_mix)
    }
    fn theme(&self) -> Theme {
        let c = self.colors();
        Theme::custom(
            "Spark",
            iced::theme::Palette {
                background: c.bg,
                text: c.text,
                primary: c.text,
                success: Color::from_rgb8(78, 174, 126),
                warning: Color::from_rgb8(227, 177, 85),
                danger: Color::from_rgb8(219, 86, 94),
            },
        )
    }
    fn motion_progress(&self) -> f32 {
        self.motion_started
            .map(|t| (t.elapsed().as_secs_f32() / 0.18).min(1.))
            .unwrap_or(1.)
    }
    fn cache_markdown(&mut self) {
        for m in &self.messages {
            if self
                .rendered
                .get(&m.id)
                .is_none_or(|(len, _)| *len != m.text.len())
            {
                self.rendered.insert(
                    m.id.clone(),
                    (m.text.len(), markdown::Content::parse(&m.text)),
                );
            }
        }
        if let Some(j) = self
            .engine
            .as_ref()
            .ok()
            .and_then(|e| self.selected.as_ref().and_then(|id| e.jobs.get(id)))
        {
            let entry = self
                .rendered
                .entry(j.draft.id.clone())
                .or_insert_with(|| (0, markdown::Content::default()));
            if entry.0 < j.draft.text.len() {
                entry.1.push_str(&j.draft.text[entry.0..]);
                entry.0 = j.draft.text.len();
            }
        }
        let mut retain: HashSet<String> = self.messages.iter().map(|m| m.id.clone()).collect();
        if let Some(j) = self
            .engine
            .as_ref()
            .ok()
            .and_then(|e| self.selected.as_ref().and_then(|id| e.jobs.get(id)))
        {
            retain.insert(j.draft.id.clone());
        }
        self.rendered.retain(|id, _| retain.contains(id));
    }
}
fn save_capture(path: &str, shot: &iced::window::Screenshot) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        shot.size.width,
        shot.size.height,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&shot.rgba)
        .map_err(|e| e.to_string())
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut c = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut c = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    c.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_access(value: &str) -> Option<AccessMode> {
    [
        AccessMode::ChatOnly,
        AccessMode::ReadOnly,
        AccessMode::Workspace,
        AccessMode::Full,
    ]
    .into_iter()
    .find(|m| m.to_string() == value)
}
fn reset_label(timestamp: i64) -> String {
    let left = timestamp.saturating_sub(now());
    if left <= 0 {
        "reset due · Refresh".into()
    } else if left < 3600 {
        format!("resets in {}m", (left + 59) / 60)
    } else {
        format!("resets in {}h {}m", left / 3600, (left % 3600) / 60)
    }
}
fn capture_destination(path: &str, light: bool, settings: bool, chat: bool) -> String {
    let mut p = path.to_string();
    if light {
        p = p.replace("-dark-", "-light-");
    }
    if settings {
        p = p.replace(".png", "-settings.png");
    } else if chat {
        p = p.replace(".png", "-chat.png");
    }
    p
}
fn composer_binding(event: text_editor::KeyPress) -> Option<text_editor::Binding<Msg>> {
    use iced::keyboard::{Key, key::Named};
    if !matches!(event.status, text_editor::Status::Focused { .. }) {
        return None;
    }
    if event.key == Key::Named(Named::Enter) && !event.modifiers.shift() && !event.modifiers.alt() {
        Some(text_editor::Binding::Custom(Msg::ComposerSubmit))
    } else {
        text_editor::Binding::from_key_press(event)
    }
}
fn with_resize_edges(main: Element<'_, Msg>) -> Element<'_, Msg> {
    use iced::widget::{mouse_area, stack};
    use iced::{Alignment, mouse::Interaction, window::Direction};
    let edge = |direction, cursor, w: Length, h: Length, x, y| {
        container(
            mouse_area(Space::new().width(w).height(h))
                .interaction(cursor)
                .on_press(Msg::Resize(direction)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(x)
        .align_y(y)
    };
    stack![
        main,
        edge(
            Direction::North,
            Interaction::ResizingVertically,
            Length::Fill,
            4.into(),
            Alignment::Start,
            Alignment::Start
        ),
        edge(
            Direction::South,
            Interaction::ResizingVertically,
            Length::Fill,
            4.into(),
            Alignment::Start,
            Alignment::End
        ),
        edge(
            Direction::West,
            Interaction::ResizingHorizontally,
            4.into(),
            Length::Fill,
            Alignment::Start,
            Alignment::Start
        ),
        edge(
            Direction::East,
            Interaction::ResizingHorizontally,
            4.into(),
            Length::Fill,
            Alignment::End,
            Alignment::Start
        ),
        edge(
            Direction::NorthWest,
            Interaction::ResizingDiagonallyDown,
            8.into(),
            8.into(),
            Alignment::Start,
            Alignment::Start
        ),
        edge(
            Direction::NorthEast,
            Interaction::ResizingDiagonallyUp,
            8.into(),
            8.into(),
            Alignment::End,
            Alignment::Start
        ),
        edge(
            Direction::SouthWest,
            Interaction::ResizingDiagonallyUp,
            8.into(),
            8.into(),
            Alignment::Start,
            Alignment::End
        ),
        edge(
            Direction::SouthEast,
            Interaction::ResizingDiagonallyDown,
            8.into(),
            8.into(),
            Alignment::End,
            Alignment::End
        ),
    ]
    .into()
}
#[cfg(test)]
mod composer_tests {
    use super::*;
    fn key(shift: bool, focused: bool) -> text_editor::KeyPress {
        text_editor::KeyPress {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
            modifiers: if shift {
                iced::keyboard::Modifiers::SHIFT
            } else {
                iced::keyboard::Modifiers::empty()
            },
            text: None,
            status: if focused {
                text_editor::Status::Focused { is_hovered: true }
            } else {
                text_editor::Status::Active
            },
        }
    }
    #[test]
    fn enter_submits_only_focused_composer() {
        assert!(matches!(
            composer_binding(key(false, true)),
            Some(text_editor::Binding::Custom(Msg::ComposerSubmit))
        ));
        assert!(composer_binding(key(false, false)).is_none());
    }
    #[test]
    fn shift_enter_remains_newline() {
        assert!(matches!(
            composer_binding(key(true, true)),
            Some(text_editor::Binding::Enter)
        ));
    }
    #[test]
    fn access_parser_rejects_unadvertised_strings() {
        assert!(parse_access("bypass-permissions").is_none());
    }
}
fn submission_blocked_by_ime(composing: bool, last_event_age: Option<Duration>) -> bool {
    composing || last_event_age.is_some_and(|age| age < Duration::from_millis(300))
}
#[cfg(test)]
mod ime_tests {
    use super::*;
    #[test]
    fn composition_confirmation_cannot_send() {
        assert!(submission_blocked_by_ime(true, None));
        assert!(submission_blocked_by_ime(
            false,
            Some(Duration::from_millis(5))
        ));
        assert!(!submission_blocked_by_ime(
            false,
            Some(Duration::from_millis(400))
        ));
        assert!(!submission_blocked_by_ime(false, None));
    }
}
