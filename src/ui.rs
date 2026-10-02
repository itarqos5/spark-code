//! Native workspace presentation. State and actions live in `gui`.
use super::{App, Msg};
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
        progress_bar, row, scrollable, stack, text, text_editor, text_input, tooltip,
    },
};
use spark_code::model::{Message, Provider};

fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..Font::with_name("Inter Variable")
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
            if app.sidebar_visible {
                workspace = workspace.push(sidebar(app)).push(vertical_line(c));
            }
            workspace = workspace.push(column![toolbar(app), line(c), center].width(Length::Fill));
            if app.show_agents {
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
            icon_button(Kind::Settings, "Settings", Msg::Settings, c),
            icon_button(Kind::Panel, "Agent activity", Msg::ToggleAgents, c),
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

fn sidebar(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let heading = row![
        text("Workspace").size(13).font(semibold()).color(c.text),
        Space::new().width(Length::Fill),
        icon_button(Kind::Panel, "Hide sidebar", Msg::ToggleSidebar, c),
    ]
    .align_y(Alignment::Center);
    let new_chat = button(
        row![
            icon(Kind::Plus, 15., c.text),
            text("New chat").size(12),
            Space::new().width(Length::Fill)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(Msg::New)
    .padding([9, 10])
    .width(Length::Fill)
    .style(a::outline);
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
    let mut projects = column![
        row![
            section_label("PROJECTS", c),
            Space::new().width(Length::Fill),
            icon_button(Kind::Plus, "Add project folder", Msg::AddProject, c)
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(3);
    projects = projects.push(
        button(
            row![
                icon(
                    Kind::Chat,
                    15.,
                    if app.project.is_none() {
                        c.text
                    } else {
                        c.muted
                    }
                ),
                text("General chat").size(12),
                Space::new().width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([8, 9])
        .style(if app.project.is_none() {
            a::selected
        } else {
            a::ghost
        })
        .on_press(Msg::GeneralChat),
    );
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
    for s in &e.sessions {
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
    if e.sessions.is_empty() {
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
    let mut bottom = column![
        provider_usage(app, Provider::Codex, true),
        provider_usage(app, Provider::Claude, true),
        line(c)
    ]
    .spacing(9);
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
            projects,
            Space::new().height(14),
            section_label("CONVERSATIONS", c),
            scrollable(sessions).height(Length::Fill),
            bottom,
        ]
        .spacing(8),
    )
    .padding([12, 12])
    .width(230)
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
        "Settings".to_owned()
    } else {
        session
            .map(|s| ellipsize(&s.title.replace('\n', " "), 43))
            .unwrap_or_else(|| "New chat".into())
    };
    let mut items = row![].spacing(11).align_y(Alignment::Center);
    if !app.sidebar_visible {
        items = items.push(icon_button(
            Kind::Panel,
            "Show sidebar",
            Msg::ToggleSidebar,
            c,
        ));
    }
    items = items
        .push(icon(Kind::Folder, 14., c.faint))
        .push(
            text(
                project
                    .map(|p| ellipsize(&p.name, 20))
                    .unwrap_or_else(|| "General chat".into()),
            )
            .size(12)
            .color(c.muted),
        )
        .push(text("/").size(12).color(c.faint))
        .push(text(title).size(12).color(c.text))
        .push(Space::new().width(Length::Fill));
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
        let mut messages = column![].spacing(28);
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
        .height(Length::Fill)
        .into()
    };
    let mut main = column![transcript].height(Length::Fill);
    if app
        .selected
        .as_ref()
        .and_then(|id| e.sessions.iter().find(|s| &s.id == id))
        .is_some_and(|s| s.id.starts_with("t3:") && s.remote_id.is_none())
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
    main.push(composer(app)).into()
}

fn welcome(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let project = app
        .project
        .as_ref()
        .and_then(|id| e.projects.iter().find(|p| &p.id == id));
    let context: Element<'_, Msg> = if let Some(project) = project {
        tooltip(
            container(
                row![
                    icon(Kind::Folder, 13., c.muted),
                    text(ellipsize(&project.name, 52)).size(12).color(c.muted),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
            )
            .padding([6, 10]),
            text(&project.path).size(11),
            tooltip::Position::Bottom,
        )
        .gap(5)
        .into()
    } else {
        text("General chat").size(12).color(c.faint).into()
    };
    let content = column![
        container(icon(Kind::Bolt, 32., c.text)).center_x(Length::Fill),
        Space::new().height(8),
        text("What are we building next?")
            .size(29)
            .font(semibold())
            .color(c.text),
        context,
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

/// Simple original provider marks, not provider logos.
fn provider_mark<'a>(provider: Provider, size: f32, c: Colors) -> Element<'a, Msg> {
    match provider {
        Provider::Codex => icon(Kind::Bolt, size, c.muted),
        Provider::Claude => container(text("A").font(semibold()).size(size - 1.).color(c.muted))
            .center_x(size)
            .center_y(size)
            .into(),
    }
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
                .on_press(Msg::Settings),
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
            markdown::Settings::with_text_size(15, &app.theme()),
        )
        .map(|uri| Msg::OpenLink(uri.to_string()))
    } else {
        text(&message.text).size(15).color(c.text).into()
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
                icon_button(
                    Kind::Copy,
                    "Copy message",
                    Msg::Copy(message.text.clone()),
                    c
                )
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
                Space::new().width(Length::Fill),
                icon_button(
                    Kind::Copy,
                    "Copy response",
                    Msg::Copy(message.text.clone()),
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
    let project = app
        .project
        .as_ref()
        .and_then(|id| e.projects.iter().find(|p| &p.id == id));
    let running = app
        .selected
        .as_ref()
        .is_some_and(|id| e.jobs.contains_key(id));
    let models = e.models.get(provider);
    let model_name = app.model_name();
    let model_control: Element<'_, Msg> = if app.provider_ready(provider) && !models.is_empty() {
        row![
            provider_mark(provider, 13., c),
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
            provider_mark(provider, 13., c),
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
                .on_press(Msg::Settings),
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
        .style(a::primary)
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
            if matches!(status, iced::widget::button::Status::Disabled) {
                style.background = Some(c.raised.into());
            }
            style
        })
        .on_press_maybe(app.can_send().then_some(Msg::Send))
    };
    let mut controls = row![model_control].spacing(5).align_y(Alignment::Center);
    let efforts = app.supported_efforts();
    if !efforts.is_empty() {
        controls = controls.push(tooltip(
            pick_list(efforts, Some(app.effort.clone()), Msg::Effort)
                .text_size(10)
                .padding([5, 6])
                .style(a::picker),
            text("Reasoning effort").size(11),
            tooltip::Position::Top,
        ));
    }
    let access = app.access_choices();
    if !access.is_empty() {
        controls = controls.push(tooltip(
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
    if let Some(project) = project {
        content = content.push(
            tooltip(
                container(
                    row![
                        icon(Kind::Folder, 12., c.muted),
                        text(ellipsize(&project.name, 36)).size(10).color(c.muted)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([5, 8])
                .style(move |_| {
                    let mut style = a::panel(c, 6.);
                    style.border.width = 0.;
                    style.background = Some(c.raised.into());
                    style
                }),
                text(&project.path).size(11),
                tooltip::Position::Top,
            )
            .gap(5),
        );
    }
    content = content
        .push(
            text_editor(&app.editor)
                .placeholder("Ask anything…")
                .on_action(Msg::Edit)
                .key_binding(super::composer_binding)
                .size(14)
                .height(67)
                .padding([8, 3])
                .style(a::editor),
        )
        .push(controls);
    let card = container(content)
        .padding(12)
        .style(move |_| a::composer(c));
    let notice = if !app.notice.is_empty() {
        app.notice.as_str()
    } else {
        e.notice.as_str()
    };
    let footer = row![
        text(ellipsize(notice, 78)).size(10).color(c.faint),
        Space::new().width(Length::Fill),
        text("Enter to send · Shift+Enter for new line")
            .size(10)
            .color(c.faint)
    ]
    .spacing(10);
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
        if let Some(timeline) = e.timelines.get(id) {
            if !timeline.is_empty() {
                card = card.push(line(c)).push(timeline_view(timeline, c));
            }
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
    {
        if let Some(timeline) = selected_timeline.filter(|timeline| !timeline.is_empty()) {
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

fn settings(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let mut connections = column![
        text("Provider connections")
            .size(15)
            .font(semibold())
            .color(c.text),
        text("Use your account through the official command-line app.")
            .size(12)
            .color(c.muted),
    ]
    .spacing(10);
    for provider in Provider::ALL {
        let (title, description, path) = match provider {
            Provider::Codex => ("Codex / ChatGPT", "OpenAI account", &e.settings.codex_path),
            Provider::Claude => ("Claude", "Anthropic account", &e.settings.claude_path),
        };
        let input = text_input(provider.cli(), path)
            .size(12)
            .padding(9)
            .style(a::input)
            .on_input(move |value| {
                if provider == Provider::Codex {
                    Msg::CodexPath(value)
                } else {
                    Msg::ClaudePath(value)
                }
            });
        let card = container(
            column![
                row![
                    container(provider_mark(provider, 18., c))
                        .padding(10)
                        .style(move |_| a::panel(c, 8.)),
                    column![
                        text(title).size(13).font(semibold()).color(c.text),
                        text(description).size(11).color(c.muted)
                    ]
                    .spacing(3),
                    Space::new().width(Length::Fill),
                    compact_button("Setup guide", Msg::OpenDocs(provider))
                ]
                .spacing(12)
                .align_y(Alignment::Center),
                row![
                    icon(
                        if app.provider_ready(provider) {
                            Kind::Bolt
                        } else {
                            Kind::Lock
                        },
                        13.,
                        c.muted
                    ),
                    text(app.provider_hint(provider)).size(11).color(c.muted)
                ]
                .spacing(7)
                .align_y(Alignment::Center),
                provider_usage(app, provider, false),
                row![text("Executable").size(11).color(c.muted).width(78), input]
                    .spacing(9)
                    .align_y(Alignment::Center),
                row![
                    compact_button("Sign in", Msg::Login(provider)),
                    button(
                        row![icon(Kind::Refresh, 12., c.muted), text("Refresh").size(11)]
                            .spacing(6)
                            .align_y(Alignment::Center)
                    )
                    .padding([7, 10])
                    .style(a::ghost)
                    .on_press_maybe(app.probe_receiver.is_none().then_some(Msg::Probe(provider))),
                    Space::new().width(Length::Fill),
                    compact_button(
                        if app.selected_provider() == provider {
                            "Selected for chat"
                        } else {
                            "Use for chat"
                        },
                        Msg::Provider(provider)
                    )
                    .style(if app.selected_provider() == provider {
                        a::selected
                    } else {
                        a::outline
                    }),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            ]
            .spacing(13),
        )
        .padding(16)
        .width(Length::Fill)
        .style(move |_| a::panel(c, 11.));
        connections = connections.push(card);
    }
    connections = connections
        .push(
            text("Sign-in opens the official CLI. Spark Code doesn't store account credentials.")
                .size(11)
                .color(c.faint),
        )
        .push(compact_button("Save connection settings", Msg::SaveSettings).style(a::primary));
    let appearance = container(
        column![
            row![
                column![
                    text("Appearance").size(13).font(semibold()).color(c.text),
                    text("A quiet workspace, in your preferred light.")
                        .size(11)
                        .color(c.muted)
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                compact_button(
                    if e.settings.light_theme {
                        "Switch to dark"
                    } else {
                        "Switch to light"
                    },
                    Msg::ToggleTheme
                ),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            line(c),
            row![
                column![
                    text("Reduced motion").size(12).color(c.text),
                    text("Keep transitions immediate.").size(11).color(c.muted)
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                checkbox(e.settings.reduced_motion).on_toggle(Msg::ReducedMotion),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(15),
    )
    .padding(16)
    .width(Length::Fill)
    .style(move |_| a::panel(c, 11.));
    let capacity = container(column![row![column![text("Concurrent agents").size(13).font(semibold()).color(c.text),
        text("Run up to four agents in separate project folders.").size(11).color(c.muted)].spacing(5),
        Space::new().width(Length::Fill), text_input("2", &e.concurrency.to_string()).on_input(Msg::Concurrency)
            .size(12).padding(8).width(54).style(a::input),
    ].spacing(12).align_y(Alignment::Center),
        text("More agents use more memory and subscription capacity. Each folder is limited to one active agent.").size(11).color(c.faint),
    ].spacing(12)).padding(16).width(Length::Fill).style(move |_| a::panel(c, 11.));
    let imports = container(column![text("Conversation history").size(13).font(semibold()).color(c.text),
        text("Preview and choose what to bring with you. Nothing is imported automatically.").size(12).color(c.muted),
        row![button(text("Choose backup file…").size(12)).padding([8, 11]).style(a::outline)
                .on_press_maybe((!app.importing).then_some(Msg::ImportFile)),
            button(text("Preview Codex history").size(12)).padding([8, 11]).style(a::outline)
                .on_press_maybe((!app.importing).then_some(Msg::ImportCodex)),
        ].spacing(8),
        line(c),
        row![text("Keep a private copy of your conversations.").size(11).color(c.muted), Space::new().width(Length::Fill),
            compact_button("Export history", Msg::Export)].spacing(8).align_y(Alignment::Center),
        text("T3 backups preserve messages and project paths, then start fresh provider sessions.").size(11).color(c.faint),
    ].spacing(13)).padding(16).width(Length::Fill).style(move |_| a::panel(c, 11.));
    let privacy = container(column![text("Private by default").size(12).font(semibold()).color(c.muted),
        text("Your local transcripts and exported backups are unencrypted and include messages and project paths. Keep backups private.").size(11).color(c.faint),
        text("Claude Code on native Windows has no OS-level sandbox. Review tool requests carefully.").size(11).color(c.faint),
        text("Spark Code is independent of OpenAI and Anthropic.").size(10).color(c.faint),
    ].spacing(8)).padding([6, 2]);
    let content = column![
        text("Make it yours")
            .size(25)
            .font(semibold())
            .color(c.text),
        text("Your workspace, tools, and preferences.")
            .size(13)
            .color(c.muted),
        Space::new().height(8),
        connections,
        Space::new().height(5),
        text("Workspace").size(15).font(semibold()).color(c.text),
        appearance,
        capacity,
        Space::new().height(5),
        text("Your data").size(15).font(semibold()).color(c.text),
        imports,
        text(if app.importing {
            "Reading selected history…"
        } else {
            &app.notice
        })
        .size(11)
        .color(c.muted),
        privacy,
    ]
    .spacing(15);
    scrollable(
        container(container(content).max_width(760).width(Length::Fill))
            .padding([30, 30])
            .center_x(Length::Fill),
    )
    .height(Length::Fill)
    .into()
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
