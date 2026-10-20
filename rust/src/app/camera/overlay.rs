//! Overlay coordinates share the image widget's contained, letterboxed fit.

use super::{Frame, Message};
use crate::appearance::accent;
use fastfill_rust::geometry::{Rect, Size, fit_contain};
use fastfill_rust::project::CropQuad;
use iced::mouse;
use iced::widget::{Space, canvas, image};
use iced::{Color, Element, Fill, Point, Radians, Rectangle, Renderer, Theme};

#[allow(clippy::cast_precision_loss)]
pub(super) fn preview(
    frame: &Frame,
    crop: Option<CropQuad>,
    mirrored: bool,
) -> Element<'_, Message> {
    let mut layers = iced::widget::stack![
        Space::new().width(Fill).height(Fill),
        image(frame.allocation.handle().clone())
            .width(Fill)
            .height(Fill)
            .content_fit(iced::ContentFit::Contain),
    ];
    if let Some(crop) = crop {
        layers = layers.push(
            canvas(Outline {
                source: Size::new(
                    frame.pixels.width() as f32,
                    frame.pixels.height() as f32,
                ),
                crop,
                mirrored,
            })
            .width(Fill)
            .height(Fill),
        );
    }
    layers.width(Fill).height(Fill).clip(true).into()
}

struct Outline {
    source: Size,
    crop: CropQuad,
    mirrored: bool,
}

impl canvas::Program<Message> for Outline {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if let Some(view) = fit_contain(
            self.source,
            Rect::new(0.0, 0.0, bounds.width, bounds.height),
        ) {
            let rect = view.page_rect();
            let points = self.crop.points().map(|p| {
                Point::new(
                    rect.x
                        + if self.mirrored { 1.0 - p.x } else { p.x }
                            * rect.width,
                    rect.y + p.y * rect.height,
                )
            });
            let outline = canvas::Path::new(|builder| {
                builder.move_to(points[0]);
                for point in &points[1..] {
                    builder.line_to(*point);
                }
                builder.close();
            });
            let color = Color::from_rgb8(0, 170, 105);
            frame.fill(&outline, Color { a: 0.18, ..color });
            frame.stroke(
                &outline,
                canvas::Stroke::default().with_color(color).with_width(2.0),
            );
            for point in points {
                let knob = canvas::Path::circle(point, 5.0);
                frame.fill(&knob, color);
                frame.stroke(
                    &knob,
                    canvas::Stroke::default()
                        .with_color(Color::WHITE)
                        .with_width(2.0),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

pub(super) fn countdown(progress: f64) -> Element<'static, Message> {
    canvas(Ring(progress)).width(40).height(40).into()
}

struct Ring(f64);

impl canvas::Program<Message> for Ring {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = bounds.width.min(bounds.height) / 2.0 - 3.0;
        let circle = canvas::Path::circle(center, radius);
        frame.stroke(
            &circle,
            canvas::Stroke::default()
                .with_width(2.0)
                .with_color(Color::from_rgb8(180, 185, 190)),
        );
        if self.0 > 0.0 {
            use std::f32::consts::{FRAC_PI_2, TAU};
            #[allow(clippy::cast_possible_truncation)]
            let progress = self.0.clamp(0.0, 1.0) as f32;
            let path = if progress >= 1.0 {
                circle
            } else {
                canvas::Path::new(|builder| {
                    builder.arc(canvas::path::Arc {
                        center,
                        radius,
                        start_angle: Radians(-FRAC_PI_2),
                        end_angle: Radians(-FRAC_PI_2 + progress * TAU),
                    });
                })
            };
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(2.0)
                    .with_color(accent())
                    .with_line_cap(canvas::LineCap::Round),
            );
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests;
