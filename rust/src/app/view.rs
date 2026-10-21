use super::{FastFill, ImagePicker, Message, TEXT_EDITOR_ID, page_size};
use crate::appearance::{self, action_button, annotation_color, text_font};
use crate::icons::Glyph;
use crate::page_canvas::{CanvasLayer, PageCanvas, page_viewport};
use fastfill_rust::annotations::{
    AnnotationColor, TEXT_EDITOR_HORIZONTAL_PADDING,
    TEXT_EDITOR_VERTICAL_PADDING, TEXT_LINE_HEIGHT,
};
use fastfill_rust::geometry::Point as PagePoint;
use fastfill_rust::geometry::Rect as PageRect;
use iced::widget::{
    Column, Space, canvas, column, container, progress_bar, responsive, row,
    text, text_editor, tooltip,
};
use iced::{Background, Border, Color, Element, Fill, Padding, Size, keyboard};

pub(super) fn canvas_viewport(window: Size) -> PageRect {
    let workspace_width = (window.width - super::rail::WIDTH).max(1.0);
    let workspace_height = (window.height - appearance::HEADER_HEIGHT).max(1.0);
    let wide = workspace_width >= appearance::PANEL_MIN_WORKSPACE_WIDTH;
    let canvas = Size::new(
        workspace_width
            - if wide {
                appearance::TOOLS_PANEL_WIDTH
            } else {
                0.0
            },
        workspace_height
            - if wide {
                0.0
            } else {
                appearance::TOOLBAR_HEIGHT
            },
    );
    page_viewport(canvas)
}

fn heading(
    state: &FastFill,
    viewport: PageRect,
    width: f32,
) -> Element<'_, Message> {
    let compact = width < 1_020.0;
    let tiny = width < 560.0;
    let project_width = if width < 440.0 {
        96.0
    } else if width < 650.0 {
        126.0
    } else if compact {
        190.0
    } else {
        240.0
    };
    let enabled = super::project_view::enabled(state);
    let zoom = |factor| Message::ZoomAt {
        factor,
        anchor: viewport.center(),
        viewport,
    };
    let mut bar = row![super::project_view::menu(state, project_width)];
    if !tiny {
        bar = bar
            .push(super::toolbar::icon_button(
                Glyph::Undo,
                "Undo",
                false,
                (enabled && state.annotations().can_undo())
                    .then_some(Message::Undo),
            ))
            .push(super::toolbar::icon_button(
                Glyph::Redo,
                "Redo",
                false,
                (enabled && state.annotations().can_redo())
                    .then_some(Message::Redo),
            ));
    }
    bar = bar.push(status_slot(state, width));
    bar = bar
        .push(super::toolbar::icon_button(
            Glyph::ZoomOut,
            "Zoom out",
            false,
            enabled.then_some(zoom(0.8)),
        ))
        .push(super::toolbar::icon_button(
            Glyph::ZoomIn,
            "Zoom in",
            false,
            enabled.then_some(zoom(1.25)),
        ));
    if !compact {
        bar = bar
            .push(super::project_view::view_action(
                Glyph::ActualSize,
                "1:1",
                enabled.then_some(Message::OneToOne(viewport)),
            ))
            .push(super::project_view::view_action(
                Glyph::Fit,
                "Fit",
                enabled.then_some(Message::Fit),
            ));
    }
    bar = bar.push(super::project_view::save(state, tiny));
    if compact {
        bar = bar.push(super::toolbar::icon_button(
            Glyph::More,
            "More commands",
            state.open_menu == Some(super::menu::Kind::More),
            enabled.then_some(Message::ToggleMenu(super::menu::Kind::More)),
        ));
    } else {
        bar = bar
            .push(super::project_view::export(state))
            .push(super::toolbar::icon_button(
                Glyph::Search,
                if cfg!(target_os = "macos") {
                    "Commands (Cmd+.)"
                } else {
                    "Commands (Ctrl+.)"
                },
                false,
                super::palette::can_open(state)
                    .then_some(Message::Palette(super::palette::Event::Toggle)),
            ))
            .push(super::toolbar::icon_button(
                Glyph::Help,
                "Shortcuts",
                false,
                super::help::can_open(state).then_some(Message::ShowShortcuts),
            ));
    }
    container(bar.spacing(8).align_y(iced::Alignment::Center))
        .padding([8, 12])
        .height(appearance::HEADER_HEIGHT)
        .width(Fill)
        .style(container::bordered_box)
        .into()
}

fn status_slot(state: &FastFill, width: f32) -> Element<'static, Message> {
    if width < 400.0 {
        return Space::new().width(Fill).into();
    }
    let status = if state.status.is_empty() {
        if state.image().is_some() {
            "Ready".to_owned()
        } else {
            "Add an image, import a PDF, or draw on the blank page.".to_owned()
        }
    } else {
        state.status.clone()
    };
    let status_text: Element<'static, Message> =
        container(text(status.clone()).size(13))
            .width(Fill)
            .clip(true)
            .into();
    tooltip(
        status_text,
        container(text(status).size(13))
            .padding([6, 10])
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .delay(std::time::Duration::from_millis(500))
    .into()
}

fn notifications(state: &FastFill) -> Option<Element<'_, Message>> {
    let mut notices: Vec<Element<'_, Message>> = Vec::new();
    if state.status_is_error && !state.status.is_empty() {
        notices.push(
            container(text(format!("⚠ {}", state.status)).size(14))
                .style(container::danger)
                .padding([6, 10])
                .into(),
        );
    }
    if let Some(warning) = super::page_edit::warning(state) {
        notices.push(warning);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(banner) = super::recovery::banner(state) {
        notices.push(banner);
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
        notices.push(
            container(
                row![
                    progress_bar(0.0..=1.0, progress).girth(8).length(Fill),
                    action_button(label, message),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            )
            .padding(8)
            .style(container::bordered_box)
            .into(),
        );
    }
    if notices.is_empty() {
        return None;
    }
    Some(
        container(
            container(Column::with_children(notices).spacing(4))
                .width(Fill)
                .max_width(900),
        )
        .width(Fill)
        .height(Fill)
        .align_x(iced::Alignment::Center)
        .padding(Padding {
            top: appearance::HEADER_HEIGHT + 8.0,
            left: 16.0,
            right: 16.0,
            ..Padding::ZERO
        })
        .into(),
    )
}

pub(super) fn view(state: &FastFill) -> Element<'_, Message> {
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    if state.camera.is_open() {
        return super::camera::view(state);
    }
    if state.page_editor.is_open() {
        return super::page_edit::view(state);
    }
    let layout = responsive(move |available| {
        let viewport = canvas_viewport(available);
        column![
            heading(state, viewport, available.width),
            row![
                super::rail::view(state),
                responsive(move |available| {
                    super::toolbar::workspace(state, available)
                }),
            ]
            .width(Fill)
            .height(Fill),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    });

    // Separate renderer layers keep page images below the modal and its scrim.
    iced::widget::stack![
        iced::widget::Space::new().width(Fill).height(Fill),
        layout,
    ]
    .extend(notifications(state))
    .extend(super::rail::overlay(state))
    .extend(super::menu::overlay(state))
    .extend(super::palette::view(state))
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
                && !state.palette.open
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
