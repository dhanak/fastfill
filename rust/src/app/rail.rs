use super::{FastFill, ImagePicker, Message};
use crate::appearance::{CONTROL_HEIGHT, accent};
use fastfill_rust::annotations::Pointer;
use iced::widget::{button, canvas, column, container, pick_list, row, text};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Size, Theme};
use iced::{mouse, touch};

const WIDTH: f32 = 164.0;
const SLOT_HEIGHT: f32 = 164.0;
const SCROLLBAR_WIDTH: f32 = 16.0;

#[allow(clippy::cast_precision_loss)]
pub(super) fn slot_top(index: usize) -> f32 {
    // Page count is limited to 20, so this conversion is exact.
    index as f32 * SLOT_HEIGHT
}

pub(super) fn view(state: &FastFill) -> Element<'_, Message> {
    let enabled = state.image_picker == ImagePicker::Idle
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none();
    let control = |label, message, available| {
        button(text(label))
            .on_press_maybe((enabled && available).then_some(message))
            .width(Fill)
            .height(CONTROL_HEIGHT)
    };
    container(
        column![
            row![
                text("Page"),
                pick_list(
                    (1..=state.pages.len()).collect::<Vec<_>>(),
                    Some(
                        state
                            .dragged_page
                            .map_or(state.pages.active_index(), |drag| drag
                                .target,)
                            + 1
                    ),
                    |number| Message::SelectPage(number - 1),
                )
                .width(56),
                text(format!("/ {}", state.pages.len())),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(4),
            control("Add image", Message::AddPage, state.pages.can_add()),
            control("Replace image", Message::AskReplacePage, true),
            control("Delete page", Message::AskDeletePage, true),
            row![
                control(
                    "↑",
                    Message::MovePageLeft,
                    state.pages.active_index() > 0,
                ),
                control(
                    "↓",
                    Message::MovePageRight,
                    state.pages.active_index() + 1 < state.pages.len(),
                ),
            ]
            .spacing(4),
            canvas(Rail { app: state }).width(Fill).height(Fill),
        ]
        .spacing(6),
    )
    .width(WIDTH)
    .height(Fill)
    .padding(8)
    .style(container::bordered_box)
    .into()
}

struct Rail<'a> {
    app: &'a FastFill,
}

#[derive(Default)]
struct RailState {
    pointer: Option<(Pointer, f32, bool)>,
}

impl Rail<'_> {
    fn scroll_thumb_height(&self, bounds: Rectangle) -> f32 {
        (bounds.height * bounds.height / slot_top(self.app.pages.len()))
            .max(32.0)
            .min(bounds.height)
    }

    fn offset(&self, bounds: Rectangle) -> f32 {
        self.app.page_scroll.clamp(
            0.0,
            (slot_top(self.app.pages.len()) - bounds.height).max(0.0),
        )
    }

    fn index_at(&self, point: Point, bounds: Rectangle) -> Option<usize> {
        if !bounds.contains(point) {
            return None;
        }
        let y = point.y - bounds.y + self.offset(bounds);
        (0..self.app.pages.len())
            .find(|index| (slot_top(*index)..slot_top(*index + 1)).contains(&y))
    }

    fn drop_index_at(&self, point: Point, bounds: Rectangle) -> Option<usize> {
        if !bounds.contains(point) {
            return None;
        }
        self.index_at(point, bounds).or_else(|| {
            let y = point.y - bounds.y + self.offset(bounds);
            (y >= slot_top(self.app.pages.len()))
                .then_some(self.app.pages.len() - 1)
        })
    }

    fn display_slot(&self, index: usize) -> usize {
        let Some(drag) = self.app.dragged_page else {
            return index;
        };
        if index == drag.source {
            drag.target
        } else if drag.source < index && index <= drag.target {
            index - 1
        } else if drag.target <= index && index < drag.source {
            index + 1
        } else {
            index
        }
    }

    fn pointer_input(
        &self,
        state: &mut RailState,
        pointer: Pointer,
        phase: Phase,
        position: Option<Point>,
        bounds: Rectangle,
    ) -> Option<canvas::Action<Message>> {
        if matches!(phase, Phase::Press) {
            if state.pointer.is_some() {
                return Some(canvas::Action::capture());
            }
            let point = position?;
            if bounds.contains(point)
                && point.x >= bounds.x + bounds.width - SCROLLBAR_WIDTH
            {
                state.pointer = Some((pointer, point.y, false));
                return Some(canvas::Action::capture());
            }
            let index = self.index_at(point, bounds)?;
            state.pointer = Some((pointer, point.y, true));
            return Some(
                canvas::Action::publish(Message::PageDragStarted(index))
                    .and_capture(),
            );
        }
        let (owner, last_y, drag) = state.pointer?;
        if pointer != owner {
            return Some(canvas::Action::capture());
        }
        if matches!(phase, Phase::Move) {
            if drag && self.app.dragged_page.is_none() {
                state.pointer = None;
                return None;
            }
            let point = position?;
            state.pointer = Some((owner, point.y, drag));
            let message = if drag {
                self.drop_index_at(point, bounds)
                    .map(Message::PageDragEntered)
            } else {
                let scale = (slot_top(self.app.pages.len()) - bounds.height)
                    .max(0.0)
                    / (bounds.height - self.scroll_thumb_height(bounds))
                        .max(1.0);
                Some(Message::ScrollPages(
                    self.offset(bounds) + (point.y - last_y) * scale,
                ))
            };
            return Some(message.map_or_else(canvas::Action::capture, |m| {
                canvas::Action::publish(m).and_capture()
            }));
        }
        state.pointer = None;
        if drag {
            let target = if matches!(phase, Phase::Release) {
                position.and_then(|point| self.drop_index_at(point, bounds))
            } else {
                None
            };
            Some(
                canvas::Action::publish(target.map_or(
                    Message::PageDragCanceled,
                    Message::PageDragFinished,
                ))
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
        if self.app.image_picker != ImagePicker::Idle
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
                        canvas::Action::publish(Message::ScrollPages(
                            self.offset(bounds) - y,
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
        _state: &RailState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let width = (bounds.width - SCROLLBAR_WIDTH).max(1.0);
        for (index, page) in self.app.pages.iter().enumerate() {
            let slot = self.display_slot(index);
            let y = slot_top(slot) - self.offset(bounds);
            if y + SLOT_HEIGHT < 0.0 || y > bounds.height {
                continue;
            }
            let selected = index == self.app.pages.active_index();
            let color = if selected { accent() } else { Color::BLACK };
            frame.fill_rectangle(
                Point::new(1.0, y + 6.0),
                Size::new(width - 2.0, SLOT_HEIGHT - 14.0),
                if selected {
                    Color::from_rgb8(219, 235, 255)
                } else {
                    Color::from_rgb8(238, 238, 238)
                },
            );
            if let Some(image) = &page.image {
                let scale = ((width - 16.0) / image.size.width)
                    .min(72.0 / image.size.height);
                let size = Size::new(
                    image.size.width * scale,
                    image.size.height * scale,
                );
                frame.draw_image(
                    Rectangle::new(
                        Point::new((width - size.width) / 2.0, y + 10.0),
                        size,
                    ),
                    &image.handle,
                );
            } else {
                frame.fill_rectangle(
                    Point::new((width - 50.0) / 2.0, y + 10.0),
                    Size::new(50.0, 72.0),
                    Color::WHITE,
                );
            }
            let name = page.image.as_ref().map_or("Blank", |p| &p.name);
            let label: String = name.chars().take(17).collect();
            for (content, top) in [
                (format!("{}. {label}", slot + 1), 84.0),
                (format!("{} annotations", page.annotations.len()), 102.0),
                ("↕ Drag page to reorder".into(), SLOT_HEIGHT - 28.0),
            ] {
                frame.fill_text(canvas::Text {
                    content,
                    position: Point::new(7.0, y + top),
                    size: 12.0.into(),
                    color,
                    ..canvas::Text::default()
                });
            }
            if self
                .app
                .dragged_page
                .is_some_and(|drag| drag.source == index)
            {
                frame.stroke_rectangle(
                    Point::new(3.0, y + 8.0),
                    Size::new(width - 6.0, SLOT_HEIGHT - 18.0),
                    canvas::Stroke::default()
                        .with_color(accent())
                        .with_width(3.0),
                );
                frame.fill_rectangle(
                    Point::new(0.0, y + SLOT_HEIGHT - 6.0),
                    Size::new(width, 4.0),
                    accent(),
                );
            }
        }
        let total = slot_top(self.app.pages.len());
        if total > bounds.height && bounds.height > 0.0 {
            let height = self.scroll_thumb_height(bounds);
            let top = self.offset(bounds) / (total - bounds.height)
                * (bounds.height - height);
            frame.fill_rectangle(
                Point::new(width + 4.0, 0.0),
                Size::new(8.0, bounds.height),
                Color::from_rgb8(230, 230, 230),
            );
            frame.fill_rectangle(
                Point::new(width + 4.0, top),
                Size::new(8.0, height),
                Color::from_rgb8(120, 120, 120),
            );
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &RailState,
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
