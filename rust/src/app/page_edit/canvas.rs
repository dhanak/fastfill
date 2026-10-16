//! Original-image corner handles; mouse/touch input stays local to this view.

use super::{Event, Message};
use crate::page_canvas::page_viewport;
use fastfill_rust::annotations::Pointer;
use fastfill_rust::geometry::{PageView, Size, fit_contain};
use fastfill_rust::project::{CropQuad, NormalizedPoint};
use iced::widget::{canvas, image::Handle};
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse, touch};

#[derive(Clone, Copy)]
pub(super) struct CropCanvas<'a> {
    pub(super) image: Option<&'a Handle>,
    pub(super) source_size: Size,
    pub(super) crop: CropQuad,
    pub(super) epoch: u64,
    pub(super) background: bool,
}

#[derive(Default)]
pub(super) struct CropState {
    drag: Option<(Pointer, usize, NormalizedPoint)>,
    epoch: u64,
}

#[derive(Clone, Copy)]
enum TouchPhase {
    Press,
    Move,
    Release,
    Lost,
}

fn touch_sample(event: &touch::Event) -> (Pointer, Point, TouchPhase) {
    let (id, position, phase) = match *event {
        touch::Event::FingerPressed { id, position } => {
            (id, position, TouchPhase::Press)
        }
        touch::Event::FingerMoved { id, position } => {
            (id, position, TouchPhase::Move)
        }
        touch::Event::FingerLifted { id, position } => {
            (id, position, TouchPhase::Release)
        }
        touch::Event::FingerLost { id, position } => {
            (id, position, TouchPhase::Lost)
        }
    };
    (Pointer::Touch(id.0), position, phase)
}

fn corners(crop: CropQuad) -> [NormalizedPoint; 4] {
    [
        crop.top_left,
        crop.top_right,
        crop.bottom_right,
        crop.bottom_left,
    ]
}

impl CropCanvas<'_> {
    fn rect(&self, bounds: Rectangle) -> Option<fastfill_rust::geometry::Rect> {
        fit_contain(self.source_size, page_viewport(bounds.size()))
            .map(PageView::page_rect)
    }

    fn normalized(
        &self,
        bounds: Rectangle,
        point: Point,
    ) -> Option<NormalizedPoint> {
        let rect = self.rect(bounds)?;
        Some(NormalizedPoint {
            x: ((point.x - rect.x) / rect.width).clamp(0.0, 1.0),
            y: ((point.y - rect.y) / rect.height).clamp(0.0, 1.0),
        })
    }

    fn hit(
        &self,
        bounds: Rectangle,
        point: Point,
        radius: f32,
    ) -> Option<usize> {
        let rect = self.rect(bounds)?;
        corners(self.crop)
            .iter()
            .enumerate()
            .filter_map(|(index, corner)| {
                let distance = (point.x - rect.x - corner.x * rect.width)
                    .hypot(point.y - rect.y - corner.y * rect.height);
                (distance <= radius).then_some((index, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
}

impl canvas::Program<Message> for CropCanvas<'_> {
    type State = CropState;

    fn update(
        &self,
        state: &mut CropState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if self.background || self.image.is_none() {
            return None;
        }
        if state.epoch != self.epoch {
            *state = CropState {
                epoch: self.epoch,
                ..CropState::default()
            };
        }
        let local =
            |point: Point| Point::new(point.x - bounds.x, point.y - bounds.y);
        let (pointer, position, pressed, released, lost) = match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )) => (
                Pointer::Mouse,
                cursor.position().map(local),
                true,
                false,
                false,
            ),
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                (Pointer::Mouse, Some(local(*position)), false, false, false)
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )) => (
                Pointer::Mouse,
                cursor.position().map(local),
                false,
                true,
                false,
            ),
            canvas::Event::Touch(event) => {
                let (pointer, position, phase) = touch_sample(event);
                (
                    pointer,
                    Some(local(position)),
                    matches!(phase, TouchPhase::Press),
                    matches!(phase, TouchPhase::Release | TouchPhase::Lost),
                    matches!(phase, TouchPhase::Lost),
                )
            }
            _ => return None,
        };
        if pressed && state.drag.is_none() {
            let position = position?;
            if !Rectangle::new(Point::ORIGIN, bounds.size()).contains(position)
            {
                return None;
            }
            let radius = if matches!(pointer, Pointer::Touch(_)) {
                28.0
            } else {
                12.0
            };
            let index = self.hit(bounds, position, radius)?;
            state.drag = Some((pointer, index, corners(self.crop)[index]));
            return Some(canvas::Action::capture());
        }
        let (owner, index, original) = state.drag?;
        if owner != pointer {
            return None;
        }
        if released {
            state.drag = None;
        }
        let point = if lost {
            original
        } else {
            self.normalized(bounds, position?)?
        };
        Some(
            canvas::Action::publish(Message::PageEdit(Event::Corner(
                index, point,
            )))
            .and_capture(),
        )
    }

    fn draw(
        &self,
        _state: &CropState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if self.background {
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                Color::from_rgb8(45, 48, 52),
            );
            if let Some(image) = self.image
                && let Some(rect) = self.rect(bounds)
            {
                frame.draw_image(
                    Rectangle::new(
                        Point::new(rect.x, rect.y),
                        iced::Size::new(rect.width, rect.height),
                    ),
                    image,
                );
            }
        } else if self.image.is_some()
            && let Some(rect) = self.rect(bounds)
        {
            let points = corners(self.crop).map(|p| {
                Point::new(
                    rect.x + p.x * rect.width,
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
            frame.fill(&outline, Color::from_rgba8(0, 170, 210, 0.18));
            frame.stroke(
                &outline,
                canvas::Stroke::default()
                    .with_width(2.0)
                    .with_color(Color::from_rgb8(0, 175, 230)),
            );
            for point in points {
                let knob = canvas::Path::circle(point, 8.0);
                frame.fill(&knob, Color::from_rgb8(0, 120, 210));
                frame.stroke(
                    &knob,
                    canvas::Stroke::default()
                        .with_width(2.0)
                        .with_color(Color::WHITE),
                );
            }
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &CropState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.background {
            return mouse::Interaction::default();
        }
        if state.drag.is_some() {
            return mouse::Interaction::Grabbing;
        }
        if cursor
            .position_in(bounds)
            .is_some_and(|point| self.hit(bounds, point, 12.0).is_some())
        {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}
