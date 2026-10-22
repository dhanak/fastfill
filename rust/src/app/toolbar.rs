//! Editor controls and options; annotation behavior stays in the controller.

use super::{FastFill, ImagePicker, Message};
use crate::appearance::{self, CONTROL_GAP, CONTROL_HEIGHT};
use crate::appearance::{TOOLBAR_PADDING, TOOLBAR_ROW_HEIGHT};
use crate::icons::{self, Glyph};
use fastfill_rust::annotations::{AnnotationColor, Tool};
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
    if wide {
        row![super::view::document(state), panel::view(state)]
            .width(Fill)
            .height(Fill)
            .into()
    } else {
        column![tools(state), options(state), super::view::document(state)]
            .width(Fill)
            .height(Fill)
            .into()
    }
}

fn enabled(state: &FastFill) -> bool {
    state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none()
}

fn tools(state: &FastFill) -> Element<'_, Message> {
    let selected = state.annotations().tool();
    let enabled = enabled(state);
    let mut tools = Row::new();
    for tool in Tool::ALL {
        tools = tools.push(tool_button(tool, selected, enabled));
    }
    tools = tools.push(icon_button(
        Glyph::ResetAngle,
        "Reset angle of selected annotations",
        false,
        (enabled && state.annotations().can_reset_rotation())
            .then_some(Message::ResetRotation),
    ));
    tools = tools.extend(actions(state, iced::Shrink, true));
    horizontal(tools)
}

fn actions(
    state: &FastFill,
    width: iced::Length,
    icon_only: bool,
) -> [Element<'static, Message>; 4] {
    let selection = enabled(state)
        && state.annotations().has_selection()
        && !state.annotations().has_text_edit()
        && state.annotations().active_pointer().is_none();
    [
        (
            "To back",
            Glyph::ToBack,
            Message::SendToBack,
            selection,
            false,
        ),
        (
            "To front",
            Glyph::ToFront,
            Message::BringToFront,
            selection,
            false,
        ),
        (
            "Delete",
            Glyph::Delete,
            Message::DeleteSelected,
            selection,
            true,
        ),
        (
            "Delete all",
            Glyph::DeleteAll,
            Message::AskDeleteAll,
            enabled(state)
                && !state.annotations().is_empty()
                && state.annotations().active_pointer().is_none(),
            true,
        ),
    ]
    .map(|(label, glyph, message, available, destructive)| {
        let content: Element<'static, Message> = if icon_only {
            icons::view(glyph, false, available)
        } else {
            container(
                row![
                    icons::view::<Message>(glyph, false, available),
                    text(label)
                        .size(11)
                        .wrapping(iced::advanced::text::Wrapping::None),
                ]
                .spacing(3)
                .align_y(iced::Alignment::Center),
            )
            .width(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
        };
        let control = button(content)
            .width(if icon_only {
                CONTROL_HEIGHT.into()
            } else {
                width
            })
            .height(CONTROL_HEIGHT)
            .padding(if icon_only { [8, 8] } else { [8, 2] })
            .on_press_maybe(available.then_some(message))
            .style(if destructive {
                appearance::destructive_action
            } else {
                appearance::secondary_action
            });
        if icon_only {
            hint(control, label, available)
        } else {
            control.into()
        }
    })
}

fn options(state: &FastFill) -> Element<'_, Message> {
    horizontal(Row::with_children(panel::properties(state, false)))
}

fn horizontal(content: Row<'_, Message>) -> Element<'_, Message> {
    super::touch_cursor::wrap(
        // Always leave eight pixels for a track, avoiding target shifts.
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
        .into(),
    )
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

pub(super) fn icon_button(
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

pub(super) fn hint(
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
