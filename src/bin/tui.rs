use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{prelude::*, widgets::*};
use spark_code::{engine::Engine, import, model::*, provider};
use std::{io, path::Path, sync::mpsc, time::Duration};
#[derive(PartialEq)]
enum Mode {
    Chat,
    Projects,
    Sessions,
    Providers,
    Model,
    AddProject,
    Help,
    Settings,
    Import,
    ImportReview,
}
struct Tui {
    engine: Engine,
    id: Option<String>,
    project: Option<String>,
    messages: Vec<Message>,
    input: String,
    mode: Mode,
    index: usize,
    notice: String,
    preview: Option<Backup>,
    selected: Vec<String>,
    probe_receiver: Option<mpsc::Receiver<(Provider, Result<provider::ProviderInfo, String>)>>,
}
fn main() {
    if let Err(e) = run() {
        eprintln!("spark-code: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--help" | "-h") => {
            println!(
                "spark-code {}\n\nUsage: spark-code [gui | doctor | export FILE]\n\nWithout arguments: native terminal workspace\nF1 help · F2 sessions · F3 projects · F4 provider · F5 model\nF7 official login · F8 refresh account/models\nCtrl+N new chat · Enter send · Esc cancel/back · Ctrl+Q quit\n\nData: {}",
                env!("CARGO_PKG_VERSION"),
                spark_code::store::data_dir().display()
            );
            return Ok(());
        }
        Some("gui") => {
            let name = if cfg!(windows) {
                "spark-code-desktop.exe"
            } else {
                "spark-code-desktop"
            };
            let exe = std::env::current_exe()?.with_file_name(name);
            std::process::Command::new(exe).spawn()?;
            return Ok(());
        }
        Some("doctor") => {
            let e = Engine::open()?;
            println!(
                "spark-code {}\nData: {}\nCodex executable: {}\nClaude executable: {}\nNo account/history discovery was run.\nUse official CLI login, then send a prompt to verify subscription access.",
                env!("CARGO_PKG_VERSION"),
                spark_code::store::data_dir().display(),
                e.settings.codex_path,
                e.settings.claude_path
            );
            return Ok(());
        }
        Some("export") => {
            let p = args.get(2).ok_or("Provide an output JSON path")?;
            let e = Engine::open()?;
            import::export(Path::new(p), &e.store.backup()?)?;
            println!("Exported to {p}. Keep it private: it contains messages and project paths.");
            return Ok(());
        }
        Some(s) => return Err(format!("Unknown command {s}. Run spark-code --help").into()),
        None => {}
    }
    let engine = Engine::open()?;
    let project = engine.projects.first().map(|p| p.id.clone());
    let mut app = Tui {
        engine,
        id: None,
        project,
        messages: vec![],
        input: String::new(),
        mode: Mode::Chat,
        index: 0,
        notice: String::new(),
        preview: None,
        selected: vec![],
        probe_receiver: None,
    };
    enable_raw_mode()?;
    let _restore = TerminalRestore;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = loop {
        terminal.draw(|f| draw(f, &app))?;
        if event::poll(Duration::from_millis(if app.engine.jobs.is_empty() {
            500
        } else {
            80
        }))? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('q') {
                    break Ok(());
                }
                if let Err(e) = key(&mut app, k) {
                    app.notice = e;
                }
            }
        }
        if let Some(rx) = &app.probe_receiver {
            if let Ok((source_provider, result)) = rx.try_recv() {
                app.probe_receiver = None;
                match result {
                    Ok(info) => {
                        app.notice = format!("{} · {}", info.status, info.usage);
                        app.engine.probe_succeeded(source_provider, info);
                    }
                    Err(e) => {
                        app.engine.provider_failed(source_provider, &e);
                        app.notice = e;
                    }
                }
            }
        }
        if app.engine.poll() {
            reload(&mut app);
        }
    };
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}
struct TerminalRestore;
impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
fn reload(a: &mut Tui) {
    a.messages =
        a.id.as_ref()
            .map(|id| a.engine.store.messages(id).unwrap_or_default())
            .unwrap_or_default();
}
fn key(a: &mut Tui, k: event::KeyEvent) -> Result<(), String> {
    if k.code == KeyCode::F(1) {
        a.mode = Mode::Help;
        return Ok(());
    }
    if k.code == KeyCode::Esc {
        if a.mode == Mode::Chat {
            if let Some(id) = &a.id {
                a.engine.cancel(id);
            }
        }
        a.mode = Mode::Chat;
        a.input.clear();
        return Ok(());
    }
    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('n') {
        a.id = Some(
            a.engine
                .new_session(a.project.clone().ok_or("Select or add a project with F3")?)?,
        );
        a.mode = Mode::Chat;
        a.input.clear();
        reload(a);
        return Ok(());
    }
    match k.code {
        KeyCode::F(2) => {
            a.mode = Mode::Sessions;
            a.index = 0;
            return Ok(());
        }
        KeyCode::F(3) => {
            a.mode = Mode::Projects;
            a.index = 0;
            return Ok(());
        }
        KeyCode::F(4) => {
            a.mode = Mode::Providers;
            a.index = 0;
            return Ok(());
        }
        KeyCode::F(5) => {
            a.mode = Mode::Model;
            a.input = a.engine.settings.model.clone();
            return Ok(());
        }
        KeyCode::F(6) => {
            a.mode = Mode::Settings;
            a.index = 0;
            return Ok(());
        }
        KeyCode::F(7) => {
            let p = a.engine.settings.provider;
            let exe = match p {
                Provider::Codex => a.engine.settings.codex_path.clone(),
                Provider::Claude => a.engine.settings.claude_path.clone(),
            };
            provider::launch_login(p, exe)?;
            a.notice = "Finish official login, then F8 Refresh".into();
            return Ok(());
        }
        KeyCode::F(8) => {
            if a.probe_receiver.is_none() {
                let p = a.engine.settings.provider;
                let exe = match p {
                    Provider::Codex => a.engine.settings.codex_path.clone(),
                    Provider::Claude => a.engine.settings.claude_path.clone(),
                };
                let cwd = a
                    .project
                    .as_ref()
                    .and_then(|id| a.engine.projects.iter().find(|p| &p.id == id))
                    .map(|p| p.path.clone())
                    .unwrap_or_else(|| ".".into());
                let (tx, rx) = mpsc::channel();
                a.probe_receiver = Some(rx);
                a.notice = "Refreshing official CLI status…".into();
                std::thread::spawn(move || {
                    let _ = tx.send((p, provider::probe(p, exe, cwd)));
                });
            }
            return Ok(());
        }
        _ => {}
    }
    if let Some(id) = a.id.clone() {
        if a.engine
            .jobs
            .get(&id)
            .is_some_and(|j| !j.approval.is_empty())
        {
            match k.code {
                KeyCode::Char('y') => {
                    a.engine.approve(&id, true);
                    return Ok(());
                }
                KeyCode::Char('n') => {
                    a.engine.approve(&id, false);
                    return Ok(());
                }
                _ => {}
            }
        }
    }
    let selected_provider =
        a.id.as_ref()
            .and_then(|id| a.engine.sessions.iter().find(|s| &s.id == id))
            .map(|s| s.provider)
            .unwrap_or(a.engine.settings.provider);
    let limit = match a.mode {
        Mode::Projects => a.engine.projects.len() + 1,
        Mode::Sessions => a.engine.sessions.len(),
        Mode::Providers => 2,
        Mode::Model => a.engine.models.get(selected_provider).len(),
        Mode::Settings => 3,
        Mode::ImportReview => a.preview.as_ref().map(|b| b.sessions.len()).unwrap_or(0),
        _ => 0,
    };
    if limit > 0 {
        match k.code {
            KeyCode::Up => {
                a.index = a.index.saturating_sub(1);
                if a.mode == Mode::Model {
                    a.input = a.engine.models.get(selected_provider)[a.index].clone();
                }
                return Ok(());
            }
            KeyCode::Down => {
                a.index = (a.index + 1).min(limit - 1);
                if a.mode == Mode::Model {
                    a.input = a.engine.models.get(selected_provider)[a.index].clone();
                }
                return Ok(());
            }
            _ => {}
        }
    }
    if a.mode == Mode::ImportReview && k.code == KeyCode::Char(' ') {
        if let Some(s) = a.preview.as_ref().and_then(|b| b.sessions.get(a.index)) {
            if a.selected.contains(&s.id) {
                a.selected.retain(|id| id != &s.id);
            } else {
                a.selected.push(s.id.clone());
            }
        }
        return Ok(());
    }
    if k.code == KeyCode::Enter {
        match a.mode {
            Mode::Chat => {
                if !a.input.trim().is_empty() {
                    if a.id.is_none() {
                        a.id = Some(a.engine.new_session(
                            a.project.clone().ok_or("Add a project folder with F3")?,
                        )?);
                    }
                    a.engine.send(a.id.as_ref().unwrap(), a.input.clone())?;
                    a.input.clear();
                    reload(a);
                }
            }
            Mode::Projects => {
                if a.index == a.engine.projects.len() {
                    a.mode = Mode::AddProject;
                    a.input.clear();
                } else {
                    a.project = Some(a.engine.projects[a.index].id.clone());
                    a.id = None;
                    a.messages.clear();
                    a.mode = Mode::Chat;
                }
            }
            Mode::Sessions => {
                if let Some(s) = a.engine.sessions.get(a.index) {
                    a.id = Some(s.id.clone());
                    a.project = Some(s.project_id.clone());
                    reload(a);
                }
                a.mode = Mode::Chat;
            }
            Mode::Providers => {
                a.engine.settings.provider = Provider::ALL[a.index];
                a.engine.settings.model.clear();
                a.engine.store.save_settings(&a.engine.settings)?;
                a.id = None;
                a.messages.clear();
                a.mode = Mode::Chat;
            }
            Mode::Model => {
                a.engine.settings.model = a.input.trim().into();
                if let Some(s) =
                    a.id.as_ref()
                        .and_then(|id| a.engine.sessions.iter_mut().find(|s| &s.id == id))
                {
                    s.model = a.engine.settings.model.clone();
                    a.engine.store.save_session(s)?;
                }
                a.engine.store.save_settings(&a.engine.settings)?;
                a.input.clear();
                a.mode = Mode::Chat;
            }
            Mode::AddProject => {
                a.project = Some(a.engine.add_project(a.input.clone())?);
                a.input.clear();
                a.id = None;
                a.mode = Mode::Chat;
            }
            Mode::Settings => match a.index {
                0 => {
                    a.mode = Mode::Import;
                    a.input.clear();
                }
                1 => {
                    a.engine.concurrency = if a.engine.concurrency == 4 {
                        1
                    } else {
                        a.engine.concurrency + 1
                    };
                }
                _ => {
                    a.notice="Run codex login or claude auth login in another terminal. Native CLI executables only; subscription auth is checked before inference.".into();
                    a.mode = Mode::Chat;
                }
            },
            Mode::Import => {
                let b = import::preview(Path::new(a.input.trim()))?;
                a.selected = b.sessions.iter().map(|s| s.id.clone()).collect();
                a.preview = Some(b);
                a.index = 0;
                a.mode = Mode::ImportReview;
            }
            Mode::ImportReview => {
                if let Some(b) = &a.preview {
                    let n = a.engine.store.import(b, &a.selected)?;
                    a.notice = format!("Imported {n} conversations; sources unchanged");
                    a.engine.refresh("")?;
                }
                a.preview = None;
                a.input.clear();
                a.mode = Mode::Chat;
            }
            Mode::Help => a.mode = Mode::Chat,
        }
        return Ok(());
    }
    if matches!(
        a.mode,
        Mode::Chat | Mode::Model | Mode::AddProject | Mode::Import
    ) {
        match k.code {
            KeyCode::Backspace => {
                a.input.pop();
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                if a.input.len() < 64 * 1024 {
                    a.input.push(c);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn draw(f: &mut Frame, a: &Tui) {
    let bg = Color::Rgb(14, 18, 25);
    let coral = Color::Rgb(254, 136, 96);
    let muted = Color::Rgb(127, 146, 167);
    f.render_widget(
        Block::default().style(Style::default().bg(bg).fg(Color::Rgb(231, 237, 245))),
        f.area(),
    );
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .margin(1)
    .split(f.area());
    let provider =
        a.id.as_ref()
            .and_then(|id| a.engine.sessions.iter().find(|s| &s.id == id))
            .map(|s| s.provider)
            .unwrap_or(a.engine.settings.provider);
    let name = a
        .project
        .as_ref()
        .and_then(|id| a.engine.projects.iter().find(|p| &p.id == id))
        .map(|p| p.name.as_str())
        .unwrap_or("no project");
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("✦ spark-code  ", Style::default().fg(coral).bold()),
            Span::raw(format!("{name}  /  {provider}")),
            Span::styled(
                format!("  •  {} agents", a.engine.jobs.len()),
                Style::default().fg(muted),
            ),
        ]))
        .block(Block::bordered().border_style(Style::default().fg(Color::Rgb(40, 50, 64)))),
        areas[0],
    );
    let cols = Layout::horizontal([Constraint::Min(20), Constraint::Length(28)]).split(areas[1]);
    let mut lines = Vec::new();
    if a.messages.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "Build something worth opening.",
            Style::default().fg(coral).bold(),
        ));
        lines.push(Line::from(""));
        lines.push(Line::from("F3: add a project  •  F4: choose a provider"));
        lines.push(Line::from(
            "Your local history is shared with the desktop app.",
        ));
    }
    for m in &a.messages {
        lines.push(Line::styled(
            m.role.to_uppercase(),
            Style::default().fg(Color::Rgb(86, 203, 177)).bold(),
        ));
        for l in m.text.lines() {
            lines.push(Line::raw(l));
        }
        lines.push(Line::from(""));
    }
    if let Some(j) = a.id.as_ref().and_then(|id| a.engine.jobs.get(id)) {
        for l in j.draft.text.lines() {
            lines.push(Line::raw(l));
        }
        if let Some((_, desc)) = j.approval.front() {
            lines.push(Line::styled(
                "APPROVAL  [y] Allow once  [n] Deny",
                Style::default().fg(coral).bold(),
            ));
            for l in desc.lines() {
                lines.push(Line::raw(l));
            }
        }
    }
    let scroll = lines
        .len()
        .saturating_sub(cols[0].height.saturating_sub(2) as usize) as u16;
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(
                Block::bordered()
                    .title(" Conversation ")
                    .border_style(Style::default().fg(Color::Rgb(40, 50, 64))),
            ),
        cols[0],
    );
    let mut agents = vec![
        Line::styled("AGENTS", Style::default().fg(coral).bold()),
        Line::raw(format!(
            "{} / {} active",
            a.engine.jobs.len(),
            a.engine.concurrency
        )),
        Line::from(""),
    ];
    for (id, j) in &a.engine.jobs {
        let s = a.engine.sessions.iter().find(|s| &s.id == id);
        agents.push(Line::raw(s.map(|s| s.title.as_str()).unwrap_or("Agent")));
        agents.push(Line::styled(&j.status, Style::default().fg(muted)));
        agents.push(Line::from(""));
    }
    if a.engine.jobs.is_empty() {
        agents.push(Line::from("Quiet by design."));
        agents.push(Line::from("Providers start on demand."));
    }
    f.render_widget(
        Paragraph::new(agents)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().border_style(Style::default().fg(Color::Rgb(40, 50, 64)))),
        cols[1],
    );
    f.render_widget(
        Paragraph::new(a.input.as_str()).block(
            Block::bordered()
                .title(" Message · Enter send ")
                .border_style(Style::default().fg(coral)),
        ),
        areas[2],
    );
    f.render_widget(Paragraph::new(vec![Line::styled("F1 Help  F2 Sessions  F3 Projects  F4 Provider  F5 Model  F6 Settings  Ctrl+N New  Ctrl+Q Quit",Style::default().fg(muted)),Line::raw(if a.notice.is_empty(){a.engine.notice.as_str()}else{a.notice.as_str()})]),areas[3]);
    if a.mode != Mode::Chat {
        let area = centered(f.area(), 74, 65);
        f.render_widget(Clear, area);
        let title = match a.mode {
            Mode::Projects => "Projects · arrows / Enter",
            Mode::Sessions => "Sessions · arrows / Enter",
            Mode::Providers => "Provider · arrows / Enter",
            Mode::Model => "Model identifier · empty = provider default",
            Mode::AddProject => "Add a project folder · full path",
            Mode::Import => "Import backup · full path",
            Mode::ImportReview => "Import preview · Space select / Enter confirm",
            Mode::Settings => "Settings · arrows / Enter",
            _ => "Keyboard shortcuts",
        };
        let options: Vec<String> = match a.mode {
            Mode::Projects => a
                .engine
                .projects
                .iter()
                .map(|p| format!("{}  {}", p.name, p.path))
                .chain(std::iter::once("+ Add project folder".into()))
                .collect(),
            Mode::Sessions => a.engine.sessions.iter().map(|s| s.title.clone()).collect(),
            Mode::Providers => vec![
                "Codex · existing ChatGPT subscription".into(),
                "Claude Code · existing Claude subscription".into(),
            ],
            Mode::Settings => vec![
                "Import a selected backup (preview first)".into(),
                format!(
                    "Concurrent agents: {} (Enter changes)",
                    a.engine.concurrency
                ),
                "Official provider login help".into(),
            ],
            Mode::ImportReview => a
                .preview
                .as_ref()
                .map(|b| {
                    b.sessions
                        .iter()
                        .map(|s| {
                            format!(
                                "[{}] {}",
                                if a.selected.contains(&s.id) { "x" } else { " " },
                                s.title
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Mode::Help => vec![
                "F2 Sessions  •  F3 Projects  •  F4 Provider  •  F5 Model".into(),
                "F6 Settings & import  •  Ctrl+N New conversation".into(),
                "F7 Official login  •  F8 Refresh account/models".into(),
                "F5 Model: type a name, or arrows choose refreshed models".into(),
                "Enter Send  •  Esc Stop current agent / close dialog".into(),
                "y Allow once / n Deny when approval is visible".into(),
                "Ctrl+Q Quit  •  spark-code gui opens the desktop app".into(),
                "One active agent per folder; different projects can run together.".into(),
                "Use official subscription login. No API billing fallback.".into(),
            ],
            _ => vec![
                a.input.clone(),
                String::new(),
                "Enter to confirm · Esc to cancel".into(),
            ],
        };
        let items = options
            .iter()
            .enumerate()
            .map(|(i, s)| {
                ListItem::new(format!("{} {s}", if i == a.index { "›" } else { " " })).style(
                    if i == a.index {
                        Style::default().fg(coral)
                    } else {
                        Style::default()
                    },
                )
            })
            .collect::<Vec<_>>();
        f.render_widget(
            List::new(items).block(
                Block::bordered()
                    .title(title)
                    .style(Style::default().bg(bg))
                    .border_style(Style::default().fg(coral)),
            ),
            area,
        );
    }
}
fn centered(r: Rect, w: u16, h: u16) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - h) / 2),
        Constraint::Percentage(h),
        Constraint::Percentage((100 - h) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - w) / 2),
        Constraint::Percentage(w),
        Constraint::Percentage((100 - w) / 2),
    ])
    .split(v[1])[1]
}
