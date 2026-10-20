//! Editor controls and options; annotation behavior stays in the controller.

use super::{FastFill, ImagePicker, Message};
use crate::appearance::{self, CONTROL_GAP, CONTROL_HEIGHT, TOOLBAR_HEIGHT};
use crate::appearance::{TOOLBAR_PADDING, TOOLBAR_ROW_HEIGHT};
use crate::icons::{self, Glyph};
use fastfill_rust::annotations::{AnnotationColor, Tool};
use fastfill_rust::geometry::Rect;
use iced::widget::{Row, Space, button, column, container, row, scrollable};
use iced::widget::{text, tooltip};
use iced::{Element, Fill};

const COLORS: [(&str, AnnotationColor); 6] = [
    ("Black", AnnotationColor::Black),
    ("White", AnnotationColor::White),
    ("Red", AnnotationColor::Red),
    ("Blue", AnnotationColor::Blue),
    ("Green", AnnotationColor::Green),
    ("Yellow", AnnotationColor::Yellow),
];

pub(super) fn view(
    state: &FastFill,
    viewport: Rect,
    width: f32,
) -> Element<'_, Message> {
    column![
        controls(state, viewport),
        tools(state, width),
        options(state)
    ]
    .width(Fill)
    .height(TOOLBAR_HEIGHT)
    .into()
}

fn enabled(state: &FastFill) -> bool {
    state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none()
}

fn controls(state: &FastFill, viewport: Rect) -> Row<'_, Message> {
    let enabled = enabled(state);
    let zoom = |factor| Message::ZoomAt {
        factor,
        anchor: viewport.center(),
        viewport,
    };
    toolbar_row(row![
        button("Undo")
            .on_press_maybe(
                (enabled && state.annotations().can_undo())
                    .then_some(Message::Undo)
            )
            .height(CONTROL_HEIGHT),
        button("Redo")
            .on_press_maybe(
                (enabled && state.annotations().can_redo())
                    .then_some(Message::Redo)
            )
            .height(CONTROL_HEIGHT),
        button("Rotation 0°")
            .on_press_maybe(
                (enabled && state.annotations().can_reset_rotation())
                    .then_some(Message::ResetRotation)
            )
            .height(CONTROL_HEIGHT),
        button("−")
            .on_press_maybe(enabled.then_some(zoom(0.8)))
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT),
        button("+")
            .on_press_maybe(enabled.then_some(zoom(1.25)))
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT),
        button("1:1")
            .on_press_maybe(enabled.then_some(Message::OneToOne(viewport)))
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT),
        button("Fit")
            .on_press_maybe(enabled.then_some(Message::Fit))
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT),
    ])
}

fn tools(state: &FastFill, width: f32) -> Element<'_, Message> {
    let selected = state.annotations().tool();
    let enabled = enabled(state);
    let mut tools = Row::new();
    for tool in Tool::ALL {
        tools = tools.push(tool_button(tool, selected, enabled));
    }
    tools = tools.push(text(selected.label()).size(14));
    let selection = enabled
        && state.annotations().has_selection()
        && !state.annotations().has_text_edit()
        && state.annotations().active_pointer().is_none();
    let action = |label: &'static str, message, available: bool| {
        button(text(label))
            .on_press_maybe(available.then_some(message))
            .height(CONTROL_HEIGHT)
    };
    tools = tools
        .push(action("Delete", Message::DeleteSelected, selection))
        .push(action(
            "Delete all",
            Message::AskDeleteAll,
            enabled
                && !state.annotations().is_empty()
                && state.annotations().active_pointer().is_none(),
        ))
        .push(action("To back", Message::SendToBack, selection))
        .push(action("To front", Message::BringToFront, selection));
    if width >= 1_200.0 {
        tools = tools.push(
            text(format!("{} annotations", state.annotations().len())).size(14),
        );
    }
    horizontal(tools)
}

fn options(state: &FastFill) -> Element<'_, Message> {
    let format = state.annotations().current_text_format();
    let style = state.annotations().current_style();
    let Some(color) =
        format.map(|f| f.color).or_else(|| style.map(|s| s.color))
    else {
        return Space::new().height(TOOLBAR_ROW_HEIGHT).into();
    };
    let enabled = enabled(state);
    let mut options = Row::new();
    if super::snap::supports(state.annotations().tool()) {
        let status = super::snap::status(state);
        options = options.push(icon_button(
            Glyph::SmartSnap,
            format!("Smart snap: {status}. Snap marks to boxes and text to lines."),
            state.smart_snap.enabled,
            super::snap::can_toggle(state)
                .then_some(Message::SmartSnap(super::snap::Event::Toggle)),
        )).push(text(status).size(14));
    }
    if let Some(format) = format {
        options = options.push(super::text_controls::view(state, format));
    }
    for (label, value) in COLORS {
        options = options.push(color_button(label, value, color, enabled));
    }
    if let Some(style) = style
        && state.annotations().stroke_available()
    {
        options = options
            .push(
                button("−")
                    .on_press_maybe(
                        (enabled && style.stroke_width > 1.0)
                            .then_some(Message::AdjustThickness(-1.0)),
                    )
                    .width(CONTROL_HEIGHT)
                    .height(CONTROL_HEIGHT),
            )
            .push(text(format!("{:.0} pt", style.stroke_width)).size(14))
            .push(
                button("+")
                    .on_press_maybe(
                        (enabled && style.stroke_width < 20.0)
                            .then_some(Message::AdjustThickness(1.0)),
                    )
                    .width(CONTROL_HEIGHT)
                    .height(CONTROL_HEIGHT),
            );
    }
    if state.annotations().fill_available() {
        options = options.push(icon_button(
            Glyph::Fill,
            "Fill closed shapes",
            state.annotations().current_fill() == Some(true),
            enabled.then_some(Message::ToggleFill),
        ));
    }
    horizontal(options)
}

fn toolbar_row(content: Row<'_, Message>) -> Row<'_, Message> {
    content
        .align_y(iced::Alignment::Center)
        .height(TOOLBAR_ROW_HEIGHT)
        .padding(TOOLBAR_PADDING)
        .spacing(CONTROL_GAP)
}

fn horizontal(content: Row<'_, Message>) -> Element<'_, Message> {
    scrollable(toolbar_row(content))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .height(TOOLBAR_ROW_HEIGHT)
        .into()
}

fn tool_button(
    tool: Tool,
    selected: Tool,
    enabled: bool,
) -> Element<'static, Message> {
    icon_button(
        Glyph::Tool(tool),
        tool.label(),
        tool == selected,
        enabled.then_some(Message::SetTool(tool)),
    )
}

fn icon_button(
    glyph: Glyph,
    label: impl Into<String>,
    selected: bool,
    message: Option<Message>,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    hint(
        button(icons::view(glyph, selected, enabled))
            .on_press_maybe(message)
            .width(CONTROL_HEIGHT)
            .height(CONTROL_HEIGHT)
            .padding((CONTROL_HEIGHT - appearance::TOOL_ICON_SIZE) / 2.0)
            .style(move |theme, status| {
                appearance::tool_button(theme, status, selected)
            }),
        label,
        enabled,
    )
}

fn color_button(
    label: &'static str,
    color: AnnotationColor,
    selected: AnnotationColor,
    enabled: bool,
) -> Element<'static, Message> {
    hint(
        button(
            container(
                Space::new()
                    .width(appearance::COLOR_SWATCH_SIZE)
                    .height(appearance::COLOR_SWATCH_SIZE),
            )
            .style(move |theme| appearance::color_patch(theme, color, enabled)),
        )
        .on_press_maybe(enabled.then_some(Message::SetColor(color)))
        .width(CONTROL_HEIGHT)
        .height(CONTROL_HEIGHT)
        .padding((CONTROL_HEIGHT - appearance::COLOR_SWATCH_SIZE) / 2.0)
        .style(move |theme, status| {
            appearance::color_button(theme, status, color == selected)
        }),
        label,
        enabled,
    )
}

fn hint(
    content: impl Into<Element<'static, Message>>,
    label: impl Into<String>,
    enabled: bool,
) -> Element<'static, Message> {
    let content = content.into();
    // Never leave an underlying tooltip above a modal/busy editor.
    if !enabled {
        return content;
    }
    tooltip(
        content,
        container(text(label.into()).size(14))
            .padding([6, 10])
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .delay(std::time::Duration::from_millis(500))
    .into()
}

#[cfg(test)]
mod tests;
