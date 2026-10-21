//! Shared native action sizing and styles; no commands or workflow state.

use iced::widget::{Button, button, text};
use iced::{Background, Color, Fill, Shrink, Theme, widget::button::Status};

pub(crate) fn action_button<Message: Clone + 'static>(
    label: &'static str,
    message: Option<Message>,
) -> Button<'static, Message> {
    button(text(label).size(14).width(Fill).height(Fill).center())
        .width(Shrink)
        .height(super::CONTROL_HEIGHT)
        .padding([0, 12])
        .on_press_maybe(message)
        .style(secondary_action)
}

pub(crate) fn secondary_action(theme: &Theme, status: Status) -> button::Style {
    let mut style = button::background(theme, status);
    style.border.width = 1.0;
    style.border.color = theme
        .extended_palette()
        .background
        .strong
        .color
        .scale_alpha(if status == Status::Disabled { 0.4 } else { 1.0 });
    finish(style, status)
}

pub(crate) fn primary_action(theme: &Theme, status: Status) -> button::Style {
    let mut style = button::primary(theme, status);
    if matches!(status, Status::Hovered | Status::Pressed) {
        // SVG children cannot inherit the parent button's hover color.
        let active =
            finish(button::primary(theme, Status::Active), Status::Active);
        style.text_color = active.text_color;
        if let Some(Background::Color(background)) = style.background
            && background.relative_contrast(style.text_color) < 4.5
        {
            style.background = active.background;
        }
    }
    finish(style, status)
}

pub(crate) fn destructive_action(
    theme: &Theme,
    status: Status,
) -> button::Style {
    let mut style = secondary_action(theme, status);
    let palette = theme.extended_palette();
    let mut color = palette.danger.base.color;
    if palette.background.base.color.relative_contrast(color) < 4.5 {
        color =
            iced::theme::palette::mix(color, palette.background.base.text, 0.3);
    }
    style.text_color =
        color.scale_alpha(if status == Status::Disabled { 0.4 } else { 1.0 });
    style.border.color = style.text_color.scale_alpha(
        if matches!(status, Status::Hovered | Status::Pressed) {
            1.0
        } else {
            0.5
        },
    );
    if matches!(status, Status::Hovered | Status::Pressed) {
        style.border.width = 2.0;
        if let Some(Background::Color(background)) = style.background
            && background.relative_contrast(style.text_color) < 4.5
        {
            style.background =
                secondary_action(theme, Status::Active).background;
        }
    }
    finish(style, status)
}

fn finish(mut style: button::Style, status: Status) -> button::Style {
    style.border.radius = 6.0.into();
    if status != Status::Disabled
        && let Some(Background::Color(background)) = style.background
        && background.relative_contrast(style.text_color) < 4.5
    {
        // Native palettes can tint foregrounds just below readable contrast.
        style.text_color = if background.relative_contrast(Color::WHITE)
            >= background.relative_contrast(Color::BLACK)
        {
            Color::WHITE
        } else {
            Color::BLACK
        };
    }
    style
}

#[cfg(test)]
mod tests;
