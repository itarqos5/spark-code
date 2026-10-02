use iced::{
    Color, Element, Length, Subscription, Task, Theme,
    widget::{
        Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
    },
};
use spark_code::{engine::Engine, import, model::*, provider};
use std::{collections::HashSet, sync::mpsc, time::Duration};

pub fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("spark-code")
        .theme(|_: &App| {
            Theme::custom(
                "spark dark",
                iced::theme::Palette {
                    background: Color::from_rgb8(14, 18, 25),
                    text: Color::from_rgb8(231, 237, 245),
                    primary: Color::from_rgb8(254, 136, 96),
                    success: Color::from_rgb8(86, 203, 177),
                    warning: Color::from_rgb8(241, 195, 92),
                    danger: Color::from_rgb8(247, 100, 120),
                },
            )
        })
        .subscription(App::subscription)
        .window_size((1180., 780.))
        .run()
}
struct App {
    engine: Result<Engine, String>,
    selected: Option<String>,
    project: Option<String>,
    messages: Vec<Message>,
    prompt: String,
    search: String,
    settings: bool,
    importing: bool,
    preview: Option<Backup>,
    chosen: HashSet<String>,
    receiver: Option<mpsc::Receiver<Result<Backup, String>>>,
    notice: String,
    model: String,
}
#[derive(Debug, Clone)]
enum Msg {
    Tick,
    Prompt(String),
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
        let project = engine
            .as_ref()
            .ok()
            .and_then(|e| e.projects.first().map(|p| p.id.clone()));
        let model = engine
            .as_ref()
            .map(|e| e.settings.model.clone())
            .unwrap_or_default();
        Self {
            engine,
            selected: None,
            project,
            messages: Vec::new(),
            prompt: String::new(),
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
        if self.engine.as_ref().is_ok_and(|e| !e.jobs.is_empty()) || self.receiver.is_some() {
            iced::time::every(Duration::from_millis(80)).map(|_| Msg::Tick)
        } else {
            Subscription::none()
        }
    }
    fn load(&mut self) {
        if let (Ok(e), Some(id)) = (&self.engine, &self.selected) {
            self.messages = e.store.messages(id).unwrap_or_default();
            if let Some(s) = e.sessions.iter().find(|s| &s.id == id) {
                self.project = Some(s.project_id.clone());
                self.model = s.model.clone();
            }
        }
    }
    fn update(&mut self, msg: Msg) -> Task<Msg> {
        if let Msg::Copy(s) = msg {
            return iced::clipboard::write(s);
        }
        let Ok(e) = &mut self.engine else {
            return Task::none();
        };
        let result: Result<(), String> = (|| {
            match msg {
                Msg::Tick => {
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
                Msg::Prompt(s) => self.prompt = s,
                Msg::Search(s) => {
                    self.search = s;
                    e.refresh(&self.search)?;
                }
                Msg::Select(id) => {
                    self.selected = Some(id);
                    self.settings = false;
                }
                Msg::Project(id) => self.project = Some(id),
                Msg::AddProject => {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Choose a project folder")
                        .pick_folder()
                    {
                        self.project = Some(e.add_project(p.to_string_lossy().into_owned())?);
                    }
                }
                Msg::New => {
                    let p = self.project.clone().ok_or("Add a project folder first")?;
                    self.selected = Some(e.new_session(p)?);
                    self.settings = false;
                    self.prompt.clear();
                }
                Msg::Send => {
                    if self.selected.is_none() {
                        self.selected = Some(e.new_session(
                            self.project.clone().ok_or("Add a project folder first")?,
                        )?);
                    }
                    let id = self.selected.clone().unwrap();
                    e.send(&id, self.prompt.clone())?;
                    self.prompt.clear();
                }
                Msg::Settings => self.settings = !self.settings,
                Msg::Provider(p) => {
                    e.settings.provider = p;
                    e.settings.model.clear();
                    self.model.clear();
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
                    if let Some(id) = &self.selected {
                        if let Some(ses) = e.sessions.iter_mut().find(|s| &s.id == id) {
                            ses.model = s;
                            e.store.save_session(ses)?;
                        }
                    }
                }
                Msg::CodexPath(s) => e.settings.codex_path = s,
                Msg::ClaudePath(s) => e.settings.claude_path = s,
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
                Msg::Copy(_) => {}
            }
            Ok(())
        })();
        if let Err(err) = result {
            self.notice = err;
        }
        self.load();
        Task::none()
    }
    fn view(&self) -> Element<'_, Msg> {
        let e = match &self.engine {
            Ok(e) => e,
            Err(err) => {
                return container(column![
                    text("spark-code").size(32),
                    text("Could not open local storage"),
                    text(err)
                ])
                .padding(40)
                .into();
            }
        };
        let mut sidebar = column![
            row![
                text("✦").size(30).color(Color::from_rgb8(254, 136, 96)),
                text("spark-code").size(25)
            ]
            .spacing(10),
            text("YOUR CODING WORKSPACE")
                .size(10)
                .color(Color::from_rgb8(117, 137, 161)),
            Space::new().height(14),
            button(text("+  New conversation").size(14))
                .on_press(Msg::New)
                .padding(12)
                .width(Length::Fill),
            text_input("Search conversations", &self.search)
                .on_input(Msg::Search)
                .padding(10),
            Space::new().height(10),
            row![
                text("PROJECTS").size(11),
                Space::new().width(Length::Fill),
                button("+").on_press(Msg::AddProject)
            ],
        ]
        .spacing(10);
        for p in &e.projects {
            let label = format!(
                "{} {}",
                if self.project.as_ref() == Some(&p.id) {
                    "●"
                } else {
                    "○"
                },
                p.name
            );
            sidebar = sidebar.push(
                button(text(label).size(13))
                    .style(button::secondary)
                    .on_press(Msg::Project(p.id.clone()))
                    .width(Length::Fill)
                    .padding(10),
            );
        }
        if e.projects.is_empty() {
            sidebar = sidebar
                .push(text("Add a folder to start. Your files stay on your computer.").size(12));
        }
        sidebar = sidebar
            .push(Space::new().height(12))
            .push(text("CONVERSATIONS").size(11));
        let mut sessions = column![].spacing(5);
        for s in &e.sessions {
            let running = e.jobs.contains_key(&s.id);
            let label = format!("{} {}", if running { "◉" } else { "·" }, s.title);
            sessions = sessions.push(
                button(text(label).size(13))
                    .style(if self.selected.as_ref() == Some(&s.id) {
                        button::primary
                    } else {
                        button::text
                    })
                    .on_press(Msg::Select(s.id.clone()))
                    .width(Length::Fill)
                    .padding(9),
            );
        }
        sidebar = sidebar
            .push(scrollable(sessions).height(Length::Fill))
            .push(
                button("Settings & imports")
                    .style(button::secondary)
                    .on_press(Msg::Settings)
                    .padding(11)
                    .width(Length::Fill),
            )
            .push(
                text("LOCAL FIRST  /  NO BROWSER ENGINE")
                    .size(9)
                    .color(Color::from_rgb8(117, 137, 161)),
            );
        let center = if self.preview.is_some() {
            self.import_view()
        } else if self.settings {
            self.settings_view()
        } else {
            self.chat_view()
        };
        let mut right=column![text("AGENTS").size(11),text(format!("{} / {} active",e.jobs.len(),e.concurrency)).size(22),text("Separate projects can run together. Shared folders are locked while an agent works.").size(12),Space::new().height(10)].spacing(12);
        for (id, j) in &e.jobs {
            let title = e
                .sessions
                .iter()
                .find(|s| &s.id == id)
                .map(|s| s.title.as_str())
                .unwrap_or("Agent");
            let mut card = column![
                text(title).size(14),
                text(&j.status).size(12),
                button("Stop agent")
                    .style(button::danger)
                    .on_press(Msg::Cancel(id.clone()))
            ]
            .spacing(8);
            if let Some((_, desc)) = j.approval.front() {
                card = card.push(text(desc).size(12)).push(
                    row![
                        button("Allow once").on_press(Msg::Approve(id.clone(), true)),
                        button("Deny")
                            .style(button::secondary)
                            .on_press(Msg::Approve(id.clone(), false))
                    ]
                    .spacing(5),
                );
            }
            if !j.usage.is_empty() {
                card = card.push(text(&j.usage).size(11));
            }
            right = right.push(container(card).style(container::rounded_box).padding(12));
        }
        if e.jobs.is_empty() {
            right=right.push(container(column![text("Quiet by design").size(16),text("Provider processes launch only for work. No background account or history scans.").size(12)].spacing(8)).style(container::rounded_box).padding(14));
        }
        right = right
            .push(Space::new().height(Length::Fill))
            .push(text("SUBSCRIPTION CONNECTIONS").size(10))
            .push(text("Codex / ChatGPT\nClaude Code").size(13))
            .push(
                text(
                    "Usage appears only when the provider exposes it. Quotas cannot be reset here.",
                )
                .size(11),
            );
        container(
            row![
                container(sidebar)
                    .padding(22)
                    .width(260)
                    .height(Length::Fill)
                    .style(container::rounded_box),
                container(center)
                    .padding(28)
                    .width(Length::Fill)
                    .height(Length::Fill),
                container(right)
                    .padding(18)
                    .width(236)
                    .height(Length::Fill)
                    .style(container::rounded_box)
            ]
            .spacing(1),
        )
        .into()
    }
    fn chat_view(&self) -> Element<'_, Msg> {
        let e = self.engine.as_ref().unwrap();
        let ses = self
            .selected
            .as_ref()
            .and_then(|id| e.sessions.iter().find(|s| &s.id == id));
        let title = ses
            .map(|s| s.title.as_str())
            .unwrap_or("A little spark. A lot of possibility.");
        let p = self
            .project
            .as_ref()
            .and_then(|id| e.projects.iter().find(|p| &p.id == id));
        let provider = ses.map(|s| s.provider).unwrap_or(e.settings.provider);
        let mut content = column![
            text(title).size(26),
            text(
                p.map(|p| p.path.as_str())
                    .unwrap_or("Choose a project folder to begin")
            )
            .size(12)
            .color(Color::from_rgb8(127, 146, 167)),
            Space::new().height(12)
        ]
        .spacing(10);
        let mut transcript = column![].spacing(18);
        if self.messages.is_empty() {
            transcript=transcript.push(container(column![text("Build something worth opening.").size(23),text("Ask your coding agent to explore a project, fix a bug, or make something new. You review sensitive tool requests as they arrive.").size(15),Space::new().height(8),row![button("Add project folder").on_press(Msg::AddProject).padding(12),button("Connect a provider").style(button::secondary).on_press(Msg::Settings).padding(12)].spacing(12)].spacing(15)).padding(30).style(container::rounded_box));
        }
        for m in &self.messages {
            let label = match m.role.as_str() {
                "user" => "YOU",
                "assistant" => "AGENT",
                _ => "NOTICE",
            };
            transcript = transcript.push(
                column![
                    row![
                        text(label).size(10).color(Color::from_rgb8(86, 203, 177)),
                        Space::new().width(Length::Fill),
                        button("Copy")
                            .style(button::text)
                            .on_press(Msg::Copy(m.text.clone()))
                    ],
                    text(&m.text).size(14)
                ]
                .spacing(6),
            );
        }
        if let Some(j) = self.selected.as_ref().and_then(|id| e.jobs.get(id)) {
            if !j.draft.text.is_empty() {
                transcript = transcript.push(
                    column![
                        text("AGENT  ·  streaming")
                            .size(10)
                            .color(Color::from_rgb8(254, 136, 96)),
                        text(&j.draft.text).size(14)
                    ]
                    .spacing(6),
                );
            }
        }
        let running = self
            .selected
            .as_ref()
            .is_some_and(|id| e.jobs.contains_key(id));
        content = content
            .push(scrollable(transcript).height(Length::Fill))
            .push(
                row![
                    pick_list(Provider::ALL, Some(provider), Msg::Provider).padding(8),
                    text_input("Model (blank = provider default)", &self.model)
                        .on_input(Msg::Model)
                        .padding(9)
                ]
                .spacing(10),
            )
            .push(
                row![
                    text_input("What would you like to build?", &self.prompt)
                        .on_input(Msg::Prompt)
                        .on_submit(Msg::Send)
                        .padding(17),
                    button(if running { "Working…" } else { "Send ↑" })
                        .on_press_maybe((!running).then_some(Msg::Send))
                        .padding(17)
                ]
                .spacing(10),
            )
            .push(
                text(if !self.notice.is_empty() {
                    &self.notice
                } else {
                    &e.notice
                })
                .size(11),
            );
        content.into()
    }
    fn settings_view(&self) -> Element<'_, Msg> {
        let e = self.engine.as_ref().unwrap();
        scrollable(column![text("Settings").size(30),text("Your tools. Your accounts. Your machine.").size(14),Space::new().height(12),text("Provider connections").size(20),text("Install the official native CLI and sign in through its own login flow. spark-code does not store passwords, tokens or API keys. Existing subscription access is checked before a prompt is sent.").size(13),row![text("Codex executable").width(150),text_input("codex",&e.settings.codex_path).on_input(Msg::CodexPath).padding(10)].spacing(8),row![text("Claude executable").width(150),text_input("claude",&e.settings.claude_path).on_input(Msg::ClaudePath).padding(10)].spacing(8),row![button("Codex setup guide").style(button::secondary).on_press(Msg::OpenDocs(Provider::Codex)),button("Claude setup guide").style(button::secondary).on_press(Msg::OpenDocs(Provider::Claude))].spacing(10),text("In a terminal, run: codex login  ·  claude auth login\nNative .exe paths are supported on Windows. Shell scripts and .cmd wrappers are rejected for safe argument handling.").size(12),button("Save connection settings").on_press(Msg::SaveSettings).padding(12),Space::new().height(15),text("Agent capacity").size(20),row![text("Concurrent agents (1–4)").width(220),text_input("2",&e.concurrency.to_string()).on_input(Msg::Concurrency).padding(8).width(80)].spacing(10),text("More agents means more memory and subscription usage. One active agent per project folder prevents colliding edits.").size(12),Space::new().height(15),text("Import & export").size(20),text("Nothing is imported automatically. Preview and choose conversations before adding them. T3 imports preserve transcript text and project paths; they start fresh provider sessions. Codex imports use its official history API for the selected project.").size(13),row![button("Choose backup file…").on_press_maybe((!self.importing).then_some(Msg::ImportFile)).padding(11),button("Preview Codex history").on_press_maybe((!self.importing).then_some(Msg::ImportCodex)).style(button::secondary).padding(11)].spacing(10),button("Export spark-code history…").style(button::secondary).on_press(Msg::Export).padding(11),text(if self.importing{"Reading selected history…"}else{&self.notice}).size(12),Space::new().height(10),text("Privacy & limits").size(20),text("Local transcripts are unencrypted in your Windows user profile. Exported backups include messages and paths. Back them up privately. Claude Code on native Windows does not provide an OS-level sandbox; review tool requests carefully. spark-code is independent of OpenAI and Anthropic.").size(12)].spacing(14)).into()
    }
    fn import_view(&self) -> Element<'_, Msg> {
        let b = self.preview.as_ref().unwrap();
        let mut choices = column![].spacing(12);
        for s in &b.sessions {
            let project = b
                .projects
                .iter()
                .find(|p| p.id == s.project_id)
                .map(|p| p.name.as_str())
                .unwrap_or("Project");
            choices = choices.push(
                checkbox(self.chosen.contains(&s.id))
                    .label(format!("{}  /  {}", project, s.title))
                    .on_toggle({
                        let id = s.id.clone();
                        move |v| Msg::ToggleImport(id.clone(), v)
                    }),
            );
        }
        column![
            text("Review your import").size(28),
            text(format!(
                "{} projects · {} conversations · {} messages",
                b.projects.len(),
                b.sessions.len(),
                b.messages.len()
            ))
            .size(14),
            text("Only selected conversations will be added. Source data stays unchanged.")
                .size(12),
            scrollable(choices).height(Length::Fill),
            row![
                button(text(format!("Import {} selected", self.chosen.len())))
                    .on_press(Msg::ConfirmImport)
                    .padding(12),
                button("Cancel")
                    .style(button::secondary)
                    .on_press(Msg::CloseImport)
                    .padding(12)
            ]
            .spacing(10)
        ]
        .spacing(18)
        .into()
    }
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
