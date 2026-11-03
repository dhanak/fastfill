//! Shared numeric spinner presentation and stepping; owners keep their edits.

use crate::appearance::{CONTROL_HEIGHT, action_button};
use iced::widget::text_input;
use iced::widget::{mouse_area, row, text};
use iced::{Element, Fill, mouse};

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Input(String),
    Apply,
    Step(i8),
    Scroll(mouse::ScrollDelta),
}

pub(super) struct Config {
    pub id: &'static str,
    pub value: String,
    pub placeholder: &'static str,
    pub unit: &'static str,
    pub current: f32,
    pub min: f32,
    pub max: f32,
    pub stacked: bool,
    pub invalid: bool,
    pub enabled: bool,
}

pub(super) fn view<Message: Clone + 'static>(
    config: &Config,
    on_event: fn(Event) -> Message,
) -> Element<'static, Message> {
    let invalid = config.invalid;
    let step_button = |label, direction| {
        action_button(
            label,
            (config.enabled
                && (step(config.current, direction, config.min, config.max)
                    - config.current)
                    .abs()
                    >= f32::EPSILON)
                .then_some(on_event(Event::Step(direction))),
        )
        .width(CONTROL_HEIGHT)
    };
    let input = text_input(config.placeholder, &config.value)
        .id(config.id)
        .on_input_maybe(
            config
                .enabled
                .then_some(move |value| on_event(Event::Input(value))),
        )
        .on_submit_maybe(config.enabled.then_some(on_event(Event::Apply)))
        .width(if config.stacked { Fill } else { 60.into() })
        .size(16)
        .line_height(iced::widget::text::LineHeight::Absolute(20.0.into()))
        .padding([
            (CONTROL_HEIGHT - 20.0) / 2.0,
            if config.stacked { 8.0 } else { 6.0 },
        ])
        .style(move |theme, status| {
            let mut style = text_input::default(theme, status);
            if invalid {
                style.border.color =
                    theme.extended_palette().danger.strong.color;
            }
            style
        });
    mouse_area(
        row![
            step_button("−", -1),
            input,
            step_button("+", 1),
            text(config.unit),
        ]
        .width(if config.stacked { Fill } else { iced::Shrink })
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .on_scroll(move |delta| on_event(Event::Scroll(delta)))
    .into()
}

pub(super) fn parse(value: &str, min: f32, max: f32) -> Option<f32> {
    let size = value.trim().replace(',', ".").parse::<f32>().ok()?;
    (size.is_finite() && (min..=max).contains(&size)).then_some(size)
}

pub(super) fn step(current: f32, direction: i8, min: f32, max: f32) -> f32 {
    (current + f32::from(direction.signum())).clamp(min, max)
}

pub(super) fn scroll(
    current: f32,
    delta: mouse::ScrollDelta,
    min: f32,
    max: f32,
) -> f32 {
    let y = match delta {
        mouse::ScrollDelta::Lines { y, .. }
        | mouse::ScrollDelta::Pixels { y, .. } => y,
    };
    if y > 0.0 {
        step(current, 1, min, max)
    } else if y < 0.0 {
        step(current, -1, min, max)
    } else {
        current
    }
}

pub(super) struct DisplayValue(pub f32);

impl std::fmt::Display for DisplayValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Round only the label; preserve exact scaled/imported geometry.
        write!(formatter, "{}", (self.0 * 10.0).round() / 10.0)
    }
}
