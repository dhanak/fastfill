//! Original-image corner handles; mouse/touch input stays local to this view.

use super::viewer::{self, Mode, ViewState};
use super::{Event, Message};
use fastfill_rust::annotations::Pointer;
use fastfill_rust::form_snap::CornerFeatures;
use fastfill_rust::geometry::{Point as GeoPoint, Rect, Size};
use fastfill_rust::interaction::{TouchEvent, TouchNavigation};
use fastfill_rust::project::{CropQuad, NormalizedPoint};
use iced::widget::{canvas, image::Handle};
use iced::{
    Color, Point, Rectangle, Renderer, Theme, Vector, keyboard, mouse, touch,
};

#[derive(Clone, Copy)]
pub(super) struct CropCanvas<'a> {
    pub(super) image: Option<&'a Handle>,
    pub(super) source_size: Size,
    pub(super) crop: CropQuad,
    pub(super) corners: Option<&'a CornerFeatures>,
    pub(super) snap: bool,
    pub(super) epoch: u64,
    pub(super) background: bool,
    pub(super) mode: Mode,
    pub(super) view: ViewState,
    pub(super) rotation: i32,
}

#[derive(Default)]
pub(super) struct CropState {
    drag: Option<Drag>,
    control: Option<bool>,
    epoch: u64,
    pan: Option<(mouse::Button, Point)>,
    touch: TouchNavigation,
}

#[derive(Clone, Copy)]
struct Drag {
    pointer: Pointer,
    index: usize,
    original: NormalizedPoint,
    raw: NormalizedPoint,
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
    fn display_size(&self) -> Size {
        if self.mode == Mode::Frame && self.rotation.rem_euclid(2) == 1 {
            Size::new(self.source_size.height, self.source_size.width)
        } else {
            self.source_size
        }
    }

    fn display_point(&self, point: NormalizedPoint) -> NormalizedPoint {
        if self.mode == Mode::Preview {
            return point;
        }
        match self.rotation.rem_euclid(4) {
            1 => NormalizedPoint {
                x: 1.0 - point.y,
                y: point.x,
            },
            2 => NormalizedPoint {
                x: 1.0 - point.x,
                y: 1.0 - point.y,
            },
            3 => NormalizedPoint {
                x: point.y,
                y: 1.0 - point.x,
            },
            _ => point,
        }
    }

    fn source_point(&self, point: NormalizedPoint) -> NormalizedPoint {
        if self.mode == Mode::Preview {
            return point;
        }
        match self.rotation.rem_euclid(4) {
            1 => NormalizedPoint {
                x: point.y,
                y: 1.0 - point.x,
            },
            2 => NormalizedPoint {
                x: 1.0 - point.x,
                y: 1.0 - point.y,
            },
            3 => NormalizedPoint {
                x: 1.0 - point.y,
                y: point.x,
            },
            _ => point,
        }
    }

    fn viewport(bounds: Rectangle) -> Rect {
        // Short windows need less image margin than the main page canvas.
        let margin = 36.0_f32.min(bounds.height * 0.1);
        Rect::new(
            margin,
            margin,
            (bounds.width - 2.0 * margin).max(0.0),
            (bounds.height - 2.0 * margin).max(0.0),
        )
    }

    fn navigate(
        &self,
        delta: fastfill_rust::interaction::GestureDelta,
        bounds: Rectangle,
    ) -> canvas::Action<Message> {
        canvas::Action::publish(Message::PageEdit(Event::View(
            viewer::Event::Navigate {
                mode: self.mode,
                delta,
                image: self.display_size(),
                viewport: Self::viewport(bounds),
            },
        )))
        .and_capture()
    }

    fn snapped(
        &self,
        index: usize,
        raw: NormalizedPoint,
        bounds: Rectangle,
        enabled: bool,
    ) -> NormalizedPoint {
        if !enabled {
            return raw;
        }
        let (Some(features), Some(rect)) = (self.corners, self.rect(bounds))
        else {
            return raw;
        };
        let radius =
            (52.0 / rect.width.min(rect.height).max(1.0)).clamp(0.015, 0.09);
        let Some(target) = features.find_corner(raw, radius) else {
            return raw;
        };
        let mut points = corners(self.crop);
        points[index] = target;
        let crop = CropQuad {
            top_left: points[0],
            top_right: points[1],
            bottom_right: points[2],
            bottom_left: points[3],
        };
        // A nearby hint must not turn a valid manual drag into a crossed crop.
        if crop.is_valid() { target } else { raw }
    }

    fn rect(&self, bounds: Rectangle) -> Option<fastfill_rust::geometry::Rect> {
        self.view
            .navigation(self.display_size(), Self::viewport(bounds))
            .view(self.display_size(), Self::viewport(bounds))
            .map(fastfill_rust::geometry::PageView::page_rect)
    }

    fn normalized(
        &self,
        bounds: Rectangle,
        point: Point,
    ) -> Option<NormalizedPoint> {
        let rect = self.rect(bounds)?;
        Some(self.source_point(NormalizedPoint {
            x: ((point.x - rect.x) / rect.width).clamp(0.0, 1.0),
            y: ((point.y - rect.y) / rect.height).clamp(0.0, 1.0),
        }))
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
                let corner = self.display_point(*corner);
                let distance = (point.x - rect.x - corner.x * rect.width)
                    .hypot(point.y - rect.y - corner.y * rect.height);
                (distance <= radius).then_some((index, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
    fn touch_input(
        &self,
        state: &mut CropState,
        event: &canvas::Event,
        bounds: Rectangle,
    ) -> Option<canvas::Action<Message>> {
        let local =
            |point: Point| Point::new(point.x - bounds.x, point.y - bounds.y);
        if let canvas::Event::Touch(event) = event {
            let (pointer, position, phase) = touch_sample(event);
            let position = local(position);
            let id = match pointer {
                Pointer::Touch(id) => id,
                Pointer::Mouse => unreachable!(),
            };
            if let Some(drag) = state.drag {
                if drag.pointer != pointer {
                    return None;
                }
                if matches!(phase, TouchPhase::Release | TouchPhase::Lost) {
                    state.drag = None;
                }
                let point = if matches!(phase, TouchPhase::Lost) {
                    drag.original
                } else {
                    let raw = self.normalized(bounds, position)?;
                    if let Some(active) = &mut state.drag {
                        active.raw = raw;
                    }
                    self.snapped(
                        drag.index,
                        raw,
                        bounds,
                        state.control.unwrap_or(self.snap),
                    )
                };
                return Some(
                    canvas::Action::publish(Message::PageEdit(Event::Corner(
                        drag.index, point,
                    )))
                    .and_capture(),
                );
            }
            if matches!(phase, TouchPhase::Press)
                && !state.touch.is_active()
                && Rectangle::new(Point::ORIGIN, bounds.size())
                    .contains(position)
                && self.mode == Mode::Frame
                && let Some(index) = self.hit(bounds, position, 28.0)
            {
                let original = corners(self.crop)[index];
                state.drag = Some(Drag {
                    pointer,
                    index,
                    original,
                    raw: original,
                });
                return Some(canvas::Action::capture());
            }
            let point = GeoPoint::new(position.x, position.y);
            let touch = match phase {
                TouchPhase::Press => TouchEvent::Pressed(id, point),
                TouchPhase::Move => TouchEvent::Moved(id, point),
                TouchPhase::Release => TouchEvent::Released(id, point),
                TouchPhase::Lost => TouchEvent::Lost(id, point),
            };
            if matches!(phase, TouchPhase::Press)
                && !Rectangle::new(Point::ORIGIN, bounds.size())
                    .contains(position)
            {
                return None;
            }
            if !matches!(phase, TouchPhase::Press) && !state.touch.is_active() {
                return None;
            }
            return state.touch.handle(touch, true).map_or_else(
                || Some(canvas::Action::capture()),
                |delta| Some(self.navigate(delta, bounds)),
            );
        }
        None
    }

    fn mouse_input(
        &self,
        state: &mut CropState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let local =
            |point: Point| Point::new(point.x - bounds.x, point.y - bounds.y);
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(button))
                if matches!(
                    button,
                    mouse::Button::Left | mouse::Button::Right
                ) && state.pan.is_none()
                    && state.drag.is_none()
                    && !state.touch.is_active() =>
            {
                let position = cursor.position_in(bounds)?;
                if *button == mouse::Button::Left
                    && self.mode == Mode::Frame
                    && let Some(index) = self.hit(bounds, position, 12.0)
                {
                    let original = corners(self.crop)[index];
                    state.drag = Some(Drag {
                        pointer: Pointer::Mouse,
                        index,
                        original,
                        raw: original,
                    });
                } else {
                    state.pan = Some((*button, position));
                }
                Some(canvas::Action::capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let position = local(*position);
                if let Some(drag) = state.drag {
                    let raw = self.normalized(bounds, position)?;
                    if let Some(active) = &mut state.drag {
                        active.raw = raw;
                    }
                    let point = self.snapped(
                        drag.index,
                        raw,
                        bounds,
                        state.control.unwrap_or(self.snap),
                    );
                    return Some(
                        canvas::Action::publish(Message::PageEdit(
                            Event::Corner(drag.index, point),
                        ))
                        .and_capture(),
                    );
                }
                let (_, previous) = state.pan.as_mut()?;
                let delta = viewer::pan(
                    position.x - previous.x,
                    position.y - previous.y,
                );
                *previous = position;
                Some(self.navigate(delta, bounds))
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if *button == mouse::Button::Left
                    && let Some(drag) = state.drag.take()
                {
                    let point = cursor
                        .position()
                        .map(local)
                        .and_then(|p| self.normalized(bounds, p))
                        .map_or(drag.original, |raw| {
                            self.snapped(
                                drag.index,
                                raw,
                                bounds,
                                state.control.unwrap_or(self.snap),
                            )
                        });
                    return Some(
                        canvas::Action::publish(Message::PageEdit(
                            Event::Corner(drag.index, point),
                        ))
                        .and_capture(),
                    );
                }
                if state.pan.is_some_and(|(held, _)| held == *button) {
                    state.pan = None;
                    return Some(canvas::Action::capture());
                }
                None
            }
            _ => None,
        }
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
        if let canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(m)) =
            event
        {
            // Pointer events may arrive before the root rebuilds this view.
            state.control = Some(m.control());
            let drag = state.drag?;
            let point = self.snapped(drag.index, drag.raw, bounds, m.control());
            let change = Message::PageEdit(Event::Corner(drag.index, point));
            return Some(canvas::Action::publish(change).and_capture());
        }
        if let canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) =
            event
        {
            let point = cursor.position_in(bounds)?;
            let steps = match *delta {
                mouse::ScrollDelta::Lines { y, .. } => y,
                mouse::ScrollDelta::Pixels { y, .. } => y / 120.0,
            };
            return Some(self.navigate(
                fastfill_rust::interaction::GestureDelta {
                    pan: GeoPoint::new(0.0, 0.0),
                    zoom_factor: 1.2_f32.powf(steps),
                    zoom_anchor: GeoPoint::new(point.x, point.y),
                },
                bounds,
            ));
        }
        if matches!(event, canvas::Event::Touch(_)) {
            self.touch_input(state, event, bounds)
        } else {
            self.mouse_input(state, event, bounds, cursor)
        }
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
                let turns = if self.mode == Mode::Frame {
                    self.rotation.rem_euclid(4)
                } else {
                    0
                };
                if turns == 0 {
                    frame.draw_image(
                        Rectangle::new(
                            Point::new(rect.x, rect.y),
                            iced::Size::new(rect.width, rect.height),
                        ),
                        image,
                    );
                } else {
                    let center = rect.center();
                    let (width, height) = if turns % 2 == 0 {
                        (rect.width, rect.height)
                    } else {
                        (rect.height, rect.width)
                    };
                    frame.with_save(|frame| {
                        frame.translate(Vector::new(center.x, center.y));
                        #[allow(clippy::cast_precision_loss)]
                        frame
                            .rotate(std::f32::consts::FRAC_PI_2 * turns as f32);
                        frame.draw_image(
                            Rectangle::new(
                                Point::new(-width / 2.0, -height / 2.0),
                                iced::Size::new(width, height),
                            ),
                            image,
                        );
                    });
                }
            }
        } else if self.mode == Mode::Frame
            && self.image.is_some()
            && let Some(rect) = self.rect(bounds)
        {
            let points = corners(self.crop).map(|p| {
                let p = self.display_point(p);
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
        if state.drag.is_some() || state.pan.is_some() {
            return mouse::Interaction::Grabbing;
        }
        if self.mode == Mode::Frame
            && cursor
                .position_in(bounds)
                .is_some_and(|point| self.hit(bounds, point, 12.0).is_some())
        {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}
