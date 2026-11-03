use super::*;
use crate::app::{Tool, update};
use iced::mouse;

fn state() -> FastFill {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Line));
    state
}

fn send(state: &mut FastFill, event: Event) {
    let _ = update(state, Message::StrokeWidth(event));
}

fn width(state: &FastFill) -> f32 {
    state.annotations().current_style().unwrap().stroke_width
}

#[test]
fn width_input_is_deferred_and_uses_the_shared_spinner_range() {
    let mut state = state();
    let original = width(&state);
    for input in ["", "7.5"] {
        send(&mut state, Event::Input(input.into()));
        assert_eq!(width(&state), original);
    }
    send(&mut state, Event::Apply);
    assert_eq!(width(&state), 7.5);
    assert!(state.stroke_width_input.is_none());
    for input in ["0", "21", "NaN", "oops"] {
        assert_eq!(parse_width(input), None);
    }
    assert_eq!(parse_width("1,5"), Some(1.5));
    assert_eq!(parse_width("20"), Some(20.0));
}

#[test]
fn invalid_width_can_be_cancelled_without_changing_the_style() {
    let mut state = state();
    let original = width(&state);
    send(&mut state, Event::Input("21".into()));
    send(&mut state, Event::Apply);
    assert!(state.status_is_error);
    assert_eq!(width(&state), original);
    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(width(&state), original);
    assert!(state.stroke_width_input.is_none());
    assert!(!state.status_is_error);
}

#[test]
fn width_steps_and_wheel_use_draft_then_clamp() {
    let mut state = state();
    send(&mut state, Event::Input("19".into()));
    send(&mut state, Event::Step(1));
    assert_eq!(width(&state), 20.0);
    send(
        &mut state,
        Event::Scroll(mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 }),
    );
    assert_eq!(width(&state), 20.0);
    send(
        &mut state,
        Event::Scroll(mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 }),
    );
    assert_eq!(width(&state), 19.0);
}

#[test]
fn stroke_width_events_respect_project_guard() {
    let mut state = state();
    let original = width(&state);
    state.project.operation = crate::app::project::Operation::Opening(1);
    send(&mut state, Event::Input("10".into()));
    send(&mut state, Event::Step(1));
    assert!(state.stroke_width_input.is_none());
    assert_eq!(width(&state), original);
}

#[test]
fn width_and_font_size_use_the_same_spinner_geometry() {
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};

    let renderer =
        iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    let state = state();
    for stacked in [false, true] {
        let mut element = view(&state, width(&state), stacked, true);
        let mut tree = widget::Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(
                iced::Size::ZERO,
                iced::Size::new(184.0, 40.0),
            ),
        );
        assert_eq!(node.size().height, 40.0);
        for index in [0, 2] {
            assert_eq!(node.children()[index].size().width, 40.0);
        }
        let entry_width = node.children()[1].size().width;
        if stacked {
            assert_eq!(node.size().width, 184.0);
            assert!(entry_width >= 40.0);
        } else {
            assert_eq!(entry_width, 60.0);
        }
    }
}
