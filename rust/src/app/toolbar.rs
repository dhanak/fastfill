//! Editor controls and options; annotation behavior stays in the controller.

use super::{FastFill, ImagePicker, Message};
use crate::appearance::action_button;
use crate::appearance::{self, CONTROL_GAP, CONTROL_HEIGHT, TOOLBAR_HEIGHT};
use crate::appearance::{TOOLBAR_PADDING, TOOLBAR_ROW_HEIGHT};
use crate::icons::{self, Glyph};
use fastfill_rust::annotations::{AnnotationColor, Tool};
use fastfill_rust::geometry::Rect;
use iced::widget::{Row, Space, button, column, container, row, scrollable};
use iced::widget::{text, tooltip};
use iced::{Element, Fill};

mod panel;

const COLORS: [(&str, AnnotationColor); 6] = [
    ("Black", AnnotationColor::Black),
    ("White", AnnotationColor::White),
    ("Red", AnnotationColor::Red),
    ("Blue", AnnotationColor::Blue),
    ("Green", AnnotationColor::Green),
    ("Yellow", AnnotationColor::Yellow),
];

pub(super) fn workspace(
    state: &FastFill,
    available: iced::Size,
) -> Element<'_, Message> {
    let wide = available.width >= appearance::PANEL_MIN_WORKSPACE_WIDTH;
    let canvas_size = iced::Size::new(
        available.width
            - if wide {
                appearance::TOOLS_PANEL_WIDTH
            } else {
                0.0
            },
        (available.height
            - if wide {
                TOOLBAR_ROW_HEIGHT
            } else {
                TOOLBAR_HEIGHT
            })
        .max(1.0),
    );
    let viewport = crate::page_canvas::page_viewport(canvas_size);
    if wide {
        row![
            column![controls(state, viewport), super::view::document(state)]
                .width(Fill)
                .height(Fill),
            panel::view(state),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    } else {
        column![view(state, viewport), super::view::document(state),]
            .width(Fill)
            .height(Fill)
            .into()
    }
}

fn view(state: &FastFill, viewport: Rect) -> Element<'_, Message> {
    column![controls(state, viewport), tools(state), options(state)]
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

fn controls(state: &FastFill, viewport: Rect) -> Element<'_, Message> {
    let enabled = enabled(state);
    let zoom = |factor| Message::ZoomAt {
        factor,
        anchor: viewport.center(),
        viewport,
    };
    horizontal(row![
        action_button(
            "Shortcuts",
            super::help::can_open(state).then_some(Message::ShowShortcuts)
        ),
        action_button(
            "Undo",
            (enabled && state.annotations().can_undo())
                .then_some(Message::Undo)
        ),
        action_button(
            "Redo",
            (enabled && state.annotations().can_redo())
                .then_some(Message::Redo)
        ),
        action_button(
            "Reset angle",
            (enabled && state.annotations().can_reset_rotation())
                .then_some(Message::ResetRotation)
        ),
        action_button("−", enabled.then_some(zoom(0.8))).width(CONTROL_HEIGHT),
        action_button("+", enabled.then_some(zoom(1.25))).width(CONTROL_HEIGHT),
        action_button("1:1", enabled.then_some(Message::OneToOne(viewport)))
            .width(CONTROL_HEIGHT),
        action_button("Fit", enabled.then_some(Message::Fit))
            .width(CONTROL_HEIGHT),
    ])
}

fn tools(state: &FastFill) -> Element<'_, Message> {
    let selected = state.annotations().tool();
    let enabled = enabled(state);
    let mut tools = Row::new();
    for tool in Tool::ALL {
        tools = tools.push(tool_button(tool, selected, enabled));
    }
    tools = tools.push(text(selected.label()).size(14));
    tools = tools.extend(actions(state, iced::Shrink));
    horizontal(tools)
}

fn actions(
    state: &FastFill,
    width: iced::Length,
) -> [Element<'static, Message>; 4] {
    let selection = enabled(state)
        && state.annotations().has_selection()
        && !state.annotations().has_text_edit()
        && state.annotations().active_pointer().is_none();
    [
        ("To back", Message::SendToBack, selection, false),
        ("To front", Message::BringToFront, selection, false),
        ("Delete", Message::DeleteSelected, selection, true),
        (
            "Delete all",
            Message::AskDeleteAll,
            enabled(state)
                && !state.annotations().is_empty()
                && state.annotations().active_pointer().is_none(),
            true,
        ),
    ]
    .map(|(label, message, available, destructive)| {
        action_button(label, available.then_some(message))
            .width(width)
            .style(if destructive {
                appearance::destructive_action
            } else {
                appearance::secondary_action
            })
            .into()
    })
}

fn options(state: &FastFill) -> Element<'_, Message> {
    horizontal(Row::with_children(panel::properties(state, false)))
}

fn horizontal(content: Row<'_, Message>) -> Element<'_, Message> {
    // Always leave eight pixels for a track, avoiding layout/target shifts.
    scrollable(
        content
            .align_y(iced::Alignment::Center)
            .height(TOOLBAR_ROW_HEIGHT - 8.0)
            .padding(TOOLBAR_PADDING)
            .spacing(CONTROL_GAP),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new()
            .width(6)
            .scroller_width(6)
            .spacing(2),
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
