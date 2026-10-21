//! Line-width input; the spinner presentation is shared with font size.

use super::{FastFill, Message, spinner};
use iced::{Element, Task};

pub(crate) use super::spinner::Event;

const INVALID_WIDTH: &str = "Line width must be a number from 1 to 20 px.";

pub(super) fn view(
    state: &FastFill,
    width: f32,
    stacked: bool,
    enabled: bool,
) -> Element<'static, Message> {
    let value = state
        .stroke_width_input
        .clone()
        .unwrap_or_else(|| spinner::DisplayValue(width).to_string());
    let current = parse_width(&value).unwrap_or(width);
    spinner::view(
        &spinner::Config {
            id: "stroke-width-input",
            value,
            placeholder: "1–20",
            unit: "px",
            current,
            min: 1.0,
            max: 20.0,
            stacked,
            invalid: state
                .stroke_width_input
                .as_deref()
                .is_some_and(|value| parse_width(value).is_none()),
            enabled,
        },
        Message::StrokeWidth,
    )
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if matches!(
        message,
        Message::CancelInteraction | Message::Undo | Message::Redo
    ) && state.stroke_width_input.is_some()
    {
        state.clear_stroke_width_input();
        return Some(Task::none());
    }
    let Message::StrokeWidth(event) = message else {
        if matches!(message, Message::SetColor(_) | Message::ToggleFill) {
            state.finish_stroke_width_input();
        }
        return None;
    };
    if !state.annotations().stroke_available() {
        state.clear_stroke_width_input();
        return Some(Task::none());
    }
    let Some(style) = state.annotations().current_style() else {
        state.clear_stroke_width_input();
        return Some(Task::none());
    };
    match event {
        Event::Input(value) => {
            if value.len() <= 32 {
                state.finish_font_size_input();
                state.stroke_width_input = Some(value.clone());
                state.clear_stroke_width_error();
            }
        }
        Event::Apply => {
            if state
                .stroke_width_input
                .as_deref()
                .is_some_and(|value| parse_width(value).is_none())
            {
                state.stroke_width_error();
            } else {
                state.finish_stroke_width_input();
            }
        }
        Event::Step(direction) => {
            let current = input_width(state, style.stroke_width);
            let width = spinner::step(current, *direction, 1.0, 20.0);
            state.clear_stroke_width_input();
            state.annotations_mut().set_stroke_width(width);
        }
        Event::Scroll(delta) => {
            let current = input_width(state, style.stroke_width);
            let width = spinner::scroll(current, *delta, 1.0, 20.0);
            if (width - current).abs() >= f32::EPSILON {
                state.clear_stroke_width_input();
                state.annotations_mut().set_stroke_width(width);
            }
        }
    }
    Some(Task::none())
}

impl FastFill {
    pub(super) fn finish_stroke_width_input(&mut self) {
        let Some(value) = self.stroke_width_input.take() else {
            return;
        };
        if let Some(width) = parse_width(&value) {
            self.annotations_mut().set_stroke_width(width);
        } else {
            self.stroke_width_error();
        }
    }

    pub(super) fn clear_stroke_width_input(&mut self) {
        self.stroke_width_input = None;
        self.clear_stroke_width_error();
    }

    fn clear_stroke_width_error(&mut self) {
        if self.status == INVALID_WIDTH {
            self.status.clear();
            self.status_is_error = false;
        }
    }

    fn stroke_width_error(&mut self) {
        self.status = INVALID_WIDTH.into();
        self.status_is_error = true;
    }
}

fn input_width(state: &FastFill, width: f32) -> f32 {
    state
        .stroke_width_input
        .as_deref()
        .and_then(parse_width)
        .unwrap_or(width)
}

fn parse_width(value: &str) -> Option<f32> {
    spinner::parse(value, 1.0, 20.0)
}

#[cfg(test)]
mod tests;
