use super::{FastFill, ImagePicker, Message, TEXT_EDITOR_ID, page_size};
use crate::appearance::{CONTROL_HEIGHT, annotation_color, text_font};
use crate::page_canvas::{CanvasLayer, PageCanvas, page_viewport};
use fastfill_rust::annotations::{
    AnnotationColor, TEXT_EDITOR_HORIZONTAL_PADDING,
    TEXT_EDITOR_VERTICAL_PADDING, TEXT_LINE_HEIGHT,
};
use fastfill_rust::geometry::Point as PagePoint;
use iced::widget::{
    button, canvas, column, container, progress_bar, responsive, row, text,
    text_editor,
};
use iced::{Background, Border, Color, Element, Fill, Padding, keyboard};

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
    container(heading).padding([10, 20]).into()
}

pub(super) fn view(state: &FastFill) -> Element<'_, Message> {
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    if state.camera.is_open() {
        return super::camera::view(state);
    }
    if state.page_editor.is_open() {
        return super::page_edit::view(state);
    }
    let body = responsive(move |available| {
        super::toolbar::workspace(state, available)
    });

    // Separate renderer layers keep page images below the modal and its scrim.
    iced::widget::stack![
        iced::widget::Space::new().width(Fill).height(Fill),
        column![heading(state), row![super::rail::view(state), body]]
            .width(Fill)
            .height(Fill),
    ]
    .extend(super::rail::overlay(state))
    .extend(super::confirmation::view(state))
    .extend(super::help::view(state))
    .width(Fill)
    .height(Fill)
    .clip(true)
    .into()
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
                && !state.show_shortcuts
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

#[cfg(test)]
mod tests {
    use super::accepts_text;
    use iced::keyboard;

    #[test]
    fn plain_enter_accepts_but_shift_enter_does_not() {
        let enter = keyboard::Key::Named(keyboard::key::Named::Enter);

        assert!(accepts_text(&enter, keyboard::Modifiers::empty()));
        assert!(!accepts_text(&enter, keyboard::Modifiers::SHIFT));
    }
}
