//! Settings surfaces share the same row hierarchy and persist through the GUI update path.
use super::*;
use iced::widget::{column, toggler};
use spark_code::local_import::SourceKind;

pub(super) fn history_sources(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let s = &app.engine.as_ref().unwrap().settings;
    let mut cards = column![].spacing(10);
    for kind in SourceKind::ALL {
        let source = app
            .history_sources
            .iter()
            .find(|source| source.kind == kind);
        let available = source.is_some_and(|source| source.available());
        let path = source
            .map(|source| source.path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Detecting local data folder…".into());
        let mut header = row![
            checkbox(kind.enabled(s))
                .label(kind.label())
                .text_size(13)
                .on_toggle(move |v| Msg::HistorySource(kind, v)),
            Space::new().width(Length::Fill),
            text(if available { "Found" } else { "Not found" })
                .size(11)
                .color(c.muted)
        ]
        .align_y(Alignment::Center)
        .spacing(10);
        header = header.push(compact_button("Choose folder", Msg::HistoryFolder(kind)));
        if !kind.folder(s).is_empty() {
            header = header.push(compact_button("Reset", Msg::ResetHistoryFolder(kind)));
        }
        let mut body = column![
            header,
            text(ellipsize(&path, 88))
                .size(11)
                .font(Font::MONOSPACE)
                .color(c.faint)
        ]
        .spacing(9);
        if let Some(result) = app
            .history_results
            .iter()
            .find(|r| r.source.kind == kind && source.is_some_and(|s| s.path == r.source.path))
        {
            let status = match &result.result {
                Ok((added, sessions, messages)) => {
                    format!("{sessions} conversations · {messages} messages · {added} new")
                }
                Err(error) => format!("Needs attention: {}", ellipsize(error, 220)),
            };
            body = body.push(text(status).size(12).color(c.muted));
        }
        cards = cards.push(
            container(body)
                .padding(14)
                .width(Length::Fill)
                .style(move |_| a::panel(c, 8.)),
        );
    }
    cards.into()
}

fn toggle_row<'a>(
    label: &'static str,
    detail: &'static str,
    value: bool,
    on_toggle: impl Fn(bool) -> Msg + 'a,
    c: Colors,
) -> Element<'a, Msg> {
    setting_row(
        label,
        detail,
        toggler(value).size(20).on_toggle(on_toggle).into(),
        c,
    )
}
fn setting_row<'a>(
    label: &'static str,
    detail: &'static str,
    control: Element<'a, Msg>,
    c: Colors,
) -> Element<'a, Msg> {
    container(
        row![
            column![
                text(label).size(14).color(c.text),
                text(detail).size(12).color(c.muted)
            ]
            .spacing(6)
            .width(Length::Fill),
            control
        ]
        .spacing(24)
        .align_y(Alignment::Center),
    )
    .padding([18, 0])
    .into()
}
fn link<'a>(label: &'static str, url: &'static str) -> Element<'a, Msg> {
    compact_button(label, Msg::OpenLink(url.into())).into()
}
fn section<'a>(title: &'static str, c: Colors) -> Element<'a, Msg> {
    container(text(title).size(15).font(semibold()).color(c.text))
        .padding(iced::padding::top(22))
        .into()
}
pub(super) fn view(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let s = &e.settings;
    let mut body = column![
        text(app.settings_tab.title())
            .size(27)
            .font(semibold())
            .color(c.text),
        text(app.settings_tab.description()).size(13).color(c.muted),
        Space::new().height(15),
    ]
    .spacing(8);
    match app.settings_tab {
        SettingsTab::General => {
            body = body.push(section("Workspace", c)).push(line(c))
                .push(toggle_row("Show workspace sidebar", "Show projects and conversations in chat. Settings navigation stays available.", app.sidebar_visible, |_| Msg::ToggleSidebar, c))
                .push(line(c)).push(toggle_row("Show agent activity", "Keep the activity panel open. Remembered on restart.", app.show_agents, |_| Msg::ToggleAgents, c))
                .push(line(c)).push(toggle_row("Follow new responses", "Scroll with your agent. Scrolling up pauses following until you return to the bottom.", s.auto_scroll, |v| Msg::Preference(Preference::AutoScroll(v)), c))
                .push(section("Connections", c)).push(line(c))
                .push(toggle_row("Find installed CLIs on startup", "Detect official Codex and Claude Code executables from PATH.", s.auto_detect_cli, |v| Msg::Preference(Preference::AutoDetect(v)), c))
                .push(line(c)).push(toggle_row("Check connections after detection", "Check sign-in, available models, and usage. No messages are sent.", s.auto_refresh_providers, |v| Msg::Preference(Preference::AutoRefresh(v)), c))
                .push(section("Spark Code", c)).push(line(c))
                .push(text(format!("Version {} · Native Rust desktop", env!("CARGO_PKG_VERSION"))).size(12).color(c.muted))
                .push(text("Settings are saved automatically on this computer.").size(12).color(c.faint));
            body = body.push(compact_button(
                "Replay welcome setup",
                Msg::ReplayOnboarding,
            ));
        }
        SettingsTab::Appearance => {
            body = body
                .push(section("Theme", c))
                .push(line(c))
                .push(setting_row(
                    "Color theme",
                    "A familiar workspace in your preferred light.",
                    row![
                        compact_button("Dark", Msg::ToggleTheme)
                            .on_press_maybe(s.light_theme.then_some(Msg::ToggleTheme))
                            .style(if !s.light_theme {
                                a::selected
                            } else {
                                a::outline
                            }),
                        compact_button("Light", Msg::ToggleTheme)
                            .on_press_maybe((!s.light_theme).then_some(Msg::ToggleTheme))
                            .style(if s.light_theme {
                                a::selected
                            } else {
                                a::outline
                            }),
                    ]
                    .spacing(8)
                    .into(),
                    c,
                ))
                .push(line(c))
                .push(toggle_row(
                    "Reduced motion",
                    "Skip transitions and keep navigation immediate.",
                    s.reduced_motion,
                    Msg::ReducedMotion,
                    c,
                ))
                .push(section("Conversation", c))
                .push(line(c))
                .push(setting_row(
                    "Text size",
                    "Change the size of messages and the composer.",
                    pick_list([13u32, 15, 17, 19], Some(s.chat_text_size), |v| {
                        Msg::Preference(Preference::TextSize(v))
                    })
                    .text_size(13)
                    .padding(8)
                    .style(a::picker)
                    .into(),
                    c,
                ))
                .push(line(c))
                .push(toggle_row(
                    "Compact conversation spacing",
                    "Fit more messages into the transcript.",
                    s.compact_layout,
                    |v| Msg::Preference(Preference::Compact(v)),
                    c,
                ))
                .push(line(c))
                .push(toggle_row(
                    "Message timestamps",
                    "Show when a message was sent beside its author.",
                    s.show_timestamps,
                    |v| Msg::Preference(Preference::Timestamps(v)),
                    c,
                ));
        }
        SettingsTab::Providers => {
            let refreshing = app.probe_receiver.is_some();
            body = body.push(
                row![
                    button(
                        text(if refreshing {
                            "Checking…"
                        } else {
                            "Refresh connections"
                        })
                        .size(12)
                    )
                    .padding([9, 12])
                    .style(a::primary)
                    .on_press_maybe((!refreshing).then_some(Msg::ProbeAll)),
                    compact_button("Find installed CLIs", Msg::DetectCli),
                ]
                .spacing(8),
            );
            for provider in Provider::ALL {
                body = body
                    .push(line(c))
                    .push(container(provider_row(app, provider)).padding([14, 0]));
            }
            body = body.push(line(c)).push(text("Already signed in to Codex? Spark Code uses the same CLI-managed account and refreshes credentials through Codex.").size(12).color(c.muted))
                .push(section("Install an official CLI", c))
                .push(row![link("Get Codex CLI", "https://developers.openai.com/codex/cli"), link("Get Claude Code", "https://code.claude.com/docs/en/setup")].spacing(8))
                .push(text("After installing or signing in, refresh connections to load your models.").size(12).color(c.faint));
        }
        SettingsTab::ChatGpt => {
            let state = e.providers.get(Provider::Codex);
            body = body.push(section("ChatGPT account", c)).push(line(c))
                .push(row![provider_mark(Provider::Codex, 28., c), column![text(if state.ready() { "Connected through Codex" } else { "Connect with ChatGPT" }).size(16).font(semibold()).color(c.text),
                    text(if state.ready() { state.status.as_str() } else { "Use your ChatGPT account with the official Codex CLI." }).size(12).color(c.muted)].spacing(6).width(Length::Fill)].spacing(14).align_y(Alignment::Center))
                .push(row![compact_button("Sign in with ChatGPT", Msg::Login(Provider::Codex)).style(a::primary), compact_button("Refresh account", Msg::ProbeAll)].spacing(8))
                .push(text("Codex owns your sign-in and token refresh. Spark Code doesn't need a pasted session token.").size(12).color(c.muted))
                .push(section("Coding in Spark Code", c)).push(line(c))
                .push(text("Stream responses, select CLI-reported models and reasoning effort, resume conversations, and review tool approvals using your Codex account.").size(13).color(c.muted))
                .push(provider_usage(app, Provider::Codex, false))
                .push(section("ChatGPT & Dots", c)).push(line(c))
                .push(text("Dots keep work moving in ChatGPT with their own cloud computer, connected apps, and memory. Open ChatGPT to use the Dots available to your account.").size(13).color(c.muted))
                .push(row![link("Open ChatGPT", "https://chatgpt.com/"), link("Meet Dots", "https://learn.chatgpt.com/docs/dots")].spacing(8))
                .push(text("Dots, voice, and other ChatGPT-only tools run in ChatGPT. They are not exposed as native Spark Code features.").size(12).color(c.faint));
        }
        SettingsTab::Performance => {
            body = body.push(section("Renderer", c)).push(line(c));
            if let Some(info) = &app.system_info {
                body = body
                    .push(
                        text(&info.graphics_adapter)
                            .size(14)
                            .font(semibold())
                            .color(c.text),
                    )
                    .push(
                        text(format!("Backend: {}", info.graphics_backend))
                            .size(12)
                            .color(c.muted),
                    );
                if let Some(bytes) = info.memory_used {
                    body = body.push(
                        text(format!(
                            "Process memory at last check: {:.0} MiB",
                            bytes as f64 / 1048576.
                        ))
                        .size(12)
                        .color(c.muted),
                    );
                }
            } else {
                body = body.push(
                    text("Reading renderer information…")
                        .size(12)
                        .color(c.muted),
                );
            }
            body = body.push(compact_button("Refresh diagnostics", Msg::RefreshDiagnostics))
                .push(text("GPU rendering is preferred automatically. A software renderer is available when a compatible GPU cannot be initialized.").size(12).color(c.muted))
                .push(line(c)).push(toggle_row("Smooth vector edges", "Enable multisample antialiasing. Requires an app restart and uses more GPU memory.", s.antialiasing, |v| Msg::Preference(Preference::Antialiasing(v)), c))
                .push(section("Responsiveness", c)).push(line(c))
                .push(setting_row("Stream refresh interval", "Milliseconds between response updates. Lower values use more CPU while agents work.", pick_list([16u64, 33, 80, 150], Some(s.stream_interval_ms), |v| Msg::Preference(Preference::StreamInterval(v))).text_size(13).padding(8).style(a::picker).into(), c))
                .push(line(c)).push(setting_row("Messages in the transcript", "Render the most recent messages. Full history remains in your local database and exports.", pick_list([20usize, 50, 100], Some(s.visible_messages), |v| Msg::Preference(Preference::HistoryLimit(v))).text_size(13).padding(8).style(a::picker).into(), c))
                .push(line(c)).push(setting_row("Concurrent agents", "Each project folder can run one agent. More agents use more memory and quota.", pick_list([1usize, 2, 3, 4], Some(e.concurrency), |v| Msg::Concurrency(v.to_string())).text_size(13).padding(8).style(a::picker).into(), c))
                .push(text("Idle workspaces stop polling. Markdown is cached, streaming updates are batched, and typing doesn't reload chat history.").size(12).color(c.faint));
        }
        SettingsTab::Data => {
            body=body.push(section("Connected history",c)).push(line(c))
                .push(toggle_row("Automatically import local history", "Sync enabled sources on launch and every minute. Changes flow into Spark Code.",s.auto_import_history,Msg::AutoImportHistory,c))
                .push(history_sources(app))
                .push(row![button(text(if app.history_receiver.is_some() {"Working…"} else {"Sync now"}).size(12)).padding([9,12]).style(a::primary).on_press_maybe(app.history_receiver.is_none().then_some(Msg::SyncHistory)),compact_button("Detect again",Msg::DiscoverHistory)].spacing(8))
                .push(text(&app.history_status).size(12).color(c.muted))
                .push(text("Imports the latest 200 conversations and up to 100 messages each. Original history stays in its source app. Local replies are preserved on every sync.").size(12).color(c.faint))
                .push(text("Codex and T3 Code import directly from their databases, including changes in live WAL files. Choose a custom data folder if your installation stores history elsewhere.").size(12).color(c.muted));
            body = body.push(section("Import conversations", c)).push(line(c))
                .push(text("Preview a Spark Code export or a T3 Code backup and choose the conversations to import.").size(13).color(c.muted))
                .push(row![button(text("Choose backup file…").size(12)).padding([9, 12]).style(a::outline).on_press_maybe((!app.importing).then_some(Msg::ImportFile)),
                    button(text("Preview Codex history").size(12)).padding([9, 12]).style(a::outline).on_press_maybe((!app.importing && app.project.is_some()).then_some(Msg::ImportCodex))].spacing(8))
                .push(text("Select a project to preview its Codex history. Imported T3 conversations start a fresh provider session.").size(12).color(c.faint))
                .push(section("Export & storage", c)).push(line(c))
                .push(setting_row("Export local history", "Save projects and conversations to a JSON backup.", compact_button("Export history", Msg::Export).into(), c))
                .push(text(spark_code::store::data_dir().to_string_lossy().into_owned()).size(12).font(Font::MONOSPACE).color(c.muted))
                .push(text("Local history and exports include messages and project paths and are unencrypted. Account credentials stay with the official CLIs.").size(12).color(c.faint));
        }
        SettingsTab::Shortcuts => {
            for (label, keys) in [
                ("New conversation", "Ctrl + N"),
                ("Open settings", "Ctrl + ,"),
                ("Toggle sidebar", "Ctrl + B"),
                ("Toggle agent activity", "Ctrl + Shift + A"),
                ("Switch theme", "Ctrl + Shift + T"),
                ("Send message", "Enter"),
                ("New line", "Shift + Enter"),
                ("Cancel access confirmation", "Escape"),
                ("Window menu (Windows)", "Alt + Space"),
            ] {
                body = body.push(line(c)).push(
                    container(
                        row![
                            text(label).size(13).color(c.text).width(Length::Fill),
                            container(text(keys).size(12).color(c.muted))
                                .padding([6, 10])
                                .style(move |_| a::panel(c, 6.))
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([12, 0]),
                );
            }
            body = body.push(text("Enter sends only while the composer is focused. Input-method composition is respected.").size(12).color(c.faint));
        }
    }
    if !app.notice.is_empty() || app.importing {
        body = body.push(Space::new().height(12)).push(line(c)).push(
            text(if app.importing {
                "Reading selected history…"
            } else {
                &app.notice
            })
            .size(12)
            .color(c.muted),
        );
    }
    scrollable(
        container(container(body).max_width(760).width(Length::Fill))
            .padding([32, 36])
            .center_x(Length::Fill),
    )
    .id("settings-content")
    .height(Length::Fill)
    .into()
}
