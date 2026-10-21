//! Text options and the uncommitted font-size field; no document ownership.

use super::{FastFill, Message, TEXT_EDITOR_ID, controller, spinner};
use crate::appearance::{self, CONTROL_GAP, CONTROL_HEIGHT};
use fastfill_rust::annotations::{TextFormat, TextTypeface};
use iced::widget::{
    Row, button, column, container, pick_list, row, rule, text,
};
use iced::{Element, Fill, Font, Task, font, mouse};

const INVALID_SIZE: &str = "Font size must be a number from 6 to 144 pt.";

pub(crate) use super::spinner::Event;
use spinner::DisplayValue as FontSize;

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
                state.finish_stroke_width_input();
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
    spinner::parse(value, 6.0, 144.0)
}

fn step_size(current: f32, direction: i8) -> f32 {
    spinner::step(current, direction, 6.0, 144.0)
}

fn text_size_after_scroll(current: f32, delta: mouse::ScrollDelta) -> f32 {
    spinner::scroll(current, delta, 6.0, 144.0)
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
    let size = spinner::view(
        &spinner::Config {
            id: "font-size-input",
            value,
            placeholder: if editing || common_size {
                "6–144"
            } else {
                "Mixed"
            },
            unit: "pt",
            current: input_size(state, format),
            min: 6.0,
            max: 144.0,
            stacked,
            invalid,
            enabled: true,
        },
        Message::FontSize,
    );
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
        size,
        modifier_button(
            "B",
            Font {
                weight: font::Weight::Bold,
                ..Font::DEFAULT
            },
            false,
            format.bold,
            Message::ToggleTextBold,
        ),
        modifier_button(
            "I",
            Font {
                style: font::Style::Italic,
                ..Font::DEFAULT
            },
            false,
            format.italic,
            Message::ToggleTextItalic,
        ),
        modifier_button(
            "U",
            Font::DEFAULT,
            true,
            format.underline,
            Message::ToggleTextUnderline,
        ),
    ]
}

fn modifier_button(
    label: &'static str,
    font: Font,
    underline: bool,
    selected: bool,
    message: Message,
) -> Element<'static, Message> {
    let glyph: Element<'static, Message> = if underline {
        // Iced rich-text underline renders beside a one-letter glyph.
        container(
            column![
                text(label).font(font).size(16),
                container(rule::horizontal(1).style(move |theme| {
                    let style = if selected {
                        appearance::primary_action(
                            theme,
                            button::Status::Active,
                        )
                    } else {
                        appearance::secondary_action(
                            theme,
                            button::Status::Active,
                        )
                    };
                    rule::Style {
                        color: style.text_color,
                        ..rule::default(theme)
                    }
                }))
                .width(12),
            ]
            .align_x(iced::Alignment::Center),
        )
        .center(Fill)
        .into()
    } else {
        text(label)
            .font(font)
            .size(16)
            .width(Fill)
            .height(Fill)
            .center()
            .into()
    };
    button(glyph)
        .width(CONTROL_HEIGHT)
        .height(CONTROL_HEIGHT)
        .padding(0)
        .on_press(message)
        .style(if selected {
            appearance::primary_action
        } else {
            appearance::secondary_action
        })
        .into()
}

#[cfg(test)]
mod tests;
