use iced::{
    Background, Border, Color, Shadow, Theme, Vector,
    widget::{button, container, pick_list, text_editor, text_input},
};
#[derive(Clone, Copy)]
pub struct Colors {
    pub bg: Color,
    pub side: Color,
    pub surface: Color,
    pub raised: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub inverse: Color,
}
impl Colors {
    pub fn at(t: f32) -> Self {
        let mix = |d: u32, l: u32| {
            let c = |h: u32| Color::from_rgb8((h >> 16) as u8, (h >> 8) as u8, h as u8);
            let a = c(d);
            let b = c(l);
            Color {
                r: a.r + (b.r - a.r) * t,
                g: a.g + (b.g - a.g) * t,
                b: a.b + (b.b - a.b) * t,
                a: 1.,
            }
        };
        Self {
            bg: mix(0x0b0b0d, 0xfafafa),
            side: mix(0x111113, 0xf2f2f3),
            surface: mix(0x161619, 0xffffff),
            raised: mix(0x202024, 0xededee),
            border: mix(0x29292e, 0xe1e1e4),
            text: mix(0xf4f4f5, 0x171719),
            muted: mix(0xa1a1aa, 0x66666f),
            faint: mix(0x62626c, 0x9999a2),
            inverse: mix(0x121214, 0xffffff),
        }
    }
    pub fn from_theme(t: &Theme) -> Self {
        if t.palette().background.r > 0.5 {
            Self::at(1.)
        } else {
            Self::at(0.)
        }
    }
}
pub fn panel(c: Colors, radius: f32) -> container::Style {
    container::Style {
        background: Some(c.surface.into()),
        text_color: Some(c.text),
        border: Border {
            color: c.border,
            width: 1.,
            radius: radius.into(),
        },
        ..Default::default()
    }
}
pub fn flat(bg: Color) -> container::Style {
    container::Style {
        background: Some(bg.into()),
        ..Default::default()
    }
}
pub fn primary(t: &Theme, s: button::Status) -> button::Style {
    let c = Colors::from_theme(t);
    button::Style {
        background: Some(
            (if matches!(s, button::Status::Hovered | button::Status::Pressed) {
                c.muted
            } else {
                c.text
            })
            .into(),
        ),
        text_color: c.inverse,
        border: Border {
            radius: 8.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn ghost(t: &Theme, s: button::Status) -> button::Style {
    let c = Colors::from_theme(t);
    button::Style {
        background: matches!(s, button::Status::Hovered | button::Status::Pressed)
            .then_some(c.raised.into()),
        text_color: c.muted,
        border: Border {
            radius: 7.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn outline(t: &Theme, s: button::Status) -> button::Style {
    let c = Colors::from_theme(t);
    button::Style {
        background: Some(
            if matches!(s, button::Status::Hovered | button::Status::Pressed) {
                c.raised
            } else {
                c.surface
            }
            .into(),
        ),
        text_color: c.text,
        border: Border {
            radius: 8.into(),
            color: c.border,
            width: 1.,
        },
        ..Default::default()
    }
}
pub fn selected(t: &Theme, _: button::Status) -> button::Style {
    let c = Colors::from_theme(t);
    button::Style {
        background: Some(c.raised.into()),
        text_color: c.text,
        border: Border {
            radius: 7.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn close(t: &Theme, s: button::Status) -> button::Style {
    let mut st = ghost(t, s);
    if matches!(s, button::Status::Hovered | button::Status::Pressed) {
        st.background = Some(Color::from_rgb8(196, 43, 52).into());
        st.text_color = Color::WHITE;
    }
    st
}
pub fn input(t: &Theme, s: text_input::Status) -> text_input::Style {
    let c = Colors::from_theme(t);
    text_input::Style {
        background: c.bg.into(),
        border: Border {
            color: if matches!(s, text_input::Status::Focused { .. }) {
                c.muted
            } else {
                c.border
            },
            width: 1.,
            radius: 7.into(),
        },
        icon: c.muted,
        placeholder: c.faint,
        value: c.text,
        selection: c.raised,
    }
}
pub fn editor(t: &Theme, _: text_editor::Status) -> text_editor::Style {
    let c = Colors::from_theme(t);
    text_editor::Style {
        background: Background::Color(c.surface),
        border: Border::default(),
        placeholder: c.faint,
        value: c.text,
        selection: c.raised,
    }
}
pub fn picker(t: &Theme, s: pick_list::Status) -> pick_list::Style {
    let c = Colors::from_theme(t);
    pick_list::Style {
        text_color: c.muted,
        placeholder_color: c.faint,
        handle_color: c.faint,
        background: if matches!(s, pick_list::Status::Hovered) {
            c.raised
        } else {
            Color::TRANSPARENT
        }
        .into(),
        border: Border {
            radius: 6.into(),
            ..Default::default()
        },
    }
}
pub fn composer(c: Colors) -> container::Style {
    let mut s = panel(c, 15.);
    s.shadow = Shadow {
        color: Color {
            a: 0.14,
            ..Color::BLACK
        },
        offset: Vector::new(0., 6.),
        blur_radius: 24.,
    };
    s
}
