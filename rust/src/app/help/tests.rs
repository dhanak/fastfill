use super::*;
use crate::app::ImagePicker;
use crate::app::controller::{shortcut_message, update};
use crate::app::tests::{annotated_page, paste_text};
use fastfill_rust::annotations::{AnnotationColor, Pointer, TextFormat, Tool};
use fastfill_rust::geometry::Point as PagePoint;
use iced::advanced::renderer::Headless;
use iced::advanced::{Layout, Shell, layout, widget};
use iced::widget::{button, text_editor};
use iced::{Event, Point, Renderer, Theme, keyboard, mouse, touch};

#[test]
fn f1_opens_help_even_when_a_text_field_captures_the_key() {
    for status in [iced::event::Status::Ignored, iced::event::Status::Captured]
    {
        assert!(matches!(
            shortcut_message(
                keyboard::Key::Named(keyboard::key::Named::F1),
                keyboard::Modifiers::empty(),
                status,
            ),
            Some(Message::ShowShortcuts),
        ));
    }
}

#[test]
fn opening_help_preserves_pending_text_format_and_numeric_drafts() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(200.0, 250.0), 1.0),
    );
    paste_text(&mut state, "Still pending");
    let _ = update(&mut state, Message::ToggleTextBold);
    state.font_size_input = Some("22.5".into());
    let preview = state.annotations().text_preview().unwrap();
    let identity = state.pages.identity();
    assert!(view(&state).is_none());
    let _ = update(&mut state, Message::ShowShortcuts);
    assert!(state.show_shortcuts);
    assert!(view(&state).is_some());
    for message in [Message::HideShortcuts, Message::CancelInteraction] {
        let _ = update(&mut state, message);
        assert!(!state.show_shortcuts);
        assert_eq!(state.pages.identity(), identity);
        assert_eq!(state.annotations().text_preview().unwrap(), preview);
        assert_eq!(state.font_size_input.as_deref(), Some("22.5"));
        let _ = update(&mut state, Message::ShowShortcuts);
    }
    let _ = update(&mut state, Message::CancelInteraction);
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(state.annotations().has_text_edit());
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.annotations().has_text_edit());
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn help_rejects_queued_background_edits_but_keeps_selection_and_history() {
    let mut state = annotated_page();
    let identity = state.pages.identity();
    let selection = state.annotations().selected_count();
    let tool = state.annotations().tool();
    let navigation = state.navigation;
    let _ = update(&mut state, Message::ShowShortcuts);
    for message in [
        Message::Undo,
        Message::DeleteSelected,
        Message::AskDeleteAll,
        Message::SetTool(Tool::Oval),
        Message::SetColor(AnnotationColor::Red),
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(80.0, 80.0), 1.0),
        Message::Pan(PagePoint::new(12.0, 12.0)),
        Message::PageRail(crate::app::rail::Event::Scroll { offset: 100.0 }),
        Message::TextAction(text_editor::Action::Edit(
            text_editor::Edit::Insert('x'),
        )),
        Message::NewProject,
        Message::OpenProject,
        Message::SaveProject,
        Message::ModalBackdrop,
    ] {
        let _ = update(&mut state, message);
        assert!(state.show_shortcuts);
        assert_eq!(state.pages.identity(), identity);
        assert_eq!(state.annotations().selected_count(), selection);
        assert_eq!(state.annotations().tool(), tool);
        assert_eq!(state.navigation, navigation);
        assert_eq!(state.page_scroll, 0.0);
    }
    let _ = update(&mut state, Message::HideShortcuts);
    let _ = update(&mut state, Message::Undo);
    assert!(state.annotations().is_empty());
}

#[test]
fn help_never_opens_over_a_confirmation_file_operation_or_pointer_drag() {
    for kind in 0..5 {
        let mut state = annotated_page();
        match kind {
            0 => state.confirm_delete_all = true,
            1 => state.confirm_delete_page = true,
            2 => state.image_picker = ImagePicker::ConfirmReplace,
            3 => {
                state.project.operation =
                    crate::app::project::Operation::Opening(1);
            }
            _ => {
                let _ = update(
                    &mut state,
                    Message::DrawPressed(
                        Pointer::Mouse,
                        PagePoint::new(400.0, 400.0),
                        1.0,
                    ),
                );
                assert!(state.annotations().active_pointer().is_some());
            }
        }
        let _ = update(&mut state, Message::ShowShortcuts);
        assert!(!state.show_shortcuts, "busy case {kind}");
    }
}

#[test]
fn closing_the_window_from_help_keeps_the_unsaved_project_guard() {
    let mut state = annotated_page();
    let identity = state.pages.identity();
    let _ = update(&mut state, Message::ShowShortcuts);
    let _ = update(
        &mut state,
        Message::CloseRequested(iced::window::Id::unique()),
    );
    assert!(!state.show_shortcuts);
    assert!(matches!(
        state.project.operation,
        crate::app::project::Operation::Confirm(
            crate::app::project::LeaveAction::Close(_)
        ),
    ));
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn focus_resize_and_worker_completions_do_not_accept_or_cancel_text() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(200.0, 250.0), 1.0),
    );
    paste_text(&mut state, "Pending");
    let preview = state.annotations().text_preview().unwrap();
    let _ = update(&mut state, Message::ShowShortcuts);
    for message in [
        Message::FocusLost,
        Message::ViewportResized,
        Message::SmartSnap(crate::app::snap::Event::Ready(
            999,
            Err("obsolete".into()),
        )),
        Message::PageEdit(crate::app::page_edit::Event::Ready(
            999,
            Err("obsolete".into()),
        )),
    ] {
        if matches!(message, Message::SmartSnap(_) | Message::PageEdit(_)) {
            assert!(super::update(&mut state, &message).is_none());
        }
        let _ = update(&mut state, message);
        assert!(state.show_shortcuts);
        assert_eq!(state.annotations().text_preview().unwrap(), preview);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = update(
            &mut state,
            Message::Recovery(crate::app::recovery::Event::Failed(
                "diagnostic retained".into(),
            )),
        );
        assert!(crate::app::recovery::banner(&state).is_some());
        assert!(state.show_shortcuts);
    }
}

fn renderer() -> Renderer {
    let _ = crate::appearance::text_font(TextFormat::default());
    iced::futures::executor::block_on(<Renderer as Headless>::new(
        iced::Font::with_name("Fira Sans"),
        iced::Pixels(16.0),
        Some("tiny-skia"),
    ))
    .unwrap()
}

fn lay_out(
    element: &mut Element<'_, Message>,
    renderer: &Renderer,
    size: iced::Size,
) -> (widget::Tree, layout::Node) {
    let mut tree = widget::Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        renderer,
        &layout::Limits::new(size, size),
    );
    (tree, node)
}

fn dispatch(
    element: &mut Element<'_, Message>,
    tree: &mut widget::Tree,
    node: &layout::Node,
    renderer: &Renderer,
    event: &Event,
    position: Point,
) -> (Vec<Message>, bool) {
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    element.as_widget_mut().update(
        tree,
        event,
        Layout::new(node),
        mouse::Cursor::Available(position),
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut shell,
        &node.bounds(),
    );
    let captured = shell.is_event_captured();
    (messages, captured)
}

#[derive(Default)]
struct Probe {
    texts: Vec<(String, iced::Rectangle)>,
    scrolls: Vec<iced::Vector>,
}

impl widget::Operation for Probe {
    fn traverse(
        &mut self,
        operate: &mut dyn FnMut(&mut dyn widget::Operation),
    ) {
        operate(self);
    }

    fn text(
        &mut self,
        _: Option<&widget::Id>,
        bounds: iced::Rectangle,
        text: &str,
    ) {
        self.texts.push((text.into(), bounds));
    }

    fn scrollable(
        &mut self,
        _: Option<&widget::Id>,
        _: iced::Rectangle,
        _: iced::Rectangle,
        translation: iced::Vector,
        _: &mut dyn widget::operation::scrollable::Scrollable,
    ) {
        self.scrolls.push(translation);
    }
}

fn probe(
    element: &mut Element<'_, Message>,
    tree: &mut widget::Tree,
    node: &layout::Node,
    renderer: &Renderer,
) -> Probe {
    let mut probe = Probe::default();
    element.as_widget_mut().operate(
        tree,
        Layout::new(node),
        renderer,
        &mut probe,
    );
    probe
}

#[test]
fn shortcuts_button_precedes_undo_and_routes_to_the_help_overlay() {
    let mut state = annotated_page();
    let renderer = renderer();
    let mut element =
        crate::app::toolbar::workspace(&state, iced::Size::new(1_036.0, 700.0));
    let (mut tree, node) =
        lay_out(&mut element, &renderer, iced::Size::new(1_036.0, 700.0));
    let texts = probe(&mut element, &mut tree, &node, &renderer).texts;
    let bounds = |label| texts.iter().find(|t| t.0 == label).unwrap().1;
    assert!(bounds("Shortcuts").x < bounds("Undo").x);
    let position = bounds("Shortcuts").center();
    let _ = dispatch(
        &mut element,
        &mut tree,
        &node,
        &renderer,
        &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        position,
    );
    let (messages, _) = dispatch(
        &mut element,
        &mut tree,
        &node,
        &renderer,
        &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        position,
    );
    assert!(matches!(messages.as_slice(), [Message::ShowShortcuts]));
    drop(element);
    for message in messages {
        let _ = update(&mut state, message);
    }
    assert!(view(&state).is_some());
}

#[test]
fn table_wraps_without_horizontal_overflow_and_scrolls_in_small_windows() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::ShowShortcuts);
    let renderer = renderer();
    for size in [
        iced::Size::new(320.0, 480.0),
        iced::Size::new(640.0, 360.0),
        iced::Size::new(1_200.0, 800.0),
    ] {
        let mut element = view(&state).unwrap();
        let (mut tree, node) = lay_out(&mut element, &renderer, size);
        let card = Layout::new(&node).children().next().unwrap();
        assert!(card.bounds().x >= 16.0 && card.bounds().y >= 16.0);
        assert!(card.bounds().x + card.bounds().width <= size.width - 16.0);
        assert!(card.bounds().y + card.bounds().height <= size.height - 16.0);
        let before = probe(&mut element, &mut tree, &node, &renderer);
        let command = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        assert!(
            !before
                .texts
                .iter()
                .any(|t| t.0.starts_with("Project/history:"))
        );
        assert!(before.texts.iter().any(|t| t.0 == format!("{command}+S")));
        assert!(before.texts.iter().any(|t| t.0 == "Ctrl+drag"));
        for (label, bounds) in &before.texts {
            assert!(bounds.x >= 16.0, "{label}: {bounds:?}");
            assert!(
                bounds.x + bounds.width <= size.width - 16.0 + 0.01,
                "{label}: {bounds:?} at {size:?}"
            );
        }
        for label in ["Context", "Shortcut", "Action"] {
            assert!(before.texts.iter().any(|t| t.0 == label));
        }
        assert_eq!(
            before.texts.iter().any(|t| t.0 == "Camera"),
            cfg!(all(feature = "native-camera", not(target_arch = "wasm32"))),
        );
        let (messages, captured) = dispatch(
            &mut element,
            &mut tree,
            &node,
            &renderer,
            &Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -100.0 },
            }),
            card.bounds().center(),
        );
        assert!(captured);
        assert!(messages.is_empty());
        let after = probe(&mut element, &mut tree, &node, &renderer);
        assert!(after.scrolls[0].y > before.scrolls[0].y);
        let close = after.texts.iter().find(|t| t.0 == "Close").unwrap().1;
        assert_eq!(
            close,
            before.texts.iter().find(|t| t.0 == "Close").unwrap().1
        );
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            let (messages, _) = dispatch(
                &mut element,
                &mut tree,
                &node,
                &renderer,
                &event,
                close.center(),
            );
            if matches!(event, Event::Mouse(mouse::Event::ButtonReleased(_))) {
                assert!(matches!(
                    messages.as_slice(),
                    [Message::HideShortcuts]
                ));
            }
        }
    }
}

#[test]
fn help_backdrop_blocks_mouse_touch_and_wheel_without_closing() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::ShowShortcuts);
    let mut element: Element<'_, Message> = iced::widget::stack![
        button("Underlying action")
            .width(Fill)
            .height(Fill)
            .on_press(Message::NewProject),
        view(&state).unwrap(),
    ]
    .into();
    let renderer = renderer();
    let (mut tree, node) =
        lay_out(&mut element, &renderer, iced::Size::new(1_200.0, 800.0));
    for event in [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        }),
        Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(1),
            position: Point::new(20.0, 20.0),
        }),
    ] {
        let (messages, captured) = dispatch(
            &mut element,
            &mut tree,
            &node,
            &renderer,
            &event,
            Point::new(20.0, 20.0),
        );
        assert!(captured, "{event:?}");
        assert!(messages.iter().all(|m| matches!(m, Message::ModalBackdrop)));
    }
}

#[test]
fn help_renders_above_the_page_in_both_themes_and_dpi_scales() {
    let mut state = crate::app::tests::rendered_page();
    let _ = update(&mut state, Message::ShowShortcuts);
    for theme in [Theme::Light, Theme::Dark] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let pixels = crate::app::tests::render_themed_element(
                crate::app::view::view(&state),
                iced::Size::new(1_200, 800),
                scale,
                dpi,
                &theme,
                mouse::Cursor::Unavailable,
            );
            let factor = usize::try_from(scale).unwrap();
            let offset = ((400 * factor) * 1_200 * factor + 160 * factor) * 4;
            let [r, g, b, _] =
                theme.extended_palette().background.base.color.into_rgba8();
            assert_eq!(&pixels[offset..offset + 3], [r, g, b]);
            if let Ok(directory) = std::env::var("FASTFILL_HELP_SNAPSHOT") {
                image::save_buffer(
                    std::path::Path::new(&directory)
                        .join(format!("help-{theme:?}-{scale}.png")),
                    &pixels,
                    1_200 * scale,
                    800 * scale,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }
}
