//! Camera glyphs share the toolbar's 24-pixel monochrome path convention.

use crate::appearance;
use iced::widget::{button, canvas};
use iced::{Color, Element, Point, Radians, Rectangle, Renderer, Theme, mouse};

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Cancel,
    Rotate,
    Mirror,
    Capture,
    Pause,
    Play,
    Retake,
    Settings,
    Switch,
    UsePhoto,
}

pub(super) fn view<Message: 'static>(
    glyph: Icon,
    selected: bool,
    enabled: bool,
) -> Element<'static, Message> {
    canvas(Drawing {
        glyph,
        selected,
        enabled,
    })
    .width(24)
    .height(24)
    .into()
}

struct Drawing {
    glyph: Icon,
    selected: bool,
    enabled: bool,
}

fn path(points: &[[f32; 2]], closed: bool) -> canvas::Path {
    canvas::Path::new(|b| {
        b.move_to(Point::from(points[0]));
        for point in &points[1..] {
            b.line_to(Point::from(*point));
        }
        if closed {
            b.close();
        }
    })
}

impl<Message> canvas::Program<Message> for Drawing {
    type State = ();

    fn draw(
        &self,
        (): &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let status = if self.enabled {
            button::Status::Active
        } else {
            button::Status::Disabled
        };
        let color = if self.selected {
            appearance::primary_action(theme, status)
        } else {
            appearance::secondary_action(theme, status)
        }
        .text_color;
        frame.scale(bounds.width.min(bounds.height) / 24.0);
        draw(&mut frame, self.glyph, color);
        vec![frame.into_geometry()]
    }
}

fn draw(frame: &mut canvas::Frame, glyph: Icon, color: Color) {
    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(1.8)
        .with_line_cap(canvas::LineCap::Round)
        .with_line_join(canvas::LineJoin::Round);
    match glyph {
        Icon::Cancel => {
            frame.stroke(&path(&[[6.0, 6.0], [18.0, 18.0]], false), stroke);
            frame.stroke(&path(&[[18.0, 6.0], [6.0, 18.0]], false), stroke);
        }
        Icon::UsePhoto => frame.stroke(
            &path(&[[4.0, 12.0], [10.0, 18.0], [20.0, 5.0]], false),
            stroke,
        ),
        Icon::Mirror => {
            frame.fill(
                &path(&[[9.0, 2.0], [9.0, 22.0], [1.0, 22.0]], true),
                color,
            );
            frame.stroke(&path(&[[12.0, 2.0], [12.0, 22.0]], false), stroke);
            frame.stroke(
                &path(&[[15.0, 2.0], [15.0, 22.0], [23.0, 22.0]], true),
                canvas::Stroke {
                    line_dash: canvas::LineDash {
                        segments: &[2.0, 2.0],
                        offset: 0,
                    },
                    line_cap: canvas::LineCap::Butt,
                    ..stroke
                },
            );
        }
        Icon::Capture | Icon::Switch => {
            camera(frame, stroke, matches!(glyph, Icon::Switch));
        }
        Icon::Pause => {
            for x in [7.0, 15.0] {
                frame.fill(
                    &canvas::Path::rectangle(
                        Point::new(x, 5.0),
                        iced::Size::new(3.0, 14.0),
                    ),
                    color,
                );
            }
        }
        Icon::Play => frame
            .fill(&path(&[[7.0, 4.0], [20.0, 12.0], [7.0, 20.0]], true), color),
        Icon::Rotate => {
            frame.stroke(
                &canvas::Path::new(|b| {
                    b.arc(canvas::path::Arc {
                        center: Point::new(12.0, 12.0),
                        radius: 8.0,
                        start_angle: Radians(0.0),
                        end_angle: Radians(std::f32::consts::PI * 1.5),
                    });
                }),
                stroke,
            );
            frame.stroke(
                &path(&[[8.0, 2.0], [12.0, 4.0], [10.0, 8.0]], false),
                stroke,
            );
        }
        Icon::Retake => {
            frame.stroke(
                &path(&[[9.0, 4.0], [3.0, 10.0], [9.0, 16.0]], false),
                stroke,
            );
            frame.stroke(
                &canvas::Path::new(|b| {
                    b.move_to(Point::new(3.0, 10.0));
                    b.line_to(Point::new(14.0, 10.0));
                    b.bezier_curve_to(
                        Point::new(18.0, 10.0),
                        Point::new(21.0, 13.0),
                        Point::new(21.0, 17.0),
                    );
                    b.line_to(Point::new(21.0, 20.0));
                }),
                stroke,
            );
        }
        Icon::Settings => gear(frame, stroke),
    }
}

fn gear(frame: &mut canvas::Frame, stroke: canvas::Stroke<'_>) {
    use std::f32::consts::FRAC_PI_4;

    let half_tooth = FRAC_PI_4 / 4.0;
    let outline = canvas::Path::new(|b| {
        for tooth in 0_u8..8 {
            for (i, (offset, radius)) in [
                (-half_tooth, 7.5),
                (-half_tooth, 10.0),
                (half_tooth, 10.0),
                (half_tooth, 7.5),
            ]
            .into_iter()
            .enumerate()
            {
                let angle = f32::from(tooth) * FRAC_PI_4 + offset;
                let point = Point::new(
                    12.0 + angle.cos() * radius,
                    12.0 + angle.sin() * radius,
                );
                if tooth == 0 && i == 0 {
                    b.move_to(point);
                } else {
                    b.line_to(point);
                }
            }
        }
        b.close();
    });
    frame.stroke(&outline, stroke);
    frame.stroke(&canvas::Path::circle(Point::new(12.0, 12.0), 3.0), stroke);
}

fn camera(frame: &mut canvas::Frame, stroke: canvas::Stroke<'_>, switch: bool) {
    frame.stroke(
        &path(
            &[
                [3.0, 7.0],
                [7.0, 7.0],
                [9.0, 4.0],
                [15.0, 4.0],
                [17.0, 7.0],
                [21.0, 7.0],
                [21.0, 20.0],
                [3.0, 20.0],
            ],
            true,
        ),
        stroke,
    );
    if switch {
        frame.stroke(
            &path(&[[6.0, 12.0], [17.0, 12.0], [15.0, 10.0]], false),
            stroke,
        );
        frame.stroke(
            &path(&[[18.0, 16.0], [7.0, 16.0], [9.0, 18.0]], false),
            stroke,
        );
    } else {
        frame
            .stroke(&canvas::Path::circle(Point::new(12.0, 13.0), 4.0), stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Message, tests::render_element};

    #[test]
    fn mirror_triangles_are_larger_with_their_tall_edges_facing_inward() {
        let pixels = render_element(
            view::<Message>(Icon::Mirror, false, true),
            iced::Size::new(24, 24),
            1,
            1.0,
        );
        let ink = |x: usize, y: usize| pixels[(y * 24 + x) * 4] < 100;
        assert!(ink(8, 6), "solid triangle's tall edge must face inward");
        assert!(ink(8, 21), "triangles must use the full glyph height");
        assert!(!ink(3, 6), "solid triangle must point outward");
        assert!(!ink(17, 18), "right triangle must be hollow");
        assert!((4..20).all(|y| ink(12, y)), "mirror axis disappeared");
    }

    #[test]
    fn retake_and_rotate_are_visually_distinct() {
        let render = |glyph| {
            render_element(
                view::<Message>(glyph, false, true),
                iced::Size::new(24, 24),
                1,
                1.0,
            )
        };
        assert!(
            render(Icon::Retake) != render(Icon::Rotate),
            "Retake must not look like Rotate"
        );
    }

    #[test]
    fn settings_has_broad_gear_teeth_and_a_clear_center_hole() {
        let pixels = render_element(
            view::<Message>(Icon::Settings, false, true),
            iced::Size::new(24, 24),
            1,
            1.0,
        );
        let ink = |x: usize, y: usize| pixels[(y * 24 + x) * 4] < 100;
        assert!(ink(13, 2), "top tooth must be wider than a radial spoke");
        assert!(!ink(12, 12), "gear center must stay hollow");
    }
}
