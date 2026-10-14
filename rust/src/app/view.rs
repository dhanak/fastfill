use super::{FastFill, ImagePicker, Message, page_size};
use crate::appearance::{
    CONTROL_GAP, CONTROL_HEIGHT, TOOLBAR_HEIGHT, TOOLBAR_PADDING,
    TOOLBAR_ROW_HEIGHT,
};
use crate::page_canvas::{PageCanvas, page_viewport};
use fastfill_rust::annotations::{AnnotationColor, Tool};
use fastfill_rust::geometry::Rect as PageRect;
use iced::widget::{
    Row, button, canvas, column, container, responsive, row, text,
};
use iced::{Element, Fill};

fn heading(state: &FastFill) -> Element<'_, Message> {
    let status = if state.status.is_empty() {
        "Open a PNG or JPEG, or draw on the blank page."
    } else {
        &state.status
    };
    let status_line: Element<'_, Message> = if state.status_is_error {
        container(text(format!("⚠ {status}")).size(14))
            .style(container::danger)
            .padding([6, 10])
            .into()
    } else {
        text(status).size(14).into()
    };
    let mut heading =
        column![text("FastFill").size(24), status_line].spacing(4);
    if state.image_picker == ImagePicker::ConfirmReplace {
        heading = heading.push(
            container(
                row![
                    text("Replace page and discard its annotations?")
                        .width(Fill),
                    button("Keep page").on_press(Message::KeepPage),
                    button("Replace").on_press(Message::ReplacePage),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(8),
            )
            .padding(8)
            .style(container::warning),
        );
    }
    if state.confirm_delete_all {
        heading = heading.push(
            container(
                row![
                    text("Delete every annotation on this page?").width(Fill),
                    button("Keep all").on_press(Message::KeepAll),
                    button("Delete all").on_press(Message::ConfirmDeleteAll),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(8),
            )
            .padding(8)
            .style(container::warning),
        );
    }
    container(heading).padding([10, 20]).into()
}

pub(super) fn view(state: &FastFill) -> Element<'_, Message> {
    let body = responsive(move |available| {
        let canvas_size = iced::Size::new(
            available.width,
            (available.height - TOOLBAR_HEIGHT).max(1.0),
        );
        let viewport = page_viewport(canvas_size);
        column![
            control_row(state, viewport),
            tool_row(state, available.width),
            style_row(state),
            document(state),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    });

    column![heading(state), body]
        .width(Fill)
        .height(Fill)
        .into()
}

fn control_row(state: &FastFill, viewport: PageRect) -> Element<'_, Message> {
    let zoom = |factor| Message::ZoomAt {
        factor,
        anchor: viewport.center(),
        viewport,
    };
    toolbar_row(row![
        button("Open image")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle)
                    .then_some(Message::OpenImage),
            )
            .height(CONTROL_HEIGHT),
        button("Undo")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations.can_undo())
                .then_some(Message::Undo),
            )
            .height(CONTROL_HEIGHT),
        button("Redo")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations.can_redo())
                .then_some(Message::Redo),
            )
            .height(CONTROL_HEIGHT),
        button("Rotation 0°")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations.can_reset_rotation())
                .then_some(Message::ResetRotation),
            )
            .height(CONTROL_HEIGHT),
        button("−").on_press(zoom(0.8)).height(CONTROL_HEIGHT),
        button("+").on_press(zoom(1.25)).height(CONTROL_HEIGHT),
        button("1:1")
            .on_press(Message::OneToOne(viewport))
            .height(CONTROL_HEIGHT),
        button("Fit").on_press(Message::Fit).height(CONTROL_HEIGHT),
    ])
    .into()
}

fn tool_row(state: &FastFill, available_width: f32) -> Element<'_, Message> {
    let selected = state.annotations.tool();
    let mut tools = Row::new();
    for tool in Tool::ALL {
        tools = tools.push(tool_button(tool, selected));
    }
    tools = tools
        .push(
            button("Delete")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && !state.annotations.selected_indices().is_empty()
                        && state.annotations.active_pointer().is_none())
                    .then_some(Message::DeleteSelected),
                )
                .height(CONTROL_HEIGHT),
        )
        .push(
            button("Delete all")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && !state.annotations.shapes().is_empty()
                        && state.annotations.active_pointer().is_none())
                    .then_some(Message::AskDeleteAll),
                )
                .height(CONTROL_HEIGHT),
        )
        .push(
            button("To back")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && !state.annotations.selected_indices().is_empty()
                        && state.annotations.active_pointer().is_none())
                    .then_some(Message::SendToBack),
                )
                .height(CONTROL_HEIGHT),
        )
        .push(
            button("To front")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && !state.annotations.selected_indices().is_empty()
                        && state.annotations.active_pointer().is_none())
                    .then_some(Message::BringToFront),
                )
                .height(CONTROL_HEIGHT),
        );
    if available_width >= 520.0 {
        tools = tools.push(
            text(format!("{} shapes", state.annotations.shapes().len()))
                .size(14),
        );
    }
    toolbar_row(tools).into()
}

fn style_row(state: &FastFill) -> Element<'_, Message> {
    let Some(style) = state.annotations.current_style() else {
        return container(iced::widget::Space::new())
            .height(TOOLBAR_ROW_HEIGHT)
            .into();
    };
    let mut styles = row![
        color_button("Black", AnnotationColor::Black, style.color),
        color_button("White", AnnotationColor::White, style.color),
        color_button("Red", AnnotationColor::Red, style.color),
        color_button("Blue", AnnotationColor::Blue, style.color),
        color_button("Green", AnnotationColor::Green, style.color),
        color_button("Yellow", AnnotationColor::Yellow, style.color),
        button("−")
            .on_press_maybe(
                (style.stroke_width > 1.0)
                    .then_some(Message::AdjustThickness(-1.0)),
            )
            .height(CONTROL_HEIGHT),
        text(format!("{:.0} px", style.stroke_width)).size(14),
        button("+")
            .on_press_maybe(
                (style.stroke_width < 20.0)
                    .then_some(Message::AdjustThickness(1.0)),
            )
            .height(CONTROL_HEIGHT),
    ];
    if state.annotations.fill_available() {
        styles = styles.push(
            button(if state.annotations.current_fill() == Some(true) {
                "Fill ✓"
            } else {
                "Fill"
            })
            .on_press(Message::ToggleFill)
            .style(if state.annotations.current_fill() == Some(true) {
                button::primary
            } else {
                button::secondary
            })
            .height(CONTROL_HEIGHT),
        );
    }
    toolbar_row(styles).into()
}

fn toolbar_row(content: Row<'_, Message>) -> Row<'_, Message> {
    content
        .align_y(iced::Alignment::Center)
        .height(TOOLBAR_ROW_HEIGHT)
        .padding(TOOLBAR_PADDING)
        .spacing(CONTROL_GAP)
}

fn color_button(
    label: &str,
    color: AnnotationColor,
    selected: AnnotationColor,
) -> iced::widget::Button<'_, Message> {
    button(label)
        .on_press(Message::SetColor(color))
        .style(if color == selected {
            button::primary
        } else {
            button::secondary
        })
        .height(CONTROL_HEIGHT)
}

pub(super) fn document(state: &FastFill) -> Element<'_, Message> {
    let page = PageCanvas {
        image: state.page.as_ref(),
        navigation: state.navigation,
        page_size: page_size(state),
        annotations: &state.annotations,
        input_epoch: state.input_epoch,
        input_enabled: state.image_picker == ImagePicker::Idle
            && !state.confirm_delete_all,
        modifiers: state.modifiers,
        background: false,
    };
    // iced batches images above paths within one layer. Stack the overlay.
    let background = PageCanvas {
        background: true,
        input_enabled: false,
        ..page
    };
    iced::widget::stack![
        canvas(background).width(Fill).height(Fill),
        canvas(page).width(Fill).height(Fill),
    ]
    .into()
}

fn tool_button(
    tool: Tool,
    selected: Tool,
) -> iced::widget::Button<'static, Message> {
    button(text(tool.label()))
        .on_press(Message::SetTool(tool))
        .style(if tool == selected {
            button::primary
        } else {
            button::secondary
        })
        .height(CONTROL_HEIGHT)
}
