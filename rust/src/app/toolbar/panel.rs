//! Wide-window layout; reuse toolbar controls and existing command guards.

use super::{COLORS, actions, color_button, enabled, icon_button, tool_button};
use crate::app::{FastFill, Message, snap, text_controls};
use crate::appearance::{
    self, CONTROL_GAP, CONTROL_HEIGHT, TOOLS_PANEL_PADDING,
};
use crate::icons::Glyph;
use fastfill_rust::annotations::Tool;
use iced::widget::{Column, Row, button, column, container, row, rule};
use iced::widget::{scrollable, text};
use iced::{Element, Fill};

pub(super) fn view(state: &FastFill) -> Element<'_, Message> {
    let selected = state.annotations().tool();
    let tools = Row::with_children(
        Tool::ALL
            .into_iter()
            .map(|tool| tool_button(tool, selected, enabled(state))),
    )
    .spacing(CONTROL_GAP)
    .wrap();
    let properties = properties(state, true);
    let properties: Element<'_, Message> = if properties.is_empty() {
        text("Select an annotation or choose a drawing tool.")
            .size(14)
            .into()
    } else {
        Column::with_children(properties)
            .spacing(CONTROL_GAP)
            .into()
    };
    let content = column![
        row![
            text("Tools").size(18).width(Fill),
            text(selected.label()).size(14)
        ]
        .align_y(iced::Alignment::Center),
        tools,
        rule::horizontal(1),
        properties,
    ]
    .spacing(12)
    .padding(TOOLS_PANEL_PADDING);
    let [back, front, delete, clear] = actions(state, Fill);
    let footer = column![
        rule::horizontal(1),
        text(format!(
            "{} selected · {} annotations",
            state.annotations().selected_count(),
            state.annotations().len(),
        ))
        .size(12),
        row![back, front].spacing(CONTROL_GAP),
        row![delete, clear].spacing(CONTROL_GAP),
    ]
    .spacing(CONTROL_GAP)
    .padding(TOOLS_PANEL_PADDING);
    container(column![
        scrollable(content)
            .direction(scrollable::Direction::Vertical(
                // Overlay the track in padding, never in the tool grid.
                scrollable::Scrollbar::new()
                    .width(6)
                    .scroller_width(6)
                    .margin(2),
            ))
            .height(Fill),
        footer,
    ])
    .width(appearance::TOOLS_PANEL_WIDTH)
    .height(Fill)
    .style(container::bordered_box)
    .into()
}

pub(super) fn properties(
    state: &FastFill,
    stacked: bool,
) -> Vec<Element<'static, Message>> {
    let format = state.annotations().current_text_format();
    let style = state.annotations().current_style();
    let Some(color) =
        format.map(|f| f.color).or_else(|| style.map(|s| s.color))
    else {
        return Vec::new();
    };
    let enabled = enabled(state);
    let mut properties = Vec::new();
    if stacked {
        properties.push(text("Color").size(14).into());
    }
    for colors in COLORS.chunks(if stacked { 3 } else { 6 }) {
        properties.push(
            Row::with_children(colors.iter().map(|(label, value)| {
                color_button(label, *value, color, enabled)
            }))
            .spacing(CONTROL_GAP)
            .into(),
        );
    }
    if let Some(style) = style
        && state.annotations().stroke_available()
    {
        let step = |label, amount, available: bool| {
            button(label)
                .on_press_maybe(
                    (enabled && available)
                        .then_some(Message::AdjustThickness(amount)),
                )
                .width(CONTROL_HEIGHT)
                .height(CONTROL_HEIGHT)
        };
        if stacked {
            properties.push(text("Line width").size(14).into());
        }
        properties.push(
            row![
                step("−", -1.0, style.stroke_width > 1.0),
                text(format!("{:.0} pt", style.stroke_width))
                    .width(if stacked { Fill } else { iced::Shrink })
                    .align_x(iced::alignment::Horizontal::Center),
                step("+", 1.0, style.stroke_width < 20.0),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(CONTROL_GAP)
            .into(),
        );
    }
    if let Some(format) = format {
        if stacked {
            properties.push(text("Text").size(14).into());
            properties.push(text_controls::panel(state, format));
        } else {
            properties.push(text_controls::view(state, format).into());
        }
    }
    if snap::supports(state.annotations().tool()) {
        properties.push(
            row![
                icon_button(
                    Glyph::SmartSnap,
                    "Snap marks to boxes and text to lines",
                    state.smart_snap.enabled,
                    snap::can_toggle(state)
                        .then_some(Message::SmartSnap(snap::Event::Toggle)),
                ),
                column![
                    text("Smart snap").size(14),
                    text(snap::status(state)).size(12),
                ],
            ]
            .align_y(iced::Alignment::Center)
            .spacing(CONTROL_GAP)
            .into(),
        );
    }
    if state.annotations().fill_available() {
        properties.push(
            row![
                icon_button(
                    Glyph::Fill,
                    "Fill closed shapes",
                    state.annotations().current_fill() == Some(true),
                    enabled.then_some(Message::ToggleFill),
                ),
                text("Fill").size(14),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(CONTROL_GAP)
            .into(),
        );
    }
    properties
}

#[cfg(test)]
mod tests;
