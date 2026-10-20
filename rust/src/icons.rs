//! Small monochrome toolbar paths. No platform icon fonts or bitmap assets.

use crate::appearance;
use fastfill_rust::annotations::Tool;
use iced::widget::{button, canvas};
use iced::{Color, Element, Point, Radians, Rectangle, Renderer, Size, Theme};
use iced::{Vector, mouse};

const VIEWBOX: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Glyph {
    Tool(Tool),
    Fill,
    SmartSnap,
}

pub(crate) fn view<Message: 'static>(
    glyph: Glyph,
    selected: bool,
    enabled: bool,
) -> Element<'static, Message> {
    canvas(Icon {
        glyph,
        selected,
        enabled,
    })
    .width(appearance::TOOL_ICON_SIZE)
    .height(appearance::TOOL_ICON_SIZE)
    .into()
}

struct Icon {
    glyph: Glyph,
    selected: bool,
    enabled: bool,
}

impl<Message> canvas::Program<Message> for Icon {
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
        let color =
            appearance::tool_button(theme, status, self.selected).text_color;
        frame.scale(bounds.width.min(bounds.height) / VIEWBOX);
        let stroke = canvas::Stroke::default()
            .with_color(color)
            .with_width(2.0)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round);
        match self.glyph {
            Glyph::Tool(tool) => draw_tool(&mut frame, tool, stroke, color),
            Glyph::Fill => draw_bucket(&mut frame, stroke, color),
            Glyph::SmartSnap => draw_magnet(&mut frame, stroke),
        }
        vec![frame.into_geometry()]
    }
}

fn path(points: &[[f32; 2]], closed: bool) -> canvas::Path {
    canvas::Path::new(|builder| {
        builder.move_to(Point::from(points[0]));
        for point in &points[1..] {
            builder.line_to(Point::from(*point));
        }
        if closed {
            builder.close();
        }
    })
}

fn draw_tool(
    frame: &mut canvas::Frame,
    tool: Tool,
    stroke: canvas::Stroke<'_>,
    color: Color,
) {
    match tool {
        Tool::Navigate => frame.stroke(&hand(), stroke),
        Tool::Select => frame.fill(
            &path(
                &[
                    [5.0, 3.0],
                    [6.0, 20.0],
                    [10.0, 15.0],
                    [13.0, 21.0],
                    [16.0, 19.5],
                    [13.0, 13.5],
                    [20.0, 13.0],
                ],
                true,
            ),
            color,
        ),
        Tool::Rectangle => frame.stroke(
            &canvas::Path::rectangle(
                Point::new(4.0, 5.0),
                Size::new(16.0, 14.0),
            ),
            stroke,
        ),
        Tool::Oval => frame.stroke(
            &canvas::Path::new(|builder| {
                builder.ellipse(canvas::path::arc::Elliptical {
                    center: Point::new(12.0, 12.0),
                    radii: Vector::new(8.0, 7.0),
                    rotation: Radians(0.0),
                    start_angle: Radians(0.0),
                    end_angle: Radians(std::f32::consts::TAU),
                });
                builder.close();
            }),
            stroke,
        ),
        Tool::Line | Tool::Arrow => {
            frame.stroke(&path(&[[4.0, 20.0], [20.0, 4.0]], false), stroke);
            if tool == Tool::Arrow {
                frame.stroke(
                    &path(&[[10.0, 4.0], [20.0, 4.0], [20.0, 14.0]], false),
                    stroke,
                );
            }
        }
        Tool::Checkmark => frame.stroke(
            &path(&[[4.0, 12.0], [10.0, 18.0], [20.0, 5.0]], false),
            stroke,
        ),
        Tool::Cross => {
            frame.stroke(&path(&[[5.0, 5.0], [19.0, 19.0]], false), stroke);
            frame.stroke(&path(&[[19.0, 5.0], [5.0, 19.0]], false), stroke);
        }
        Tool::Text => frame.fill_text(canvas::Text {
            content: "Aa".into(),
            position: Point::new(12.0, 12.0),
            color,
            size: 20.0.into(),
            line_height: iced::widget::text::LineHeight::Relative(1.0),
            font: iced::Font::with_name("Fira Sans"),
            align_x: iced::widget::text::Alignment::Center,
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        }),
        Tool::Pen | Tool::Highlighter => {
            draw_writing_tool(frame, tool, stroke, color);
        }
    }
}

fn draw_writing_tool(
    frame: &mut canvas::Frame,
    tool: Tool,
    stroke: canvas::Stroke<'_>,
    color: Color,
) {
    match tool {
        Tool::Pen => {
            frame.stroke(
                &path(
                    &[
                        [4.0, 20.0],
                        [7.0, 11.0],
                        [15.0, 3.0],
                        [21.0, 9.0],
                        [13.0, 17.0],
                    ],
                    true,
                ),
                stroke,
            );
            frame.stroke(&path(&[[7.0, 11.0], [13.0, 17.0]], false), stroke);
            frame.stroke(&path(&[[4.0, 20.0], [10.0, 14.0]], false), stroke);
        }
        Tool::Highlighter => {
            frame.stroke(
                &path(
                    &[
                        [4.0, 15.0],
                        [14.0, 3.0],
                        [21.0, 10.0],
                        [11.0, 19.0],
                        [7.0, 19.0],
                        [7.0, 16.0],
                    ],
                    true,
                ),
                stroke,
            );
            frame.stroke(&path(&[[11.0, 6.5], [18.0, 13.0]], false), stroke);
            frame.fill(
                &canvas::Path::rectangle(
                    Point::new(3.0, 21.0),
                    Size::new(18.0, 2.0),
                ),
                color,
            );
        }
        _ => unreachable!("writing tool icons only"),
    }
}

fn hand() -> canvas::Path {
    canvas::Path::new(|b| {
        b.move_to(Point::new(8.0, 12.0));
        b.line_to(Point::new(5.0, 10.0));
        b.bezier_curve_to(
            Point::new(2.0, 8.0),
            Point::new(2.0, 11.0),
            Point::new(4.0, 14.0),
        );
        b.line_to(Point::new(7.0, 19.0));
        b.bezier_curve_to(
            Point::new(10.0, 23.0),
            Point::new(19.0, 22.0),
            Point::new(20.0, 16.0),
        );
        b.line_to(Point::new(20.0, 8.0));
        b.bezier_curve_to(
            Point::new(20.0, 5.0),
            Point::new(17.0, 5.0),
            Point::new(17.0, 8.0),
        );
        b.line_to(Point::new(17.0, 11.0));
        b.line_to(Point::new(17.0, 6.0));
        b.bezier_curve_to(
            Point::new(17.0, 3.0),
            Point::new(14.0, 3.0),
            Point::new(14.0, 6.0),
        );
        b.line_to(Point::new(14.0, 10.0));
        b.line_to(Point::new(14.0, 5.0));
        b.bezier_curve_to(
            Point::new(14.0, 2.0),
            Point::new(11.0, 2.0),
            Point::new(11.0, 5.0),
        );
        b.line_to(Point::new(11.0, 10.0));
        b.line_to(Point::new(11.0, 6.0));
        b.bezier_curve_to(
            Point::new(11.0, 3.0),
            Point::new(8.0, 3.0),
            Point::new(8.0, 6.0),
        );
        b.close();
    })
}

fn draw_bucket(
    frame: &mut canvas::Frame,
    stroke: canvas::Stroke<'_>,
    color: Color,
) {
    frame.stroke(
        &path(
            &[[3.0, 11.0], [11.0, 3.0], [19.0, 11.0], [11.0, 19.0]],
            true,
        ),
        stroke,
    );
    frame.stroke(&path(&[[3.0, 11.0], [19.0, 11.0]], false), stroke);
    frame.stroke(&path(&[[4.0, 3.0], [9.0, 8.0]], false), stroke);
    frame.fill(
        &canvas::Path::new(|b| {
            b.move_to(Point::new(20.0, 14.0));
            b.bezier_curve_to(
                Point::new(14.0, 22.0),
                Point::new(26.0, 22.0),
                Point::new(20.0, 14.0),
            );
            b.close();
        }),
        color,
    );
}

fn draw_magnet(frame: &mut canvas::Frame, stroke: canvas::Stroke<'_>) {
    let outline = canvas::Path::new(|b| {
        b.move_to(Point::new(4.0, 4.0));
        for point in [[9.0, 4.0], [9.0, 13.0]] {
            b.line_to(Point::from(point));
        }
        b.bezier_curve_to(
            Point::new(9.0, 18.0),
            Point::new(15.0, 18.0),
            Point::new(15.0, 13.0),
        );
        for point in [[15.0, 4.0], [20.0, 4.0], [20.0, 13.0]] {
            b.line_to(Point::from(point));
        }
        b.bezier_curve_to(
            Point::new(20.0, 22.5),
            Point::new(4.0, 22.5),
            Point::new(4.0, 13.0),
        );
        b.close();
    });
    frame.stroke(&outline, stroke);
    for points in [[[4.0, 8.0], [9.0, 8.0]], [[15.0, 8.0], [20.0, 8.0]]] {
        frame.stroke(&path(&points, false), stroke);
    }
}
