use super::*;
use crate::app::tests::paste_text;
use crate::app::{PagePoint, Pointer, Tool, update};

fn send(state: &mut FastFill, event: Event) {
    let _ = update(state, Message::FontSize(event));
}

fn with_text() -> FastFill {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 200.0), 1.0),
    );
    paste_text(&mut state, "Font sample");
    let _ = update(&mut state, Message::AcceptText);
    state
}

#[test]
fn font_size_wheel_steps_and_stops_at_limits() {
    let up = mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };
    let down = mouse::ScrollDelta::Pixels { x: 0.0, y: -1.0 };
    assert_eq!(text_size_after_scroll(24.0, up), 25.0);
    assert_eq!(text_size_after_scroll(24.0, down), 23.0);
    assert_eq!(text_size_after_scroll(22.0, up), 23.0);
    assert_eq!(text_size_after_scroll(144.0, up), 144.0);
    assert_eq!(text_size_after_scroll(6.0, down), 6.0);
    assert_eq!(text_size_after_scroll(14.5, up), 15.5);
    assert_eq!(text_size_after_scroll(14.5, down), 13.5);
    assert_eq!(
        text_size_after_scroll(
            14.5,
            mouse::ScrollDelta::Lines { x: 1.0, y: 0.0 }
        ),
        14.5
    );
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

#[test]
fn numeric_input_accepts_the_full_range_and_rejects_invalid_values() {
    for (input, expected) in [
        ("6", 6.0),
        ("22", 22.0),
        ("144", 144.0),
        (" 22.5 ", 22.5),
        ("22,5", 22.5),
    ] {
        assert_eq!(parse_size(input), Some(expected));
    }
    for input in ["", "oops", "NaN", "inf", "-1", "5.9", "144.1"] {
        assert_eq!(parse_size(input), None, "{input}");
    }
}

#[test]
fn numeric_draft_commits_once_and_undo_restores_original_format() {
    let mut state = with_text();
    let original = state.annotations().texts().next().unwrap().clone();
    for value in ["", "2", "22"] {
        send(&mut state, Event::Input(value.into()));
        assert_eq!(state.annotations().texts().next().unwrap(), &original);
    }
    send(&mut state, Event::Apply);
    assert_eq!(
        state.annotations().texts().next().unwrap().format.font_size,
        22.0
    );
    assert!(state.font_size_input.is_none());
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
}

#[test]
fn invalid_input_changes_nothing_and_escape_keeps_the_selection() {
    let mut state = with_text();
    let original = state.annotations().texts().next().unwrap().clone();
    send(&mut state, Event::Input("200".into()));
    send(&mut state, Event::Apply);
    assert!(state.status_is_error);
    assert_eq!(state.font_size_input.as_deref(), Some("200"));
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(state.font_size_input.is_none());
    assert!(!state.status_is_error);
    assert!(state.annotations().has_selection());
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
}

#[test]
fn accepting_number_does_not_accept_inline_text_and_escape_reverts_it() {
    let mut state = with_text();
    let original = state.annotations().texts().next().unwrap().clone();
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 200.0), 1.0),
    );
    send(&mut state, Event::Input("22".into()));
    send(&mut state, Event::Apply);
    assert!(state.annotations().has_text_edit());
    assert_eq!(
        state.annotations().text_preview().unwrap().format.font_size,
        22.0
    );
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
    send(&mut state, Event::Input("30".into()));
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(state.annotations().has_text_edit());
    assert_eq!(
        state.annotations().text_preview().unwrap().format.font_size,
        22.0
    );
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.annotations().has_text_edit());
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
}

#[test]
fn click_away_accepts_valid_size_but_discards_an_invalid_size() {
    for input in ["22", "200"] {
        let mut state = with_text();
        send(&mut state, Event::Input(input.into()));
        let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
        let size = state.annotations().texts().next().unwrap().format.font_size;
        assert_eq!(size, if input == "22" { 22.0 } else { 24.0 });
        assert!(state.font_size_input.is_none());
    }
}

#[test]
fn stepping_uses_valid_numeric_draft_and_never_rounds_untouched_sizes() {
    let mut state = with_text();
    let size = 14.666_667_f32;
    let _ = update(&mut state, Message::SetTextSize(size));
    send(&mut state, Event::Apply);
    let shown = state.annotations().current_text_format().unwrap();
    let _ = view(&state, shown);
    assert_eq!(shown.font_size.to_bits(), size.to_bits());
    assert_eq!(
        state
            .annotations()
            .texts()
            .next()
            .unwrap()
            .format
            .font_size
            .to_bits(),
        size.to_bits()
    );
    send(&mut state, Event::Input("22".into()));
    send(&mut state, Event::Step(1));
    assert_eq!(
        state.annotations().texts().next().unwrap().format.font_size,
        23.0
    );
    let _ = update(&mut state, Message::Undo);
    assert_eq!(
        state
            .annotations()
            .texts()
            .next()
            .unwrap()
            .format
            .font_size
            .to_bits(),
        size.to_bits()
    );
}

#[test]
fn undo_discards_only_the_pending_numeric_draft() {
    let mut state = with_text();
    let original = state.annotations().texts().next().unwrap().clone();
    send(&mut state, Event::Input("22".into()));
    let _ = update(&mut state, Message::Undo);
    assert!(state.font_size_input.is_none());
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
    send(
        &mut state,
        Event::Scroll(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
    );
    assert_eq!(
        state.annotations().texts().next().unwrap().format.font_size,
        25.0
    );
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
}

#[test]
fn group_size_change_is_atomic_and_preserves_shapes_and_other_text_style() {
    let mut state = crate::app::tests::annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    for (point, size) in [
        (PagePoint::new(100.0, 200.0), 24.0),
        (PagePoint::new(300.0, 400.0), 32.0),
    ] {
        state.annotations_mut().clear_selection();
        let _ = update(&mut state, Message::SetTextSize(size));
        let _ = update(
            &mut state,
            Message::DrawPressed(Pointer::Mouse, point, 1.0),
        );
        paste_text(&mut state, "Sample");
        let _ = update(&mut state, Message::AcceptText);
    }
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(1.0, 1.0), 1.0),
    );
    let _ = update(
        &mut state,
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(600.0, 700.0),
            false,
        ),
    );
    assert_eq!(state.annotations().selected_count(), 3);
    let before = state.annotations().texts().cloned().collect::<Vec<_>>();
    let shape = state.annotations().shapes().next().unwrap().clone();
    send(&mut state, Event::Input("22".into()));
    send(&mut state, Event::Apply);
    for (text, original) in state.annotations().texts().zip(&before) {
        assert_eq!(
            text.format,
            TextFormat {
                font_size: 22.0,
                ..original.format
            }
        );
        assert_eq!(text.id, original.id);
    }
    assert_eq!(state.annotations().shapes().next().unwrap(), &shape);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(
        state.annotations().texts().cloned().collect::<Vec<_>>(),
        before
    );
    assert_eq!(state.annotations().shapes().next().unwrap(), &shape);
}

#[test]
fn font_size_events_cannot_bypass_a_project_operation_guard() {
    let mut state = with_text();
    let original = state.annotations().texts().next().unwrap().clone();
    state.project.operation = crate::app::project::Operation::Opening(1);
    send(&mut state, Event::Step(1));
    send(&mut state, Event::Input("22".into()));
    send(&mut state, Event::Apply);
    assert!(state.font_size_input.is_none());
    assert_eq!(state.annotations().texts().next().unwrap(), &original);
}

#[test]
fn captured_delete_edits_the_field_but_escape_still_cancels_its_draft() {
    use iced::{event::Status, keyboard};
    let key = keyboard::Key::Named(keyboard::key::Named::Delete);
    assert!(
        controller::shortcut_message(
            key.clone(),
            keyboard::Modifiers::empty(),
            Status::Captured,
        )
        .is_none()
    );
    assert!(matches!(
        controller::shortcut_message(
            key,
            keyboard::Modifiers::empty(),
            Status::Ignored,
        ),
        Some(Message::DeleteSelected)
    ));
    assert!(matches!(
        controller::shortcut_message(
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::empty(),
            Status::Captured,
        ),
        Some(Message::CancelInteraction)
    ));
}

#[test]
fn stacked_text_controls_fit_without_horizontal_scroll_or_small_targets() {
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};

    let renderer =
        iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    let state = with_text();
    let format = state.annotations().current_text_format().unwrap();
    let mut element = panel(&state, format);
    let mut tree = widget::Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(iced::Size::ZERO, iced::Size::new(184.0, 200.0)),
    );
    assert_eq!(node.size(), iced::Size::new(184.0, 136.0));
    assert_eq!(node.children()[0].size(), iced::Size::new(184.0, 40.0));
    for child in node.children() {
        assert_eq!(child.bounds().height, CONTROL_HEIGHT);
        assert!(child.bounds().x + child.bounds().width <= 184.0);
    }
}

#[test]
fn spinner_buttons_and_entry_do_not_grow_with_annotation_font_size() {
    use crate::app::Renderer;
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};

    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    let mut state = with_text();
    for size in [6.0, 14.666_667, 22.0, 144.0] {
        let _ = update(&mut state, Message::SetTextSize(size));
        let format = state.annotations().current_text_format().unwrap();
        let mut element: iced::Element<'_, Message> =
            view(&state, format).into();
        let mut tree = widget::Tree::new(element.as_widget());
        let bounds = iced::Size::new(600.0, 56.0);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(bounds, bounds),
        );
        let spinner = &node.children()[1];
        for index in [0, 2] {
            let button = spinner.children()[index].bounds();
            assert_eq!(button.width, CONTROL_HEIGHT);
            assert_eq!(button.height, CONTROL_HEIGHT);
        }
        let entry = spinner.children()[1].bounds();
        assert_eq!(entry.width, 60.0);
        assert_eq!(entry.height, CONTROL_HEIGHT);
    }
}
