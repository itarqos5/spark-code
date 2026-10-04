//! Native workspace presentation. State and actions live in `gui`.
// THESIS: a focused conversation workspace familiar to users of T3 Code and ChatGPT.
// OWN-WORLD: graphite and paper surfaces, DM Sans, precise seams, quiet state colors.
// STORY: choose a project or chat, connect an official CLI, then work in one transcript.
// FIRST VIEWPORT: compact left rail; spacious conversation; centered, anchored composer.
// FORM: the user's explicit T3 Code / ChatGPT commitment overrides seed 1fbf973f.
// FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, and DESIGN.md
use super::{App, Msg, Preference, SettingsTab};
#[path = "onboarding_ui.rs"]
mod onboarding_ui;
#[path = "settings_ui.rs"]
mod settings_ui;
use crate::{
    appearance as a,
    appearance::Colors,
    icons::{Kind, icon},
};
use iced::{
    Alignment, Color, Element, Font, Length, Theme,
    font::Weight,
    widget::{
        Space, button, checkbox, column, container, markdown, mouse_area, opaque, pick_list,
        progress_bar, row, scrollable, stack, svg, text, text_editor, text_input, tooltip,
    },
};
use spark_code::{
    model::{Message, Provider},
    provider_status::Readiness,
};

fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..Font::with_name("DM Sans")
    }
}
fn line<'a>(c: Colors) -> Element<'a, Msg> {
    container(Space::new().height(1))
        .width(Length::Fill)
        .style(move |_| a::flat(c.border))
        .into()
}
fn vertical_line<'a>(c: Colors) -> Element<'a, Msg> {
    container(Space::new().width(1))
        .height(Length::Fill)
        .style(move |_| a::flat(c.border))
        .into()
}
fn icon_button<'a>(kind: Kind, label: &'static str, msg: Msg, c: Colors) -> Element<'a, Msg> {
    tooltip(
        button(
            container(icon(kind, 16., c.muted))
                .center_x(30)
                .center_y(28),
        )
        .padding(0)
        .style(a::ghost)
        .on_press(msg),
        text(label).size(11),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}
fn compact_button<'a>(label: &'a str, msg: Msg) -> iced::widget::Button<'a, Msg> {
    button(text(label).size(12))
        .padding([7, 11])
        .style(a::outline)
        .on_press(msg)
}
fn section_label<'a>(label: &'a str, c: Colors) -> Element<'a, Msg> {
    text(label).size(10).font(semibold()).color(c.faint).into()
}
fn ellipsize(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_owned()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(limit.saturating_sub(1))
                .collect::<String>()
        )
    }
}

pub(super) fn view(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let main: Element<'_, Msg> = match &app.engine {
        Err(error) => container(
            column![
                icon(Kind::Bolt, 34., c.text),
                text("Couldn't open this workspace")
                    .size(24)
                    .font(semibold()),
                text("Spark Code couldn't open its local storage.")
                    .size(14)
                    .color(c.muted),
                text(error).size(13).color(c.muted),
            ]
            .spacing(16),
        )
        .padding(48)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into(),
        Ok(_) if app.onboarding.is_some() => onboarding_ui::view(app),
        Ok(_) => {
            let center = if app.preview.is_some() {
                import_review(app)
            } else if app.settings {
                settings(app)
            } else {
                chat(app)
            };
            let center =
                container(center).padding(iced::padding::top((1. - app.motion_progress()) * 7.));
            let mut workspace = row![].height(Length::Fill);
            if app.sidebar_visible || app.settings {
                workspace = workspace.push(sidebar(app)).push(vertical_line(c));
            }
            workspace = workspace.push(column![toolbar(app), line(c), center].width(Length::Fill));
            if app.show_agents && !app.settings {
                workspace = workspace.push(vertical_line(c)).push(activity(app));
            }
            workspace.into()
        }
    };
    let workspace: Element<'_, Msg> = container(column![titlebar(app), line(c), main].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| a::flat(c.bg))
        .into();
    if app.full_access_confirmation {
        stack![workspace, full_access_confirmation(app)].into()
    } else {
        workspace
    }
}

fn titlebar(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let light = app.engine.as_ref().is_ok_and(|e| e.settings.light_theme);
    let drag = mouse_area(
        container(
            row![
                icon(Kind::Bolt, 17., c.text),
                text("Spark Code").size(12).font(semibold()).color(c.text),
                Space::new().width(Length::Fill),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        )
        .padding([0, 14])
        .height(37)
        .width(Length::Fill)
        .center_y(37),
    )
    .on_press(Msg::DragWindow)
    .on_double_click(Msg::Maximize)
    .on_right_press(Msg::SystemMenu);
    container(
        row![
            drag,
            icon_button(
                if light { Kind::Moon } else { Kind::Sun },
                "Switch appearance",
                Msg::ToggleTheme,
                c
            ),
            if app.onboarding.is_none() {
                icon_button(Kind::Settings, "Settings", Msg::Settings, c)
            } else {
                Space::new().width(0).into()
            },
            if app.onboarding.is_none() {
                icon_button(Kind::Panel, "Agent activity", Msg::ToggleAgents, c)
            } else {
                Space::new().width(0).into()
            },
            Space::new().width(8),
            button(
                container(icon(Kind::Minimize, 13., c.muted))
                    .center_x(43)
                    .center_y(37)
            )
            .padding(0)
            .style(a::ghost)
            .on_press(Msg::Minimize),
            button(
                container(icon(Kind::Maximize, 12., c.muted))
                    .center_x(43)
                    .center_y(37)
            )
            .padding(0)
            .style(a::ghost)
            .on_press(Msg::Maximize),
            button(
                container(icon(Kind::Close, 15., c.muted))
                    .center_x(43)
                    .center_y(37)
            )
            .padding(0)
            .style(a::close)
            .on_press(Msg::CloseWindow),
        ]
        .align_y(Alignment::Center),
    )
    .style(move |_| a::flat(c.side))
    .into()
}

fn settings_sidebar(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let heading = row![
        text("Settings").size(13).font(semibold()).color(c.text),
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);
    let mut tabs = column![].spacing(5);
    for tab in SettingsTab::ALL {
        let kind = match tab {
            SettingsTab::General => Kind::Settings,
            SettingsTab::Appearance => Kind::Sun,
            SettingsTab::Providers => Kind::Terminal,
            SettingsTab::ChatGpt => Kind::Chat,
            SettingsTab::Performance => Kind::Bolt,
            SettingsTab::Data => Kind::Folder,
            SettingsTab::Shortcuts => Kind::Code,
        };
        tabs = tabs.push(
            button(
                row![
                    icon(kind, 16., c.muted),
                    text(tab.title()).size(13),
                    Space::new().width(Length::Fill)
                ]
                .spacing(11)
                .align_y(Alignment::Center),
            )
            .padding([11, 10])
            .width(Length::Fill)
            .style(if app.settings_tab == tab {
                a::selected
            } else {
                a::ghost
            })
            .on_press(Msg::SettingsTab(tab)),
        );
    }
    let back = button(
        row![
            icon(Kind::Back, 15., c.muted),
            text("Back").size(12),
            Space::new().width(Length::Fill)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(Msg::Settings)
    .padding([8, 9])
    .width(Length::Fill)
    .style(a::ghost);
    container(
        column![
            heading,
            Space::new().height(5),
            tabs,
            Space::new().height(Length::Fill),
            line(c),
            back,
        ]
        .spacing(8),
    )
    .padding([12, 12])
    .width(248)
    .height(Length::Fill)
    .style(move |_| a::flat(c.side))
    .into()
}

fn sidebar(app: &App) -> Element<'_, Msg> {
    if app.settings {
        return settings_sidebar(app);
    }
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let heading = row![
        text("Your workspace")
            .size(13)
            .font(semibold())
            .color(c.text),
        Space::new().width(Length::Fill),
        icon_button(Kind::Panel, "Hide sidebar", Msg::ToggleSidebar, c),
    ]
    .align_y(Alignment::Center);
    let new_chat = button(
        row![
            icon(Kind::Plus, 15., c.text),
            text("New chat").size(13),
            Space::new().width(Length::Fill),
            text("Ctrl N").size(10).color(c.faint),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(Msg::New)
    .padding([9, 10])
    .width(Length::Fill)
    .style(a::selected);
    let search = container(
        row![
            icon(Kind::Search, 14., c.faint),
            text_input("Search conversations", &app.search)
                .on_input(Msg::Search)
                .size(11)
                .padding([7, 0])
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: c.side.into(),
                    border: iced::Border::default(),
                    icon: c.faint,
                    placeholder: c.faint,
                    value: c.text,
                    selection: c.raised,
                }),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([0, 8]);
    let projects_heading = row![
        section_label("PROJECTS", c),
        Space::new().width(Length::Fill),
        icon_button(Kind::Plus, "Add project folder", Msg::AddProject, c)
    ]
    .align_y(Alignment::Center);
    let mut projects = column![].spacing(3);
    for p in &e.projects {
        let selected = app.project.as_ref() == Some(&p.id);
        projects = projects.push(
            button(
                row![
                    icon(Kind::Folder, 15., if selected { c.text } else { c.muted }),
                    text(ellipsize(&p.name, 23)).size(12),
                    Space::new().width(Length::Fill)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([8, 9])
            .style(if selected { a::selected } else { a::ghost })
            .on_press(Msg::Project(p.id.clone())),
        );
    }
    let mut sessions = column![].spacing(3);
    let mut last_group = "";
    let visible_sessions = app.filtered_sessions.as_deref().unwrap_or(&e.sessions);
    for s in visible_sessions {
        let age = spark_code::model::now().saturating_sub(s.updated);
        let group = if age < 86400 {
            "Today"
        } else if age < 7 * 86400 {
            "Previous 7 days"
        } else {
            "Earlier"
        };
        if group != last_group && app.search.is_empty() {
            sessions =
                sessions.push(container(text(group).size(11).color(c.faint)).padding([10, 9]));
            last_group = group;
        }
        let selected = app.selected.as_ref() == Some(&s.id) && !app.settings;
        let running = e.jobs.contains_key(&s.id);
        sessions = sessions.push(
            button(
                row![
                    icon(
                        if running { Kind::Bolt } else { Kind::Chat },
                        14.,
                        if selected || running { c.text } else { c.faint }
                    ),
                    text(ellipsize(&s.title.replace('\n', " "), 25)).size(12),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press(Msg::Select(s.id.clone()))
            .width(Length::Fill)
            .padding([8, 9])
            .style(if selected { a::selected } else { a::ghost }),
        );
    }
    if visible_sessions.is_empty() {
        sessions = sessions.push(
            container(
                text(if app.search.is_empty() {
                    "Your conversations will appear here."
                } else {
                    "No matching conversations."
                })
                .size(11)
                .color(c.faint),
            )
            .padding([9, 9]),
        );
    }
    let mut bottom = column![line(c)].spacing(7);
    for provider in Provider::ALL {
        if e.settings.enabled(provider) {
            bottom = bottom.push(provider_usage(app, provider, true));
        }
    }
    bottom = bottom.push(
        button(
            row![
                provider_mark(Provider::Codex, 15., c),
                text("ChatGPT & Dots").size(12),
                Space::new().width(Length::Fill),
                icon(Kind::Chevron, 12., c.faint)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([8, 9])
        .width(Length::Fill)
        .style(a::ghost)
        .on_press(Msg::SettingsTab(SettingsTab::ChatGpt)),
    );
    bottom = bottom.push(
        button(
            row![
                icon(Kind::Bolt, 15., c.muted),
                text("Agents").size(12),
                Space::new().width(Length::Fill),
                text(if e.jobs.is_empty() {
                    "Idle".into()
                } else {
                    format!("{} active", e.jobs.len())
                })
                .size(10)
                .color(c.faint)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(Msg::ToggleAgents)
        .padding([8, 9])
        .style(a::ghost)
        .width(Length::Fill),
    );
    bottom = bottom.push(
        button(
            row![
                icon(Kind::Settings, 15., c.muted),
                text("Settings").size(12),
                Space::new().width(Length::Fill)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(Msg::Settings)
        .padding([8, 9])
        .width(Length::Fill)
        .style(if app.settings { a::selected } else { a::ghost }),
    );
    container(
        column![
            heading,
            Space::new().height(5),
            new_chat,
            search,
            Space::new().height(8),
            column![
                projects_heading,
                container(scrollable(projects)).max_height(64)
            ]
            .spacing(3),
            Space::new().height(14),
            section_label("CONVERSATIONS", c),
            scrollable(sessions).height(Length::Fill),
            bottom,
        ]
        .spacing(8),
    )
    .padding([12, 12])
    .width(248)
    .height(Length::Fill)
    .style(move |_| a::flat(c.side))
    .into()
}

fn toolbar(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let project = app
        .project
        .as_ref()
        .and_then(|id| e.projects.iter().find(|p| &p.id == id));
    let session = app
        .selected
        .as_ref()
        .and_then(|id| e.sessions.iter().find(|s| &s.id == id));
    let title = if app.preview.is_some() {
        "Import review".to_owned()
    } else if app.settings {
        format!("Settings / {}", app.settings_tab.title())
    } else {
        session
            .map(|s| ellipsize(&s.title.replace('\n', " "), 43))
            .unwrap_or_else(|| "New chat".into())
    };
    let mut items = row![].spacing(11).align_y(Alignment::Center);
    if !app.sidebar_visible && !app.settings {
        items = items.push(icon_button(
            Kind::Panel,
            "Show sidebar",
            Msg::ToggleSidebar,
            c,
        ));
    }
    if let (Some(project), false) = (project, app.settings) {
        items = items
            .push(icon(Kind::Folder, 14., c.faint))
            .push(text(ellipsize(&project.name, 20)).size(12).color(c.muted))
            .push(text("/").size(12).color(c.faint));
    }
    items = items
        .push(text(title).size(12).color(c.text))
        .push(Space::new().width(Length::Fill));
    if !app.settings && app.preview.is_none() {
        items = items.push(
            pick_list(Provider::ALL, Some(app.current_provider), Msg::Provider)
                .text_size(12)
                .padding([6, 8])
                .style(a::picker),
        );
        items = items.push(icon_button(
            Kind::Panel,
            "Agent activity · Ctrl Shift A",
            Msg::ToggleAgents,
            c,
        ));
    }
    if !e.jobs.is_empty() {
        items = items.push(
            button(
                row![
                    icon(Kind::Bolt, 12., c.muted),
                    text(format!("{} working", e.jobs.len())).size(11)
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .padding([5, 8])
            .style(a::ghost)
            .on_press(Msg::ToggleAgents),
        );
    }
    if app.settings || app.preview.is_some() {
        items = items.push(icon_button(
            Kind::Close,
            "Back to chat",
            if app.preview.is_some() {
                Msg::CloseImport
            } else {
                Msg::Settings
            },
            c,
        ));
    }
    container(items)
        .padding([0, 22])
        .height(51)
        .center_y(51)
        .width(Length::Fill)
        .into()
}

fn chat(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let current = app.selected.as_ref().and_then(|id| e.jobs.get(id));
    let has_content = !app.messages.is_empty() || current.is_some();
    let transcript: Element<'_, Msg> = if !has_content {
        welcome(app)
    } else {
        let mut messages = column![].spacing(if e.settings.compact_layout { 18 } else { 32 });
        for message in &app.messages {
            messages = messages.push(message_view(app, message, false));
        }
        if let Some(job) = current {
            if !job.draft.text.is_empty() {
                messages = messages.push(message_view(app, &job.draft, true));
            } else {
                messages = messages.push(
                    row![
                        icon(Kind::Bolt, 17., c.muted),
                        text(&job.status).size(13).color(c.muted)
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                );
            }
            if let Some((_, request)) = job.approval.front() {
                let id = app.selected.as_ref().unwrap();
                messages = messages.push(
                    container(
                        column![
                            row![
                                icon(Kind::Terminal, 16., c.text),
                                text("Permission needed").size(13).font(semibold())
                            ]
                            .spacing(9),
                            text(request).size(13).color(c.muted),
                            row![
                                compact_button("Allow once", Msg::Approve(id.clone(), true))
                                    .style(a::primary),
                                compact_button("Deny", Msg::Approve(id.clone(), false))
                            ]
                            .spacing(8),
                        ]
                        .spacing(11),
                    )
                    .padding(16)
                    .style(move |_| a::panel(c, 10.)),
                );
            }
        }
        scrollable(
            container(container(messages).max_width(820).width(Length::Fill))
                .padding([28, 26])
                .center_x(Length::Fill),
        )
        .id("transcript")
        .on_scroll(|viewport| {
            Msg::Scrolled(
                viewport.absolute_offset().y
                    >= (viewport.content_bounds().height - viewport.bounds().height - 36.).max(0.),
            )
        })
        .height(Length::Fill)
        .into()
    };
    let mut main = column![transcript].height(Length::Fill);
    if app
        .selected
        .as_ref()
        .and_then(|id| e.sessions.iter().find(|s| &s.id == id))
        .is_some_and(|s| {
            (s.id.starts_with("t3:") || s.id.starts_with("linked:")) && s.remote_id.is_none()
        })
    {
        main = main.push(
            container(compact_button(
                "Add imported conversation to draft",
                Msg::UseHistory,
            ))
            .padding([5, 25])
            .center_x(Length::Fill),
        );
    }
    if !app.follow_stream && current.is_some() && e.settings.auto_scroll {
        main = main.push(
            container(compact_button("Jump to latest", Msg::JumpToLatest))
                .center_x(Length::Fill)
                .padding(5),
        );
    }
    main.push(composer(app)).into()
}

fn welcome(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let project = app
        .project
        .as_ref()
        .and_then(|id| e.projects.iter().find(|p| &p.id == id));
    let title = project
        .map(|p| format!("Let's work on {}", ellipsize(&p.name, 24)))
        .unwrap_or_else(|| "What are we building today?".into());
    let heading = text(title).size(32).font(semibold()).color(c.text);
    let subtitle = if project.is_some() {
        "Plan a change, explore your code, or let an agent help you build."
    } else {
        "A little clarity. A big idea. Your next great project."
    };
    let prompts = row![
        suggestion("Explore code", "Help me understand the structure of this project and identify the important entry points.", Kind::Code, c),
        suggestion("Fix a bug", "Help me investigate a bug. Start by asking me about the behavior and how to reproduce it.", Kind::Bug, c),
        suggestion("Plan a feature", "Help me plan a feature. Ask about the requirements, then outline an implementation plan.", Kind::Plus, c),
    ].spacing(10).wrap();
    let connection: Element<'_, Msg> = if !app.provider_ready(app.current_provider) {
        button(
            row![
                icon(Kind::Terminal, 14., c.muted),
                text("Connect your agent to get started").size(12),
                icon(Kind::Chevron, 12., c.faint)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([9, 12])
        .style(a::ghost)
        .on_press(Msg::SettingsTab(SettingsTab::Providers))
        .into()
    } else {
        text("Connected through your official CLI")
            .size(11)
            .color(c.faint)
            .into()
    };
    let content = column![
        container(icon(Kind::Bolt, 38., c.text)).center_x(Length::Fill),
        Space::new().height(8),
        heading,
        text(subtitle).size(14).color(c.muted),
        Space::new().height(18),
        prompts,
        Space::new().height(10),
        connection,
    ]
    .spacing(11)
    .align_x(Alignment::Center)
    .width(Length::Fill);
    container(container(content).max_width(720).width(Length::Fill))
        .padding([24, 30])
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn suggestion<'a>(
    label: &'static str,
    prompt: &'static str,
    kind: Kind,
    c: Colors,
) -> Element<'a, Msg> {
    button(
        row![icon(kind, 15., c.muted), text(label).size(12)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([12, 15])
    .style(a::outline)
    .on_press(Msg::QuickPrompt(prompt.into()))
    .into()
}

/// Official provider logos as embedded SVGs: OpenAI (theme text color) and Claude (brand orange).
fn provider_mark<'a>(provider: Provider, size: f32, c: Colors) -> Element<'a, Msg> {
    let (bytes, tint): (&'static [u8], Color) = match provider {
        Provider::Codex => (include_bytes!("../assets/icons/openai.svg"), c.text),
        Provider::Claude => (
            include_bytes!("../assets/icons/claude.svg"),
            Color::from_rgb8(0xD9, 0x77, 0x57),
        ),
    };
    svg(svg::Handle::from_memory(bytes))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(tint) })
        .into()
}

fn usage_meter<'a>(fraction: f32, c: Colors) -> Element<'a, Msg> {
    progress_bar(0.0..=1.0, fraction.clamp(0., 1.))
        .girth(3)
        .style(move |_| iced::widget::progress_bar::Style {
            background: c.raised.into(),
            bar: c.muted.into(),
            border: iced::Border {
                radius: 2.into(),
                ..Default::default()
            },
        })
        .into()
}

fn provider_usage(app: &App, provider: Provider, compact: bool) -> Element<'_, Msg> {
    let c = app.colors();
    let label = app.usage_label(provider);
    let title = match provider {
        Provider::Codex => "Codex",
        Provider::Claude => "Claude",
    };
    let mut content = column![
        row![
            provider_mark(provider, 12., c),
            text(title).size(10).font(semibold()).color(c.muted),
            Space::new().width(Length::Fill),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        text(if compact {
            ellipsize(&label, 31)
        } else {
            label.clone()
        })
        .size(10)
        .color(c.faint),
    ]
    .spacing(6);
    if let Some(fraction) = app.usage_fraction(provider) {
        content = content.push(usage_meter(fraction, c));
    }
    if compact {
        tooltip(
            button(content)
                .padding([5, 9])
                .width(Length::Fill)
                .style(a::ghost)
                .on_press(Msg::SettingsTab(SettingsTab::Providers)),
            text(label).size(11),
            tooltip::Position::Right,
        )
        .gap(6)
        .into()
    } else {
        content.into()
    }
}

fn full_access_confirmation(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let card = container(column![
        row![icon(Kind::Lock, 20., c.text), text("Allow full access?").size(20).font(semibold()).color(c.text)]
            .spacing(10).align_y(Alignment::Center),
        text("Full access lets this provider read and change files outside the selected project and run commands with broader access to your computer.")
            .size(13).color(c.muted),
        text("Tool approval requests still require your approval. Only continue for work and files you trust this provider to access.")
            .size(12).color(c.muted),
        row![Space::new().width(Length::Fill),
            compact_button("Cancel", Msg::CancelFullAccess).style(a::primary),
            compact_button("Allow full access", Msg::ConfirmFullAccess),
        ].spacing(8).align_y(Alignment::Center),
    ].spacing(17)).padding(24).max_width(490).width(Length::Fill).style(move |_| a::panel(c, 14.));
    opaque(
        container(card)
            .padding(28)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| a::flat(Color::from_rgba(0., 0., 0., 0.56))),
    )
}

fn message_view<'a>(app: &'a App, message: &'a Message, streaming: bool) -> Element<'a, Msg> {
    let c = app.colors();
    let is_user = message.role == "user";
    let body: Element<'a, Msg> = if let Some((_, content)) = app.rendered.get(&message.id) {
        markdown::view(
            content.items(),
            markdown::Settings::with_text_size(
                app.engine
                    .as_ref()
                    .map(|e| e.settings.chat_text_size)
                    .unwrap_or(15),
                app.theme(),
            ),
        )
        .map(|uri| Msg::OpenLink(uri.to_string()))
    } else {
        text(&message.text)
            .size(
                app.engine
                    .as_ref()
                    .map(|e| e.settings.chat_text_size)
                    .unwrap_or(15),
            )
            .color(c.text)
            .into()
    };
    if is_user {
        let bubble = container(body)
            .max_width(670)
            .padding([14, 18])
            .style(move |_| {
                let mut s = a::panel(c, 14.);
                s.background = Some(c.raised.into());
                s.border.width = 0.;
                s
            });
        column![
            row![
                Space::new().width(Length::Fill),
                text("You").size(11).font(semibold()).color(c.muted),
                text(
                    if app
                        .engine
                        .as_ref()
                        .is_ok_and(|e| e.settings.show_timestamps)
                    {
                        super::ago_label(message.created)
                    } else {
                        String::new()
                    }
                )
                .size(10)
                .color(c.faint),
                icon_button(Kind::Copy, "Copy message", Msg::Copy(message.id.clone()), c)
            ]
            .spacing(7)
            .align_y(Alignment::Center),
            container(bubble).align_right(Length::Fill),
        ]
        .spacing(3)
        .into()
    } else {
        column![
            row![
                icon(Kind::Bolt, 17., c.text),
                text(if message.role == "assistant" {
                    "Spark"
                } else {
                    "Workspace"
                })
                .size(12)
                .font(semibold())
                .color(c.text),
                text(if streaming { "Working" } else { "" })
                    .size(10)
                    .color(c.faint),
                text(
                    if !streaming
                        && app
                            .engine
                            .as_ref()
                            .is_ok_and(|e| e.settings.show_timestamps)
                    {
                        super::ago_label(message.created)
                    } else {
                        String::new()
                    }
                )
                .size(10)
                .color(c.faint),
                Space::new().width(Length::Fill),
                icon_button(
                    Kind::Copy,
                    "Copy response",
                    Msg::Copy(message.id.clone()),
                    c
                ),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
            container(body).padding([4, 0]),
        ]
        .spacing(8)
        .into()
    }
}

fn composer(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let provider = app.selected_provider();
    let running = app
        .selected
        .as_ref()
        .is_some_and(|id| e.jobs.contains_key(id));
    let models = e.models.get(provider);
    let model_name = app.model_name();
    let model_control: Element<'_, Msg> = if app.provider_ready(provider) && !models.is_empty() {
        row![
            provider_mark(provider, 15., c),
            pick_list(
                models,
                models.iter().find(|model| *model == &app.model),
                Msg::Model
            )
            .placeholder(model_name)
            .text_size(11)
            .padding([5, 6])
            .style(a::picker)
            .width(165),
        ]
        .spacing(3)
        .align_y(Alignment::Center)
        .into()
    } else {
        let mut model = row![
            provider_mark(provider, 15., c),
            text(model_name).size(11).color(c.muted)
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        if !app.provider_ready(provider) {
            model = model.push(icon(Kind::Lock, 12., c.faint));
        }
        tooltip(
            button(model)
                .padding([5, 7])
                .style(a::ghost)
                .on_press(Msg::SettingsTab(SettingsTab::Providers)),
            text(app.provider_hint(provider)).size(11),
            tooltip::Position::Top,
        )
        .gap(6)
        .into()
    };
    let send = if running {
        button(
            container(icon(Kind::Close, 17., c.inverse))
                .center_x(31)
                .center_y(31),
        )
        .padding(0)
        .style(|theme, status| {
            let mut style = a::primary(theme, status);
            style.border.radius = 16.into();
            style
        })
        .on_press(Msg::Cancel(app.selected.clone().unwrap()))
    } else {
        button(
            container(icon(
                Kind::ArrowUp,
                18.,
                if app.can_send() { c.inverse } else { c.faint },
            ))
            .center_x(31)
            .center_y(31),
        )
        .padding(0)
        .style(move |theme, status| {
            let mut style = a::primary(theme, status);
            style.border.radius = 16.into();
            if matches!(status, iced::widget::button::Status::Disabled) {
                style.background = Some(c.raised.into());
            }
            style
        })
        .on_press_maybe(app.can_send().then_some(Msg::Send))
    };
    let divider = || -> Element<'_, Msg> {
        container(Space::new().width(1).height(16))
            .style(move |_| a::flat(c.border))
            .into()
    };
    let mut controls = row![model_control].spacing(6).align_y(Alignment::Center);
    let efforts = app.supported_efforts();
    if !efforts.is_empty() {
        controls = controls.push(divider()).push(tooltip(
            pick_list(
                efforts,
                (!app.effort.is_empty()).then(|| app.effort.clone()),
                Msg::Effort,
            )
            .placeholder("Reasoning")
            .text_size(10)
            .padding([5, 6])
            .style(a::picker),
            text("Reasoning effort").size(11),
            tooltip::Position::Top,
        ));
    }
    let access = app.access_choices();
    if !access.is_empty() {
        controls = controls.push(divider()).push(tooltip(
            pick_list(access, Some(app.access.clone()), Msg::AccessRequested)
                .text_size(10)
                .padding([5, 6])
                .style(a::picker),
            text("Workspace access").size(11),
            tooltip::Position::Top,
        ));
    }
    let send_hint = if running {
        "Stop this agent".to_owned()
    } else if !app.provider_ready(provider) {
        app.provider_hint(provider)
    } else if app.prompt.trim().is_empty() {
        "Write a message to send".to_owned()
    } else if !app.can_send() {
        app.send_hint()
    } else {
        "Send message".to_owned()
    };
    let controls = row![
        controls.width(Length::Fill).wrap(),
        tooltip(send, text(send_hint).size(11), tooltip::Position::Top).gap(6),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let mut content = column![].spacing(3);
    content = content
        .push(
            text_editor(&app.editor)
                .placeholder(if app.project.is_some() {
                    "Ask about your project, or describe a change…"
                } else {
                    "Ask anything, or start with an idea…"
                })
                .on_action(Msg::Edit)
                .key_binding(super::composer_binding)
                .size(e.settings.chat_text_size)
                .height(67)
                .padding([8, 3])
                .style(a::editor),
        )
        .push(controls);
    let card = container(content)
        .padding([14, 16])
        .style(move |_| a::composer(c));
    let notice = if !app.notice.is_empty() {
        app.notice.as_str()
    } else {
        e.notice.as_str()
    };
    let mut footer = row![
        text(ellipsize(notice, 140))
            .size(10)
            .color(c.faint)
            .width(Length::Fill),
        text("Enter to send · Shift+Enter for new line")
            .size(10)
            .color(c.faint)
    ]
    .spacing(10);
    if !notice.is_empty() {
        footer = footer.push(icon_button(
            Kind::Close,
            "Dismiss notification",
            Msg::DismissNotice,
            c,
        ));
    }
    container(
        container(column![card, container(footer).padding([7, 4])].spacing(3))
            .max_width(860)
            .width(Length::Fill),
    )
    .padding([12, 25])
    .center_x(Length::Fill)
    .into()
}

fn activity(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let header = row![
        text("Agent activity")
            .size(13)
            .font(semibold())
            .color(c.text),
        Space::new().width(Length::Fill),
        icon_button(Kind::Close, "Close activity", Msg::ToggleAgents, c)
    ]
    .align_y(Alignment::Center);
    let mut content = column![
        text(format!(
            "{} of {} slots in use",
            e.jobs.len(),
            e.concurrency
        ))
        .size(11)
        .color(c.faint)
    ]
    .spacing(12);
    let selected_timeline = app.selected.as_ref().and_then(|id| e.timelines.get(id));
    if e.jobs.is_empty() && selected_timeline.is_none_or(|timeline| timeline.is_empty()) {
        content = content.push(
            container(
                column![
                    icon(Kind::Bolt, 24., c.faint),
                    text("Ready when you are")
                        .size(14)
                        .font(semibold())
                        .color(c.text),
                    text("Your agents will appear here when you send a message.")
                        .size(12)
                        .color(c.muted),
                    text("Separate projects can run together.")
                        .size(11)
                        .color(c.faint),
                ]
                .spacing(12),
            )
            .padding([28, 6]),
        );
    }
    for (id, job) in &e.jobs {
        let title = e
            .sessions
            .iter()
            .find(|s| &s.id == id)
            .map(|s| s.title.as_str())
            .unwrap_or("Agent");
        let mut card = column![
            button(text(ellipsize(title, 43)).size(12).font(semibold()))
                .padding(0)
                .style(a::ghost)
                .on_press(Msg::Select(id.clone())),
            text(&job.status).size(11).color(c.muted),
        ]
        .spacing(10);
        if let Some((_, description)) = job.approval.front() {
            card = card
                .push(line(c))
                .push(
                    text("Permission needed")
                        .size(11)
                        .font(semibold())
                        .color(c.text),
                )
                .push(text(description).size(11).color(c.muted))
                .push(
                    row![
                        compact_button("Allow once", Msg::Approve(id.clone(), true))
                            .style(a::primary),
                        compact_button("Deny", Msg::Approve(id.clone(), false))
                    ]
                    .spacing(6),
                );
        }
        if let Some(timeline) = e.timelines.get(id)
            && !timeline.is_empty()
        {
            card = card.push(line(c)).push(timeline_view(timeline, c));
        }
        if !job.usage.is_empty() {
            card = card.push(text(&job.usage).size(10).color(c.faint));
        }
        card = card.push(compact_button("Stop agent", Msg::Cancel(id.clone())));
        content = content.push(
            container(card)
                .padding(13)
                .width(Length::Fill)
                .style(move |_| a::panel(c, 9.)),
        );
    }
    if app
        .selected
        .as_ref()
        .is_some_and(|id| !e.jobs.contains_key(id))
        && let Some(timeline) = selected_timeline.filter(|timeline| !timeline.is_empty())
    {
        content = content.push(
            container(
                column![
                    text("Recent activity")
                        .size(12)
                        .font(semibold())
                        .color(c.text),
                    timeline_view(timeline, c)
                ]
                .spacing(12),
            )
            .padding(13)
            .width(Length::Fill)
            .style(move |_| a::panel(c, 9.)),
        );
    }
    container(
        column![
            header,
            line(c),
            scrollable(content).height(Length::Fill),
            text("Tool approvals stay in your hands.")
                .size(10)
                .color(c.faint),
        ]
        .spacing(14),
    )
    .padding(16)
    .width(278)
    .height(Length::Fill)
    .style(move |_| a::flat(c.side))
    .into()
}

fn timeline_view<'a>(
    entries: &'a std::collections::VecDeque<spark_code::provider_status::TimelineEntry>,
    c: Colors,
) -> Element<'a, Msg> {
    let mut content = column![].spacing(12);
    for entry in entries.iter().skip(entries.len().saturating_sub(10)) {
        let seconds = entry.elapsed_ms / 1_000;
        let elapsed = if seconds >= 60 {
            format!("{}m {:02}s", seconds / 60, seconds % 60)
        } else {
            format!("{seconds}s")
        };
        let mut event = column![
            row![
                text("·").size(14).color(c.faint),
                text(ellipsize(&entry.label, 72))
                    .size(11)
                    .font(semibold())
                    .color(c.muted)
                    .width(Length::Fill),
                text(elapsed).size(9).color(c.faint),
            ]
            .spacing(5)
            .align_y(Alignment::Center)
        ]
        .spacing(4);
        if !entry.detail.is_empty() {
            event = event.push(
                container(
                    text(ellipsize(&entry.detail.replace('\n', " "), 240))
                        .size(10)
                        .color(c.faint),
                )
                .padding([0, 10]),
            );
        }
        content = content.push(event);
    }
    content.into()
}

fn provider_row(app: &App, provider: Provider) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let state = e.providers.get(provider);
    let enabled = e.settings.enabled(provider);
    let name = super::provider_title(provider);
    let open = app.expanded == Some(provider);
    let dot_color = if !enabled {
        Color::from_rgb8(0xF5, 0xA6, 0x23)
    } else {
        match state.readiness {
            Readiness::Ready => Color::from_rgb8(0x22, 0xC5, 0x7B),
            Readiness::Checking | Readiness::Unchecked => Color::from_rgb8(0xF5, 0xA6, 0x23),
            _ => Color::from_rgb8(0xE5, 0x48, 0x4D),
        }
    };
    let dot = container(Space::new().width(8).height(8)).style(move |_| container::Style {
        background: Some(dot_color.into()),
        border: iced::Border {
            radius: 4.into(),
            color: c.bg,
            width: 1.5,
        },
        ..Default::default()
    });
    let mark = stack![
        container(provider_mark(provider, 18., c)).padding([4, 4]),
        dot
    ];
    let subtitle = if !enabled {
        format!("Disabled - {name} is disabled in Spark Code settings.")
    } else {
        match state.readiness {
            Readiness::Ready if !state.status.is_empty() => state.status.clone(),
            Readiness::Ready => "Connected".into(),
            Readiness::Checking => "Checking the official CLI connection…".into(),
            Readiness::Unchecked => "Not checked yet. Use refresh to check this provider.".into(),
            _ => state.status.clone(),
        }
    };
    let mut title = row![text(name).size(14).font(semibold()).color(c.text)]
        .spacing(8)
        .align_y(Alignment::Center);
    if !state.version.is_empty() {
        title = title.push(
            text(format!("v{}", state.version))
                .size(11)
                .font(Font::MONOSPACE)
                .color(c.faint),
        );
    }
    let toggle = iced::widget::toggler(enabled)
        .size(20)
        .on_toggle(move |value| Msg::ToggleProvider(provider, value))
        .style(move |_, status| {
            let on = matches!(
                status,
                iced::widget::toggler::Status::Active { is_toggled: true }
                    | iced::widget::toggler::Status::Hovered { is_toggled: true }
                    | iced::widget::toggler::Status::Disabled { is_toggled: true }
            );
            iced::widget::toggler::Style {
                background: if on {
                    Color::from_rgb8(0x3B, 0x6E, 0xF6).into()
                } else {
                    c.raised.into()
                },
                background_border_width: 1.,
                background_border_color: if on { Color::TRANSPARENT } else { c.border },
                foreground: if on {
                    Color::WHITE.into()
                } else {
                    c.muted.into()
                },
                foreground_border_width: 0.,
                foreground_border_color: Color::TRANSPARENT,
                text_color: None,
                border_radius: None,
                padding_ratio: 0.2,
            }
        });
    let head = row![
        mark,
        column![title, text(subtitle).size(12).color(c.muted)]
            .spacing(4)
            .width(Length::Fill),
        button(
            container(icon(
                if open { Kind::ChevronUp } else { Kind::Chevron },
                16.,
                c.muted
            ))
            .center_x(30)
            .center_y(28),
        )
        .padding(0)
        .style(a::ghost)
        .on_press(Msg::ToggleExpand(provider)),
        toggle,
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let mut body = column![head].spacing(14);
    if open {
        let path = match provider {
            Provider::Codex => &e.settings.codex_path,
            Provider::Claude => &e.settings.claude_path,
        };
        let input = text_input(provider.cli(), path)
            .size(13)
            .padding([9, 11])
            .style(a::input)
            .on_input(move |value| match provider {
                Provider::Codex => Msg::CodexPath(value),
                Provider::Claude => Msg::ClaudePath(value),
            });
        body = body.push(
            column![
                text("Binary path").size(12).font(semibold()).color(c.text),
                input,
                text(format!("Path to the {name} binary used by Spark Code."))
                    .size(11)
                    .color(c.faint),
            ]
            .spacing(7),
        );
        if !state.usage_windows.is_empty() {
            body = body.push(provider_usage(app, provider, false));
        }
        body = body.push(
            row![
                compact_button(
                    if provider == Provider::Codex {
                        "Sign in with ChatGPT"
                    } else {
                        "Sign in to Claude"
                    },
                    Msg::Login(provider)
                ),
                Space::new().width(Length::Fill),
                compact_button(
                    if app.selected_provider() == provider {
                        "Selected for chat"
                    } else {
                        "Use for chat"
                    },
                    Msg::Provider(provider)
                )
                .on_press_maybe(
                    (app.selected_provider() != provider).then_some(Msg::Provider(provider))
                )
                .style(if app.selected_provider() == provider {
                    a::selected
                } else {
                    a::outline
                }),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    container(body).padding([4, 0]).into()
}

fn settings(app: &App) -> Element<'_, Msg> {
    settings_ui::view(app)
}

fn import_review(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let backup = app.preview.as_ref().unwrap();
    let mut choices = column![].spacing(8);
    for session in &backup.sessions {
        let project = backup
            .projects
            .iter()
            .find(|p| p.id == session.project_id)
            .map(|p| p.name.as_str())
            .unwrap_or("Project");
        let count = backup
            .messages
            .iter()
            .filter(|m| m.session_id == session.id)
            .count();
        let chosen = app.chosen.contains(&session.id);
        let check = checkbox(chosen).on_toggle({
            let id = session.id.clone();
            move |value| Msg::ToggleImport(id.clone(), value)
        });
        choices = choices.push(
            container(
                row![
                    check,
                    column![
                        text(&session.title).size(13).font(semibold()).color(c.text),
                        row![
                            icon(Kind::Folder, 12., c.faint),
                            text(project).size(11).color(c.muted),
                            text("·").size(11).color(c.faint),
                            text(format!("{count} messages")).size(11).color(c.faint)
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    ]
                    .spacing(6),
                    Space::new().width(Length::Fill),
                    text(session.provider.to_string()).size(10).color(c.faint)
                ]
                .spacing(13)
                .align_y(Alignment::Center),
            )
            .padding(15)
            .width(Length::Fill)
            .style(move |_| {
                let mut style = a::panel(c, 9.);
                if chosen {
                    style.background = Some(c.raised.into());
                }
                style
            }),
        );
    }
    let header = column![
        text("Bring your conversations along")
            .size(25)
            .font(semibold())
            .color(c.text),
        text(format!(
            "{} projects  ·  {} conversations  ·  {} messages",
            backup.projects.len(),
            backup.sessions.len(),
            backup.messages.len()
        ))
        .size(12)
        .color(c.muted),
        text("Choose the conversations to add. Your source data stays unchanged.")
            .size(12)
            .color(c.faint),
    ]
    .spacing(10);
    let footer = row![
        text(format!("{} selected", app.chosen.len()))
            .size(12)
            .color(c.muted),
        Space::new().width(Length::Fill),
        compact_button("Cancel", Msg::CloseImport),
        button(text(format!("Import {} conversations", app.chosen.len())).size(12))
            .padding([10, 15])
            .style(a::primary)
            .on_press_maybe((!app.chosen.is_empty()).then_some(Msg::ConfirmImport)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    container(
        container(
            column![
                header,
                Space::new().height(8),
                scrollable(choices).height(Length::Fill),
                line(c),
                footer
            ]
            .spacing(18),
        )
        .max_width(820)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .padding(30)
    .center_x(Length::Fill)
    .height(Length::Fill)
    .into()
}
