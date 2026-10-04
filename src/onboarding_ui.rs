//! Three optional setup steps with a finite, reduced-motion-aware native animation.
use super::*;
use iced::widget::column;
use iced::{
    Point, Rectangle, Renderer, mouse,
    widget::{
        canvas::{self, Canvas, Frame, Geometry, Path, Stroke},
        toggler,
    },
};

pub(super) fn view(app: &App) -> Element<'_, Msg> {
    let c = app.colors();
    let e = app.engine.as_ref().unwrap();
    let s = &e.settings;
    let step = app.onboarding.unwrap_or(0);
    let progress = if s.reduced_motion {
        1.
    } else {
        app.onboarding_started
            .map(|t| (t.elapsed().as_secs_f32() / 0.8).min(1.))
            .unwrap_or(1.)
    };
    let eased = 1. - (1. - progress).powi(3);
    let (title, detail) = match step {
        0 => (
            "Your next idea starts here.",
            "A quiet workspace for your agents, projects, and conversations. Make it yours in a few quick steps.",
        ),
        1 => (
            "Bring your agents.",
            "Connect through the official Codex or Claude Code CLI. Your existing subscription and sign-in stay with your agent.",
        ),
        _ => (
            "Pick up where you left off.",
            "Bring your local conversation history into one workspace. Choose your sources, then keep them in sync.",
        ),
    };
    let mut content = column![
        text(format!("SETUP  /  0{}", step + 1))
            .size(11)
            .font(semibold())
            .color(c.faint),
        text(title).size(32).font(semibold()).color(c.text),
        text(detail).size(14).color(c.muted),
        Space::new().height(8)
    ]
    .spacing(14);
    match step {
        0 => {
            content = content
                .push(text("Choose your light").size(13).font(semibold()))
                .push(
                    row![
                        button(
                            column![icon(Kind::Moon, 25., c.text), text("Dark").size(14)]
                                .spacing(14)
                        )
                        .width(Length::Fill)
                        .padding(22)
                        .style(if !s.light_theme {
                            a::selected
                        } else {
                            a::outline
                        })
                        .on_press_maybe(s.light_theme.then_some(Msg::ToggleTheme)),
                        button(
                            column![icon(Kind::Sun, 25., c.text), text("Light").size(14)]
                                .spacing(14)
                        )
                        .width(Length::Fill)
                        .padding(22)
                        .style(if s.light_theme {
                            a::selected
                        } else {
                            a::outline
                        })
                        .on_press_maybe((!s.light_theme).then_some(Msg::ToggleTheme)),
                    ]
                    .spacing(12),
                )
                .push(
                    row![
                        column![
                            text("Less motion").size(13),
                            text("Keep transitions instant and still.")
                                .size(12)
                                .color(c.muted)
                        ]
                        .spacing(5)
                        .width(Length::Fill),
                        toggler(s.reduced_motion)
                            .size(20)
                            .on_toggle(Msg::ReducedMotion)
                    ]
                    .align_y(Alignment::Center)
                    .spacing(20),
                );
        }
        1 => {
            for provider in Provider::ALL {
                let state = e.providers.get(provider);
                content = content.push(
                    container(
                        row![
                            icon(Kind::Terminal, 22., c.muted),
                            column![
                                text(provider.to_string()).size(14).font(semibold()),
                                text(state.readiness.to_string()).size(12).color(c.muted)
                            ]
                            .spacing(6)
                            .width(Length::Fill),
                            compact_button("Sign in", Msg::Login(provider)),
                        ]
                        .spacing(14)
                        .align_y(Alignment::Center),
                    )
                    .padding(18)
                    .style(move |_| a::panel(c, 8.)),
                );
            }
            content = content
                .push(
                    row![
                        compact_button("Find installed CLIs", Msg::DetectCli),
                        compact_button("Refresh connections", Msg::ProbeAll)
                    ]
                    .spacing(8),
                )
                .push(
                    text("You can connect later in Settings → Providers.")
                        .size(12)
                        .color(c.faint),
                )
                .push(compact_button("Choose a project folder", Msg::AddProject));
            if let Some(project) = app
                .project
                .as_ref()
                .and_then(|id| e.projects.iter().find(|p| &p.id == id))
            {
                content = content.push(
                    text(format!("Project ready: {}", project.name))
                        .size(12)
                        .color(c.muted),
                );
            }
        }
        _ => {
            content=content.push(super::settings_ui::history_sources(app))
                .push(row![column![text("Keep history in sync").size(13),text("On launch, then every minute.").size(12).color(c.muted)].spacing(5).width(Length::Fill),toggler(s.auto_import_history).size(20).on_toggle(Msg::AutoImportHistory)].align_y(Alignment::Center).spacing(16))
                .push(row![button(text(if app.history_receiver.is_some() {"Working…"} else {"Import now"}).size(12)).padding([9,12]).style(a::outline).on_press_maybe(app.history_receiver.is_none().then_some(Msg::SyncHistory)),text(&app.history_status).size(11).color(c.muted)].spacing(12).align_y(Alignment::Center))
                .push(text("Codex and T3 Code import directly from live databases. No export file needed.").size(11).color(c.faint));
        }
    }
    if !app.notice.is_empty() {
        content = content.push(text(ellipsize(&app.notice, 180)).size(12).color(c.muted));
    }
    let mut steps = row![].spacing(6);
    for i in 0..3 {
        steps = steps.push(
            button(
                container(Space::new().height(3).width(34))
                    .style(move |_| a::flat(if i == step { c.text } else { c.border })),
            )
            .padding([9, 0])
            .style(a::ghost)
            .on_press(Msg::OnboardingStep(i)),
        );
    }
    let left = container(
        column![
            icon(Kind::Bolt, 32., c.text),
            Space::new().height(12),
            text("One spark.\nRoom to build.")
                .size(32)
                .font(semibold())
                .color(c.text),
            text("Your tools. Your history.\nA workspace that stays with you.")
                .size(13)
                .color(c.muted),
            Canvas::new(SparkMotion { progress, c })
                .width(244)
                .height(180),
            steps,
            text("You can change everything later.")
                .size(11)
                .color(c.faint),
        ]
        .spacing(14),
    )
    .padding(28)
    .width(300)
    .height(Length::Fill)
    .style(move |_| a::flat(c.side));
    let back: Element<'_, Msg> = if step > 0 {
        compact_button("Back", Msg::OnboardingStep(step - 1)).into()
    } else {
        Space::new().width(0).into()
    };
    let footer = row![
        back,
        Space::new().width(Length::Fill),
        compact_button("Skip setup", Msg::FinishOnboarding),
        button(
            text(if step == 2 {
                "Open workspace"
            } else {
                "Continue"
            })
            .size(13)
        )
        .padding([11, 18])
        .style(a::primary)
        .on_press(if step == 2 {
            Msg::FinishOnboarding
        } else {
            Msg::OnboardingStep(step + 1)
        })
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let right = container(column![
        scrollable(
            container(content)
                .width(Length::Fill)
                .padding(iced::Padding {
                    top: (1. - eased) * 18.,
                    right: 16.,
                    ..Default::default()
                })
        )
        .height(Length::Fill),
        Space::new().height(18),
        footer,
    ])
    .padding(32)
    .width(Length::Fill)
    .height(Length::Fill);
    container(row![left, right].height(Length::Fill))
        .max_width(1060)
        .height(Length::Fill)
        .padding([22, 24])
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

struct SparkMotion {
    progress: f32,
    c: Colors,
}
impl canvas::Program<Msg> for SparkMotion {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        f.translate(iced::Vector::new(16., 0.));
        f.scale((bounds.height / 220.).min(1.));
        let p = 1. - (1. - self.progress).powi(3);
        let c = self.c;
        // A short assembled workspace: cards slide into place and wires draw toward the spark.
        for (i, (x, y, w, h)) in [
            (12., 25., 108., 61.),
            (139., 52., 108., 76.),
            (38., 140., 176., 56.),
        ]
        .into_iter()
        .enumerate()
        {
            let t = ((self.progress - i as f32 * 0.12) / 0.64).clamp(0., 1.);
            let t = 1. - (1. - t).powi(3);
            let y = y + (1. - t) * 30.;
            let rect = Path::rectangle(Point::new(x, y), iced::Size::new(w, h));
            f.fill(&rect, Color { a: t, ..c.surface });
            f.stroke(
                &rect,
                Stroke::default()
                    .with_color(Color { a: t, ..c.border })
                    .with_width(1.),
            );
            for line in 0..3 {
                f.fill_rectangle(
                    Point::new(x + 12., y + 15. + line as f32 * 10.),
                    iced::Size::new((w - 24.) * (if line == 2 { 0.55 } else { 0.8 }), 2.),
                    Color { a: t, ..c.muted },
                );
            }
        }
        let wire = Path::new(|b| {
            b.move_to(Point::new(65., 86.));
            b.line_to(Point::new(65., 112.));
            b.line_to(Point::new(130., 112.));
            b.line_to(Point::new(193., 112.));
            b.line_to(Point::new(193., 128.));
            b.move_to(Point::new(130., 112.));
            b.line_to(Point::new(130., 140.));
        });
        f.stroke(
            &wire,
            Stroke::default()
                .with_color(Color { a: p, ..c.muted })
                .with_width(1.),
        );
        let radius = 7. + (1. - p) * 20.;
        f.fill(
            &Path::circle(Point::new(130., 112.), radius),
            Color { a: p, ..c.text },
        );
        vec![f.into_geometry()]
    }
}
