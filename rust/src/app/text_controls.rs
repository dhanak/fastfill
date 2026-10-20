//! Text options and the uncommitted font-size field; no document ownership.

use super::{FastFill, Message, TEXT_EDITOR_ID, controller};
use crate::appearance::{CONTROL_GAP, CONTROL_HEIGHT};
use fastfill_rust::annotations::{TextFormat, TextTypeface};
use iced::widget::text_input;
use iced::widget::{Row, button, column, mouse_area, pick_list, row, text};
use iced::{Element, Fill, Task, mouse};

const INVALID_SIZE: &str = "Font size must be a number from 6 to 144 pt.";

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Input(String),
    Apply,
    Step(i8),
    Scroll(mouse::ScrollDelta),
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if matches!(
        message,
        Message::CancelInteraction | Message::Undo | Message::Redo
    ) && state.font_size_input.is_some()
    {
        state.clear_font_size_input();
        return Some(resume_text(state));
    }
    let Message::FontSize(event) = message else {
        return None;
    };
    let Some(format) = state.annotations().current_text_format() else {
        state.clear_font_size_input();
        return Some(Task::none());
    };
    match event {
        Event::Input(value) => {
            if value.len() <= 32 {
                state.font_size_input = Some(value.clone());
                state.clear_font_size_error();
            }
            return Some(Task::none());
        }
        Event::Apply => {
            if state
                .font_size_input
                .as_deref()
                .is_some_and(|value| parse_size(value).is_none())
            {
                state.font_size_error();
                return Some(Task::none());
            }
            state.finish_font_size_input();
        }
        Event::Step(direction) => {
            let size = step_size(input_size(state, format), *direction);
            controller::handle_text_format(state, &Message::SetTextSize(size));
        }
        Event::Scroll(delta) => {
            let current = input_size(state, format);
            let size = text_size_after_scroll(current, *delta);
            if (size - current).abs() < f32::EPSILON {
                return Some(Task::none());
            }
            controller::handle_text_format(state, &Message::SetTextSize(size));
        }
    }
    Some(resume_text(state))
}

fn resume_text(state: &FastFill) -> Task<Message> {
    if state.annotations().has_text_edit() {
        iced::widget::operation::focus(TEXT_EDITOR_ID)
    } else {
        Task::none()
    }
}

impl FastFill {
    pub(super) fn finish_font_size_input(&mut self) {
        let Some(value) = self.font_size_input.take() else {
            return;
        };
        if let Some(size) = parse_size(&value) {
            controller::handle_text_format(self, &Message::SetTextSize(size));
        } else {
            self.font_size_error();
        }
    }

    pub(super) fn clear_font_size_input(&mut self) {
        self.font_size_input = None;
        self.clear_font_size_error();
    }

    fn clear_font_size_error(&mut self) {
        if self.status == INVALID_SIZE {
            self.status.clear();
            self.status_is_error = false;
        }
    }

    fn font_size_error(&mut self) {
        self.status = INVALID_SIZE.into();
        self.status_is_error = true;
    }
}

fn input_size(state: &FastFill, format: TextFormat) -> f32 {
    state
        .font_size_input
        .as_deref()
        .and_then(parse_size)
        .unwrap_or(format.font_size)
}

fn parse_size(value: &str) -> Option<f32> {
    let size = value.trim().replace(',', ".").parse::<f32>().ok()?;
    (size.is_finite() && (6.0..=144.0).contains(&size)).then_some(size)
}

fn step_size(current: f32, direction: i8) -> f32 {
    (current + f32::from(direction.signum())).clamp(6.0, 144.0)
}

fn text_size_after_scroll(current: f32, delta: mouse::ScrollDelta) -> f32 {
    let y = match delta {
        mouse::ScrollDelta::Lines { y, .. }
        | mouse::ScrollDelta::Pixels { y, .. } => y,
    };
    if y > 0.0 {
        step_size(current, 1)
    } else if y < 0.0 {
        step_size(current, -1)
    } else {
        current
    }
}

struct FontSize(f32);

impl std::fmt::Display for FontSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Round only the label; preserve exact scaled/imported geometry.
        write!(formatter, "{}", (self.0 * 10.0).round() / 10.0)
    }
}

pub(super) fn view(
    state: &FastFill,
    format: TextFormat,
) -> Row<'static, Message> {
    Row::with_children(controls(state, format, false))
        .spacing(CONTROL_GAP)
        .align_y(iced::Alignment::Center)
}

pub(super) fn panel(
    state: &FastFill,
    format: TextFormat,
) -> Element<'static, Message> {
    let [typeface, size, bold, italic, underline] =
        controls(state, format, true);
    column![
        typeface,
        size,
        row![bold, italic, underline].spacing(CONTROL_GAP),
    ]
    .spacing(CONTROL_GAP)
    .into()
}

fn controls(
    state: &FastFill,
    format: TextFormat,
    stacked: bool,
) -> [Element<'static, Message>; 5] {
    let annotations = state.annotations();
    let selected = annotations.selected_text_indices();
    let texts = || selected.iter().filter_map(|index| annotations.text(*index));
    let common_typeface =
        texts().all(|text| text.format.typeface == format.typeface);
    let common_size = texts().all(|text| {
        (text.format.font_size - format.font_size).abs() < f32::EPSILON
    });
    let editing = annotations.has_text_edit();
    let value = state.font_size_input.clone().unwrap_or_else(|| {
        if editing || common_size {
            FontSize(format.font_size).to_string()
        } else {
            String::new()
        }
    });
    let invalid =
        state.font_size_input.is_some() && parse_size(&value).is_none();
    let current = input_size(state, format);
    let step = |label, direction| {
        button(label)
            .on_press_maybe(
                ((step_size(current, direction) - current).abs()
                    >= f32::EPSILON)
                    .then_some(Message::FontSize(Event::Step(direction))),
            )
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT)
    };
    let size = text_input(
        if editing || common_size {
            "6–144"
        } else {
            "Mixed"
        },
        &value,
    )
    .id("font-size-input")
    .on_input(|value| Message::FontSize(Event::Input(value)))
    .on_submit(Message::FontSize(Event::Apply))
    .width(if stacked { Fill } else { 76.into() })
    .size(16)
    .line_height(iced::widget::text::LineHeight::Absolute(20.0.into()))
    .padding([(CONTROL_HEIGHT - 20.0) / 2.0, 8.0])
    .style(move |theme, status| {
        let mut style = text_input::default(theme, status);
        if invalid {
            style.border.color = theme.extended_palette().danger.strong.color;
        }
        style
    });
    let toggle = |label, enabled, message| {
        button(label)
            .on_press(message)
            .style(if enabled {
                button::primary
            } else {
                button::secondary
            })
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT)
    };
    [
        pick_list(
            TextTypeface::ALL,
            (editing || common_typeface).then_some(format.typeface),
            Message::SetTextTypeface,
        )
        .width(if stacked { Fill } else { 130.into() })
        .text_size(16)
        .text_line_height(iced::widget::text::LineHeight::Absolute(20.0.into()))
        .padding([(CONTROL_HEIGHT - 20.0) / 2.0, 8.0])
        .placeholder("Mixed fonts")
        .into(),
        mouse_area(
            row![step("−", -1), size, step("+", 1), text("pt")]
                .width(if stacked { Fill } else { iced::Shrink })
                .spacing(4)
                .align_y(iced::Alignment::Center),
        )
        .on_scroll(|delta| Message::FontSize(Event::Scroll(delta)))
        .into(),
        toggle("B", format.bold, Message::ToggleTextBold).into(),
        toggle("I", format.italic, Message::ToggleTextItalic).into(),
        toggle("U", format.underline, Message::ToggleTextUnderline).into(),
    ]
}

#[cfg(test)]
mod tests;
