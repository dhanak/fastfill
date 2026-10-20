use super::{FastFill, ImagePicker, Message};
use fastfill_rust::annotations::Pointer;
#[cfg(test)]
use iced::Size;
use iced::widget::canvas;
use iced::{Point, Rectangle, Renderer, Theme, Vector, mouse, touch};

mod layout;
mod motion;
mod view;
pub(super) use layout::page_top;
pub(super) use motion::Motion;
pub(super) use view::{overlay, view};

const WIDTH: f32 = 164.0;
const SCROLLBAR_WIDTH: f32 = 16.0;
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct PageDrag {
    pub(super) source: usize,
    pub(super) target: usize,
    origin: Rectangle,
    bounds: Rectangle,
    position: Point,
    grab: Vector,
    moving: bool,
}

impl PageDrag {
    fn has_moved(self, pointer: Point) -> bool {
        let start = self.origin.position() + self.grab;
        self.moving
            || (pointer.x - start.x).hypot(pointer.y - start.y)
                >= DRAG_THRESHOLD
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Start {
        index: usize,
        origin: Rectangle,
        pointer: Point,
        bounds: Rectangle,
    },
    Move {
        pointer: Point,
    },
    Finish {
        pointer: Option<Point>,
    },
    Scroll {
        offset: f32,
    },
}

pub(super) fn update(app: &mut FastFill, event: &Event) {
    match *event {
        Event::Start {
            index,
            origin,
            pointer,
            bounds,
        } if index < app.pages.len() && app.dragged_page.is_none() => {
            app.select_page(index);
            app.accept_text();
            app.cancel_interaction();
            app.dragged_page = Some(PageDrag {
                source: index,
                target: index,
                origin,
                bounds,
                position: origin.position(),
                grab: pointer - origin.position(),
                moving: false,
            });
        }
        Event::Move { pointer } => {
            let target = app.dragged_page.and_then(|drag| {
                (Rail { app }).drop_index_at(pointer, drag.bounds)
            });
            if let Some(drag) = &mut app.dragged_page {
                drag.moving = drag.has_moved(pointer);
                drag.position = pointer - drag.grab;
                if drag.moving
                    && let Some(target) =
                        target.filter(|i| *i < app.pages.len())
                {
                    drag.target = target;
                }
            }
        }
        Event::Finish { pointer } => {
            let target = app.dragged_page.and_then(|drag| {
                pointer.and_then(|point| {
                    (Rail { app }).drop_index_at(point, drag.bounds)
                })
            });
            if let Some(drag) = app.dragged_page.take()
                && let Some(target) = target
                && let Some(pointer) = pointer
            {
                // The valid gap is already visible; do not scroll it away.
                let offset = app.page_scroll;
                app.move_page(if drag.has_moved(pointer) {
                    target
                } else {
                    drag.source
                });
                app.page_scroll = offset;
            }
        }
        Event::Scroll { offset } if offset.is_finite() => {
            app.page_scroll = offset.max(0.0);
            let target = app.dragged_page.and_then(|drag| {
                (Rail { app })
                    .drop_index_at(drag.position + drag.grab, drag.bounds)
            });
            if let Some(drag) = &mut app.dragged_page
                && drag.moving
                && let Some(target) = target.filter(|i| *i < app.pages.len())
            {
                drag.target = target;
            }
        }
        _ => {}
    }
}

struct Rail<'a> {
    app: &'a FastFill,
}

#[derive(Default)]
struct RailState {
    pointer: Option<(Pointer, f32, bool)>,
    epoch: u64,
}

impl Rail<'_> {
    fn scroll(&self, offset: f32, bounds: Rectangle) -> Event {
        let offset = offset.clamp(0.0, self.max_offset(bounds));
        Event::Scroll { offset }
    }

    fn pointer_input(
        &self,
        state: &mut RailState,
        pointer: Pointer,
        phase: Phase,
        position: Option<Point>,
        bounds: Rectangle,
    ) -> Option<canvas::Action<Message>> {
        let position = position.filter(|p| p.x.is_finite() && p.y.is_finite());
        // An unacknowledged press can share a native event batch with moves.
        // Only a new epoch with no drag proves the capture was canceled.
        if state.pointer.is_some()
            && state.epoch != self.app.input_epoch
            && self.app.dragged_page.is_none()
        {
            state.pointer = None;
        }
        if matches!(phase, Phase::Press) {
            if state.pointer.is_some() {
                return Some(canvas::Action::capture());
            }
            let point = position?;
            state.epoch = self.app.input_epoch;
            if bounds.contains(point)
                && point.x >= bounds.x + bounds.width - SCROLLBAR_WIDTH
            {
                state.pointer = Some((pointer, point.y, false));
                return Some(canvas::Action::capture());
            }
            let index = self.index_at(point, bounds)?;
            state.pointer = Some((pointer, point.y, true));
            return Some(
                canvas::Action::publish(Message::PageRail(Event::Start {
                    index,
                    origin: self.shown_card_bounds(index, bounds),
                    pointer: point,
                    bounds,
                }))
                .and_capture(),
            );
        }
        let (owner, last_y, drag) = state.pointer?;
        if pointer != owner {
            return Some(canvas::Action::capture());
        }
        if matches!(phase, Phase::Move) {
            let point = position?;
            state.pointer = Some((owner, point.y, drag));
            let event = if drag {
                Event::Move { pointer: point }
            } else {
                let scale = self.max_offset(bounds)
                    / (bounds.height - self.scroll_thumb_height(bounds))
                        .max(1.0);
                self.scroll(
                    self.offset(bounds) + (point.y - last_y) * scale,
                    bounds,
                )
            };
            return Some(
                canvas::Action::publish(Message::PageRail(event)).and_capture(),
            );
        }
        state.pointer = None;
        if drag {
            let pointer = if matches!(phase, Phase::Release) {
                position
            } else {
                None
            };
            Some(
                canvas::Action::publish(Message::PageRail(Event::Finish {
                    pointer,
                }))
                .and_capture(),
            )
        } else {
            Some(canvas::Action::capture())
        }
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Press,
    Move,
    Release,
    Lost,
}

impl canvas::Program<Message> for Rail<'_> {
    type State = RailState;

    fn update(
        &self,
        state: &mut RailState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let canvas::Event::Window(iced::window::Event::RedrawRequested(at)) =
            event
        {
            return self
                .app
                .rail_motion
                .borrow_mut()
                .sync(self.app, bounds, *at, motion::duration())
                .then(canvas::Action::request_redraw);
        }
        if self.app.image_picker != ImagePicker::Idle
            || !self.app.project.is_idle()
            || self.app.confirm_delete_all
            || self.app.confirm_delete_page
        {
            state.pointer = None;
            return None;
        }
        let (pointer, phase, position) = match event {
            canvas::Event::Mouse(event) => match event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    (Pointer::Mouse, Phase::Press, cursor.position())
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    (Pointer::Mouse, Phase::Release, cursor.position())
                }
                mouse::Event::CursorMoved { position } => {
                    (Pointer::Mouse, Phase::Move, Some(*position))
                }
                mouse::Event::WheelScrolled { delta }
                    if cursor.is_over(bounds)
                        && (state.pointer.is_none()
                            || self.app.dragged_page.is_some()) =>
                {
                    let y = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y * 36.0,
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    return Some(
                        canvas::Action::publish(Message::PageRail(
                            self.scroll(self.offset(bounds) - y, bounds),
                        ))
                        .and_capture(),
                    );
                }
                _ => return None,
            },
            canvas::Event::Touch(event) => {
                let (id, phase, position) = match *event {
                    touch::Event::FingerPressed { id, position } => {
                        (id, Phase::Press, position)
                    }
                    touch::Event::FingerMoved { id, position } => {
                        (id, Phase::Move, position)
                    }
                    touch::Event::FingerLifted { id, position } => {
                        (id, Phase::Release, position)
                    }
                    touch::Event::FingerLost { id, position } => {
                        (id, Phase::Lost, position)
                    }
                };
                (Pointer::Touch(id.0), phase, Some(position))
            }
            _ => return None,
        };
        self.pointer_input(state, pointer, phase, position, bounds)
    }

    fn draw(
        &self,
        _: &RailState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        view::draw(self, renderer, theme, bounds)
    }

    fn mouse_interaction(
        &self,
        _: &RailState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.app.dragged_page.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

#[cfg(test)]
mod tests;
