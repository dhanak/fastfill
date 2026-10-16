use super::{FastFill, ImagePicker, Message, TEXT_EDITOR_ID, page_size};
use crate::appearance::{
    CONTROL_GAP, CONTROL_HEIGHT, TOOLBAR_HEIGHT, TOOLBAR_PADDING,
    TOOLBAR_ROW_HEIGHT, annotation_color, text_font,
};
use crate::page_canvas::{CanvasLayer, PageCanvas, page_viewport};
use fastfill_rust::annotations::{
    AnnotationColor, TEXT_EDITOR_HORIZONTAL_PADDING,
    TEXT_EDITOR_VERTICAL_PADDING, TEXT_LINE_HEIGHT, TextFormat, TextTypeface,
    Tool,
};
use fastfill_rust::geometry::{Point as PagePoint, Rect as PageRect};
use iced::widget::{
    Row, button, canvas, column, container, mouse_area, pick_list,
    progress_bar, responsive, row, scrollable, text, text_editor,
};
use iced::{
    Background, Border, Color, Element, Fill, Padding, keyboard, mouse,
};

fn heading(state: &FastFill) -> Element<'_, Message> {
    let status = if state.status.is_empty() {
        "Add an image, import a PDF, or draw on the blank page."
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
        column![super::project_view::controls(state), status_line].spacing(4);
    if let Some(warning) = super::page_edit::warning(state) {
        heading = heading.push(warning);
    }
    if let Some(confirmation) = super::project_view::confirmation(state) {
        heading = heading.push(confirmation);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(banner) = super::recovery::banner(state) {
        heading = heading.push(banner);
    }
    let pdf_progress = match &state.project.operation {
        super::project::Operation::ImportingPdf { progress, .. } => Some((
            *progress,
            "Cancel PDF import",
            Some(Message::CancelPdfImport),
        )),
        super::project::Operation::ExportingPdf {
            progress, cancel, ..
        } => Some((
            *progress,
            "Cancel PDF export",
            (!cancel.load(std::sync::atomic::Ordering::Relaxed))
                .then_some(Message::CancelPdfExport),
        )),
        _ => None,
    };
    if let Some((progress, label, message)) = pdf_progress {
        heading = heading.push(
            row![
                progress_bar(0.0..=1.0, progress).girth(8).length(Fill),
                button(label).on_press_maybe(message).height(CONTROL_HEIGHT),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        );
    }
    if state.image_picker == ImagePicker::ConfirmReplace {
        heading = heading.push(
            container(
                row![
                    text("Replace page and discard its annotations?")
                        .width(Fill),
                    button("Keep page").on_press(Message::KeepPage),
                    button("Replace").on_press(Message::ConfirmReplacePage),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(8),
            )
            .padding(8)
            .style(container::warning),
        );
    }
    if state.confirm_delete_page {
        heading = heading.push(
            container(
                row![
                    text("Delete this page and its annotations?").width(Fill),
                    button("Keep page").on_press(Message::KeepPage),
                    button("Delete page").on_press(Message::ConfirmDeletePage),
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
    if state.page_editor.is_open() {
        return super::page_edit::view(state);
    }
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

    column![heading(state), row![super::rail::view(state), body]]
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
        button("Undo")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations().can_undo())
                .then_some(Message::Undo),
            )
            .height(CONTROL_HEIGHT),
        button("Redo")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations().can_redo())
                .then_some(Message::Redo),
            )
            .height(CONTROL_HEIGHT),
        button("Rotation 0°")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations().can_reset_rotation())
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
    let selected = state.annotations().tool();
    let mut tools = Row::new();
    for tool in Tool::ALL {
        tools = tools.push(tool_button(tool, selected));
    }
    tools = tools
        .push(
            button("Delete")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && state.annotations().has_selection()
                        && !state.annotations().has_text_edit()
                        && state.annotations().active_pointer().is_none())
                    .then_some(Message::DeleteSelected),
                )
                .height(CONTROL_HEIGHT),
        )
        .push(
            button("Delete all")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && !state.annotations().is_empty()
                        && state.annotations().active_pointer().is_none())
                    .then_some(Message::AskDeleteAll),
                )
                .height(CONTROL_HEIGHT),
        )
        .push(
            button("To back")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && state.annotations().has_selection()
                        && !state.annotations().has_text_edit()
                        && state.annotations().active_pointer().is_none())
                    .then_some(Message::SendToBack),
                )
                .height(CONTROL_HEIGHT),
        )
        .push(
            button("To front")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && state.annotations().has_selection()
                        && !state.annotations().has_text_edit()
                        && state.annotations().active_pointer().is_none())
                    .then_some(Message::BringToFront),
                )
                .height(CONTROL_HEIGHT),
        );
    if available_width >= 520.0 {
        tools = tools.push(
            text(format!("{} annotations", state.annotations().len())).size(14),
        );
    }
    scrollable(toolbar_row(tools))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .height(TOOLBAR_ROW_HEIGHT)
        .into()
}

fn style_row(state: &FastFill) -> Element<'_, Message> {
    let format = state.annotations().current_text_format();
    let style = state.annotations().current_style();
    let Some(color) = format
        .map(|format| format.color)
        .or_else(|| style.map(|style| style.color))
    else {
        return container(iced::widget::Space::new())
            .height(TOOLBAR_ROW_HEIGHT)
            .into();
    };
    let mut styles = format
        .map_or_else(Row::new, |format| text_style_controls(state, format));
    for (label, value) in [
        ("Black", AnnotationColor::Black),
        ("White", AnnotationColor::White),
        ("Red", AnnotationColor::Red),
        ("Blue", AnnotationColor::Blue),
        ("Green", AnnotationColor::Green),
        ("Yellow", AnnotationColor::Yellow),
    ] {
        styles = styles.push(color_button(label, value, color));
    }
    if let Some(style) = style
        && (state.annotations().tool() != Tool::Select
            || !state.annotations().selected_indices().is_empty())
    {
        styles = styles
            .push(
                button("−")
                    .on_press_maybe(
                        (style.stroke_width > 1.0)
                            .then_some(Message::AdjustThickness(-1.0)),
                    )
                    .height(CONTROL_HEIGHT),
            )
            .push(text(format!("{:.0} pt", style.stroke_width)).size(14))
            .push(
                button("+")
                    .on_press_maybe(
                        (style.stroke_width < 20.0)
                            .then_some(Message::AdjustThickness(1.0)),
                    )
                    .height(CONTROL_HEIGHT),
            );
    }
    if state.annotations().fill_available() {
        styles = styles.push(
            button(if state.annotations().current_fill() == Some(true) {
                "Fill ✓"
            } else {
                "Fill"
            })
            .on_press(Message::ToggleFill)
            .style(if state.annotations().current_fill() == Some(true) {
                button::primary
            } else {
                button::secondary
            })
            .height(CONTROL_HEIGHT),
        );
    }
    scrollable(toolbar_row(styles))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .height(TOOLBAR_ROW_HEIGHT)
        .into()
}

const TEXT_SIZES: [f32; 10] =
    [8.0, 10.0, 12.0, 16.0, 20.0, 24.0, 32.0, 40.0, 48.0, 64.0];

#[derive(Clone, Copy, PartialEq)]
struct FontSize(f32);

impl std::fmt::Display for FontSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Round only the label; preserve exact scaled/imported geometry.
        write!(formatter, "{}", (self.0 * 10.0).round() / 10.0)
    }
}

fn text_style_controls(
    state: &FastFill,
    format: TextFormat,
) -> Row<'static, Message> {
    let selected = state.annotations().selected_text_indices();
    let common_typeface = selected.iter().all(|index| {
        state.annotations().texts()[*index].format.typeface == format.typeface
    });
    let common_size = selected.iter().all(|index| {
        (state.annotations().texts()[*index].format.font_size
            - format.font_size)
            .abs()
            < f32::EPSILON
    });
    let editing = state.annotations().has_text_edit();
    let toggle = |label, enabled, message| {
        button(label)
            .on_press(message)
            .style(if enabled {
                button::primary
            } else {
                button::secondary
            })
            .height(CONTROL_HEIGHT)
    };
    row![
        pick_list(
            TextTypeface::ALL,
            (editing || common_typeface).then_some(format.typeface),
            Message::SetTextTypeface,
        )
        .width(130)
        .placeholder("Mixed fonts"),
        mouse_area(
            pick_list(
                TEXT_SIZES.map(FontSize),
                (editing || common_size).then_some(FontSize(format.font_size)),
                |size| Message::SetTextSize(size.0),
            )
            .width(75)
            .placeholder("Mixed"),
        )
        .on_scroll(move |delta| {
            Message::SetTextSize(text_size_after_scroll(
                format.font_size,
                delta,
            ))
        }),
        toggle("B", format.bold, Message::ToggleTextBold),
        toggle("I", format.italic, Message::ToggleTextItalic),
        toggle("U", format.underline, Message::ToggleTextUnderline),
    ]
}

fn text_size_after_scroll(current: f32, delta: mouse::ScrollDelta) -> f32 {
    let y = match delta {
        mouse::ScrollDelta::Lines { y, .. }
        | mouse::ScrollDelta::Pixels { y, .. } => y,
    };
    if y > 0.0 {
        TEXT_SIZES
            .iter()
            .copied()
            .find(|size| *size > current)
            .unwrap_or(current)
    } else if y < 0.0 {
        TEXT_SIZES
            .iter()
            .copied()
            .rev()
            .find(|size| *size < current)
            .unwrap_or(current)
    } else {
        current
    }
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
    responsive(move |available| {
        let page_size = page_size(state);
        let page = PageCanvas {
            image: state.image(),
            navigation: state.navigation,
            page_size,
            annotations: state.annotations(),
            input_epoch: state.input_epoch,
            input_enabled: state.image_picker == ImagePicker::Idle
                && state.project.is_idle()
                && !state.confirm_delete_all
                && !state.confirm_delete_page
                && state.dragged_page.is_none(),
            modifiers: state.modifiers,
            layer: CanvasLayer::Overlay,
        };
        // iced batches unlike primitives within one canvas. Stack each layer.
        let background = PageCanvas {
            layer: CanvasLayer::Background,
            input_enabled: false,
            ..page
        };
        // iced leaves a stack's first child unlayered. An empty base makes
        // even the page background respect the canvas viewport's clip.
        let mut layers = iced::widget::stack![
            iced::widget::Space::new().width(Fill).height(Fill),
            canvas(background).width(Fill).height(Fill),
        ];
        for annotation in state.annotations().ordered_annotations() {
            let annotation = PageCanvas {
                layer: CanvasLayer::Annotation(annotation),
                input_enabled: false,
                ..page
            };
            layers = layers.push(canvas(annotation).width(Fill).height(Fill));
        }
        layers = layers.push(canvas(page).width(Fill).height(Fill));
        if let Some((_, bounds, format, _)) = state.annotations().text_edit()
            && let Some(view) =
                state.navigation.view(page_size, page_viewport(available))
        {
            let page = view.page_rect();
            let scale = page.width / page_size.width;
            let position =
                view.page_to_view(PagePoint::new(bounds.x, bounds.y));
            let width =
                bounds.width * scale + 2.0 * TEXT_EDITOR_HORIZONTAL_PADDING;
            let height =
                bounds.height * scale + 2.0 * TEXT_EDITOR_VERTICAL_PADDING;
            let left = position.x - TEXT_EDITOR_HORIZONTAL_PADDING;
            let top = position.y - TEXT_EDITOR_VERTICAL_PADDING;
            let editor = text_editor(&state.text_content)
                .id(TEXT_EDITOR_ID)
                .placeholder("Text")
                .on_action(Message::TextAction)
                .key_binding(text_key_binding)
                .padding([
                    TEXT_EDITOR_VERTICAL_PADDING,
                    TEXT_EDITOR_HORIZONTAL_PADDING,
                ])
                .font(text_font(format))
                .size(format.font_size * scale)
                .line_height(TEXT_LINE_HEIGHT)
                .wrapping(iced::advanced::text::Wrapping::None)
                .width(width)
                .height(height)
                .style(move |theme, status| {
                    inline_text_style(theme, status, format.color)
                });
            layers = layers.push(
                container(editor).width(Fill).height(Fill).padding(Padding {
                    top,
                    left,
                    ..Padding::ZERO
                }),
            );
        }
        layers.width(Fill).height(Fill).clip(true).into()
    })
    .into()
}

fn inline_text_style(
    theme: &iced::Theme,
    status: text_editor::Status,
    color: AnnotationColor,
) -> text_editor::Style {
    let style = text_editor::default(theme, status);
    text_editor::Style {
        background: Background::Color(Color::from_rgba8(255, 255, 255, 0.72)),
        border: Border {
            width: 2.0,
            color: Color::from_rgb8(0, 102, 204),
            ..style.border
        },
        placeholder: Color::from_rgb8(72, 72, 72),
        value: annotation_color(color),
        ..style
    }
}

fn text_key_binding(
    key: text_editor::KeyPress,
) -> Option<text_editor::Binding<Message>> {
    if accepts_text(&key.key, key.modifiers) {
        Some(text_editor::Binding::Custom(Message::AcceptText))
    } else {
        text_editor::Binding::from_key_press(key)
    }
}

fn accepts_text(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> bool {
    matches!(
        key.as_ref(),
        keyboard::Key::Named(keyboard::key::Named::Enter)
    ) && !modifiers.shift()
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

#[cfg(test)]
mod tests {
    use super::{FontSize, accepts_text, text_size_after_scroll};
    use iced::{keyboard, mouse};

    #[test]
    fn plain_enter_accepts_but_shift_enter_does_not() {
        let enter = keyboard::Key::Named(keyboard::key::Named::Enter);

        assert!(accepts_text(&enter, keyboard::Modifiers::empty()));
        assert!(!accepts_text(&enter, keyboard::Modifiers::SHIFT));
    }

    #[test]
    fn font_size_wheel_steps_and_stops_at_limits() {
        let up = mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };
        let down = mouse::ScrollDelta::Pixels { x: 0.0, y: -1.0 };

        assert_eq!(text_size_after_scroll(24.0, up), 32.0);
        assert_eq!(text_size_after_scroll(24.0, down), 20.0);
        assert_eq!(text_size_after_scroll(64.0, up), 64.0);
        assert_eq!(text_size_after_scroll(8.0, down), 8.0);
    }

    #[test]
    fn scaled_font_sizes_have_compact_labels_without_rounding_the_value() {
        for (value, label) in [
            (24.0, "24"),
            (22.000_002, "22"),
            (14.666_667, "14.7"),
            (143.999_98, "144"),
            (6.000_001, "6"),
        ] {
            let size = FontSize(value);
            assert_eq!(size.to_string(), label);
            assert_eq!(size.0.to_bits(), value.to_bits());
        }
    }
}
