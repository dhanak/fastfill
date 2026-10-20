use super::*;
use crate::app::controller::update;
use fastfill_rust::annotations::{Pointer, Tool};
use fastfill_rust::geometry::{Point as PagePoint, Rect};
use iced::advanced::renderer::Headless;
use iced::advanced::{Layout, Shell, layout, widget};
use iced::{Event, Point, Renderer, mouse, touch};

fn pending(kind: u8) -> FastFill {
    let mut state = crate::app::tests::rendered_page();
    for message in [
        Message::SetTool(Tool::Rectangle),
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(80.0, 80.0), 1.0),
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(160.0, 160.0),
            false,
        ),
    ] {
        let _ = update(&mut state, message);
    }
    match kind {
        0 => state.image_picker = ImagePicker::ConfirmReplace,
        1 => state.confirm_delete_page = true,
        2 => state.confirm_delete_all = true,
        3..=5 => {
            let leave = match kind {
                3 => LeaveAction::New,
                4 => LeaveAction::Open,
                _ => LeaveAction::Close(iced::window::Id::unique()),
            };
            state.project.operation = Operation::Confirm(leave);
        }
        _ => unreachable!("six confirmation workflows"),
    }
    state
}

fn renderer() -> Renderer {
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

#[test]
fn regular_editing_has_no_confirmation() {
    assert!(view(&FastFill::default()).is_none());
}

#[test]
fn confirmations_do_not_resize_or_move_the_editor() {
    fn collect(node: &layout::Node, bounds: &mut Vec<iced::Rectangle>) {
        bounds.push(node.bounds());
        for child in node.children() {
            collect(child, bounds);
        }
    }

    let renderer = renderer();
    for size in [
        iced::Size::new(1_200.0, 800.0),
        iced::Size::new(640.0, 600.0),
    ] {
        let snapshot = |state: &FastFill| {
            let mut element = crate::app::view::view(state);
            let (_, node) = lay_out(&mut element, &renderer, size);
            let mut bounds = Vec::new();
            // Compare the editor layer, not the confirmation above it.
            collect(&node.children()[1], &mut bounds);
            bounds
        };
        for kind in 0..6 {
            let mut state = pending(kind);
            let before = snapshot(&state);
            let _ = update(&mut state, Message::CancelInteraction);
            assert_eq!(snapshot(&state), before, "prompt {kind} at {size:?}");
        }
    }
}

#[test]
fn actions_fit_small_windows_and_route_to_existing_workflows() {
    let renderer = renderer();
    for size in [
        iced::Size::new(320.0, 480.0),
        iced::Size::new(1_200.0, 800.0),
    ] {
        for kind in 0..6 {
            let state = pending(kind);
            let mut element = view(&state).unwrap();
            let (mut tree, node) = lay_out(&mut element, &renderer, size);
            let card = Layout::new(&node).children().next().unwrap();
            let bounds = card.bounds();
            assert!(bounds.x >= 16.0 && bounds.y >= 16.0);
            assert!(bounds.x + bounds.width <= size.width - 16.0);
            assert!(bounds.y + bounds.height <= size.height - 16.0);
            assert!((bounds.center().x - size.width / 2.0).abs() < 0.01);
            assert!((bounds.center().y - size.height / 2.0).abs() < 0.01);
            let column = card.children().next().unwrap();
            let actions = column.children().nth(2).unwrap();
            let buttons =
                actions.children().map(|b| b.bounds()).collect::<Vec<_>>();
            let expected = match kind {
                0 => vec![Message::KeepPage, Message::ConfirmReplacePage],
                1 => vec![Message::KeepPage, Message::ConfirmDeletePage],
                2 => vec![Message::KeepAll, Message::ConfirmDeleteAll],
                _ => vec![
                    Message::SaveBeforeLeave,
                    Message::DiscardProject,
                    Message::KeepProject,
                ],
            };
            assert_eq!(buttons.len(), expected.len());
            for (bounds, expected) in buttons.iter().zip(expected) {
                assert_eq!(bounds.height, CONTROL_HEIGHT);
                assert!((bounds.width - buttons[0].width).abs() < 0.01);
                assert!(bounds.width >= CONTROL_HEIGHT);
                let (pressed, captured) = dispatch(
                    &mut element,
                    &mut tree,
                    &node,
                    &renderer,
                    &Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    )),
                    bounds.center(),
                );
                assert!(pressed.is_empty());
                assert!(captured);
                let (messages, _) = dispatch(
                    &mut element,
                    &mut tree,
                    &node,
                    &renderer,
                    &Event::Mouse(mouse::Event::ButtonReleased(
                        mouse::Button::Left,
                    )),
                    bounds.center(),
                );
                assert_eq!(messages.len(), 1);
                assert_eq!(
                    std::mem::discriminant(&messages[0]),
                    std::mem::discriminant(&expected),
                );
            }
        }
    }
}

#[test]
fn backdrop_blocks_mouse_touch_and_wheel_without_dismissing() {
    let state = pending(2);
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
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
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
        assert!(captured, "backdrop must absorb {event:?}");
        assert!(messages.iter().all(|m| matches!(m, Message::ModalBackdrop)));
    }
}

#[test]
fn pending_confirmations_reject_late_background_commands_and_escape_keeps_data()
{
    for kind in 0..6 {
        let mut state = pending(kind);
        let identity = state.pages.identity();
        let tool = state.annotations().tool();
        let navigation = state.navigation;
        for message in [
            Message::ModalBackdrop,
            Message::Undo,
            Message::DeleteSelected,
            Message::SetTool(Tool::Oval),
            Message::SetTextSize(22.0),
            Message::DrawPressed(
                Pointer::Mouse,
                PagePoint::new(90.0, 90.0),
                1.0,
            ),
            Message::Pan(PagePoint::new(12.0, 12.0)),
            Message::ScrollPages(100.0),
            Message::NewProject,
        ] {
            let _ = update(&mut state, message);
            assert!(view(&state).is_some());
            assert_eq!(state.pages.identity(), identity);
            assert_eq!(state.annotations().tool(), tool);
            assert_eq!(state.navigation, navigation);
            assert_eq!(state.page_scroll, 0.0);
        }
        let _ = update(&mut state, Message::CancelInteraction);
        assert!(view(&state).is_none());
        assert_eq!(state.pages.identity(), identity);
    }
    let mut state = pending(2);
    let count = state.annotations().len();
    let _ = update(&mut state, Message::ConfirmDeleteAll);
    assert!(state.annotations().is_empty());
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().len(), count);
}

#[test]
fn confirmation_and_scrim_render_above_a_zoomed_image_at_both_dpi_scales() {
    let mut state = pending(2);
    state.navigation.zoom_at(
        4.0,
        PagePoint::new(500.0, 300.0),
        crate::app::page_size(&state),
        Rect::new(36.0, 36.0, 1_036.0, 568.0),
    );
    state.navigation.pan_by(PagePoint::new(-300.0, -500.0));
    for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
        state.confirm_delete_all = false;
        let baseline = crate::app::tests::render_element(
            crate::app::view::view(&state),
            iced::Size::new(1_200, 800),
            scale,
            dpi,
        );
        state.confirm_delete_all = true;
        let pixels = crate::app::tests::render_element(
            crate::app::view::view(&state),
            iced::Size::new(1_200, 800),
            scale,
            dpi,
        );
        let scale = usize::try_from(scale).unwrap();
        let offset =
            |x: usize, y: usize| ((y * scale) * 1_200 * scale + x * scale) * 4;
        let card = offset(350, 400);
        assert_eq!(&pixels[card..card + 4], [255, 255, 255, 255]);
        let backdrop = offset(1_180, 780);
        assert!(
            pixels[backdrop..backdrop + 3]
                .iter()
                .zip(&baseline[backdrop..backdrop + 3])
                .all(|(dimmed, original)| dimmed < original),
            "scrim must darken the image/background at {dpi} DPI scale"
        );
    }
}
