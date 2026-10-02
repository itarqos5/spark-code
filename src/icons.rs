use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse,
    widget::canvas::{self, Canvas, Frame, Geometry, LineCap, LineJoin, Path, Stroke},
};
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum Kind {
    Lock,
    Bolt,
    Plus,
    Search,
    Folder,
    Chat,
    Settings,
    Sun,
    Moon,
    Chevron,
    ArrowUp,
    Panel,
    Close,
    Minimize,
    Maximize,
    Code,
    Bug,
    Refresh,
    Copy,
    Check,
    Back,
    Terminal,
}
pub fn icon<'a, Msg: 'a>(kind: Kind, size: f32, color: Color) -> Element<'a, Msg> {
    Canvas::new(Symbol { kind, color })
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}
struct Symbol {
    kind: Kind,
    color: Color,
}
impl<Message> canvas::Program<Message> for Symbol {
    type State = ();
    fn draw(
        &self,
        _: &(),
        r: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(r, bounds.size());
        f.scale(bounds.width / 24.0);
        let stroke = Stroke::default()
            .with_color(self.color)
            .with_width(1.65)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        let path = |points: &[(f32, f32)]| {
            Path::new(|p| {
                if let Some(v) = points.first() {
                    p.move_to(Point::new(v.0, v.1));
                }
                for v in &points[1..] {
                    p.line_to(Point::new(v.0, v.1));
                }
            })
        };
        let mut line = |ps: &[(f32, f32)]| f.stroke(&path(ps), stroke);
        match self.kind {
            Kind::Lock => {
                line(&[(5., 10.), (19., 10.), (19., 21.), (5., 21.), (5., 10.)]);
                f.stroke(
                    &Path::new(|p| {
                        p.move_to(Point::new(8., 10.));
                        p.line_to(Point::new(8., 6.));
                        p.bezier_curve_to(
                            Point::new(8., 1.),
                            Point::new(16., 1.),
                            Point::new(16., 6.),
                        );
                        p.line_to(Point::new(16., 10.));
                    }),
                    stroke,
                );
                f.fill(&Path::circle(Point::new(12., 15.), 1.2), self.color);
            }
            Kind::Bolt => {
                let p = Path::new(|p| {
                    p.move_to(Point::new(13.8, 2.8));
                    for (x, y) in [
                        (5.7, 13.3),
                        (11.1, 13.3),
                        (9.7, 21.4),
                        (18.8, 10.2),
                        (13.5, 10.2),
                        (15., 2.8),
                    ] {
                        p.line_to(Point::new(x, y));
                    }
                    p.close();
                });
                f.fill(&p, self.color);
            }
            Kind::Plus => {
                line(&[(12., 5.), (12., 19.)]);
                line(&[(5., 12.), (19., 12.)]);
            }
            Kind::Search => {
                line(&[(16., 16.), (21., 21.)]);
                f.stroke(&Path::circle(Point::new(10.5, 10.5), 6.5), stroke);
            }
            Kind::Folder => line(&[
                (3., 7.),
                (9., 7.),
                (11., 9.),
                (21., 9.),
                (21., 19.),
                (3., 19.),
                (3., 7.),
                (3., 5.),
                (9., 5.),
                (11., 7.),
                (20., 7.),
            ]),
            Kind::Chat => line(&[
                (4., 4.),
                (20., 4.),
                (20., 17.),
                (10., 17.),
                (4., 21.),
                (4., 4.),
            ]),
            Kind::Settings => {
                f.stroke(&Path::circle(Point::new(12., 12.), 3.3), stroke);
                f.stroke(&Path::circle(Point::new(12., 12.), 7.5), stroke);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.;
                    f.stroke(
                        &path(&[
                            (12. + a.cos() * 8., 12. + a.sin() * 8.),
                            (12. + a.cos() * 10., 12. + a.sin() * 10.),
                        ]),
                        stroke,
                    );
                }
            }
            Kind::Sun => {
                f.stroke(&Path::circle(Point::new(12., 12.), 4.), stroke);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.;
                    f.stroke(
                        &path(&[
                            (12. + a.cos() * 7., 12. + a.sin() * 7.),
                            (12. + a.cos() * 9.5, 12. + a.sin() * 9.5),
                        ]),
                        stroke,
                    );
                }
            }
            Kind::Moon => {
                f.stroke(
                    &Path::new(|p| {
                        p.move_to(Point::new(17.8, 18.8));
                        p.bezier_curve_to(
                            Point::new(3., 23.),
                            Point::new(0., 6.),
                            Point::new(11., 3.),
                        );
                        p.bezier_curve_to(
                            Point::new(6., 13.),
                            Point::new(16., 16.),
                            Point::new(20., 12.),
                        );
                        p.bezier_curve_to(
                            Point::new(20., 15.),
                            Point::new(19., 17.),
                            Point::new(17.8, 18.8),
                        );
                    }),
                    stroke,
                );
            }
            Kind::Chevron => line(&[(7., 10.), (12., 15.), (17., 10.)]),
            Kind::ArrowUp => {
                line(&[(12., 19.), (12., 5.)]);
                line(&[(6., 11.), (12., 5.), (18., 11.)]);
            }
            Kind::Panel => {
                line(&[(3., 4.), (21., 4.), (21., 20.), (3., 20.), (3., 4.)]);
                line(&[(9., 4.), (9., 20.)]);
            }
            Kind::Close => {
                line(&[(7., 7.), (17., 17.)]);
                line(&[(17., 7.), (7., 17.)]);
            }
            Kind::Minimize => line(&[(6., 12.), (18., 12.)]),
            Kind::Maximize => line(&[(6., 6.), (18., 6.), (18., 18.), (6., 18.), (6., 6.)]),
            Kind::Code => {
                line(&[(8., 7.), (3., 12.), (8., 17.)]);
                line(&[(16., 7.), (21., 12.), (16., 17.)]);
                line(&[(14., 4.), (10., 20.)]);
            }
            Kind::Bug => {
                line(&[
                    (8., 8.),
                    (8., 17.),
                    (12., 20.),
                    (16., 17.),
                    (16., 8.),
                    (8., 8.),
                ]);
                line(&[(8., 8.), (10., 4.), (14., 4.), (16., 8.)]);
                line(&[(3., 10.), (8., 12.), (16., 12.), (21., 10.)]);
                line(&[(3., 18.), (8., 16.), (16., 16.), (21., 18.)]);
            }
            Kind::Refresh => {
                line(&[(18., 3.), (18., 8.), (23., 8.)]);
                f.stroke(
                    &Path::new(|p| {
                        p.move_to(Point::new(18., 7.));
                        p.bezier_curve_to(
                            Point::new(6., -1.),
                            Point::new(-1., 12.),
                            Point::new(7., 19.),
                        );
                        p.bezier_curve_to(
                            Point::new(13., 24.),
                            Point::new(22., 19.),
                            Point::new(21., 13.),
                        );
                    }),
                    stroke,
                );
            }
            Kind::Copy => {
                line(&[(8., 8.), (20., 8.), (20., 21.), (8., 21.), (8., 8.)]);
                line(&[(15., 8.), (15., 3.), (3., 3.), (3., 16.), (8., 16.)]);
            }
            Kind::Check => line(&[(5., 12.), (10., 17.), (20., 6.)]),
            Kind::Back => {
                line(&[(19., 12.), (5., 12.)]);
                line(&[(10., 6.), (4., 12.), (10., 18.)]);
            }
            Kind::Terminal => {
                line(&[(4., 6.), (10., 12.), (4., 18.)]);
                line(&[(12., 18.), (20., 18.)]);
            }
        }
        vec![f.into_geometry()]
    }
}
