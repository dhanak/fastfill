use super::*;
use crate::appearance::accent;

fn pointer(
    app: &mut FastFill,
    input: &mut RailState,
    owner: Pointer,
    phase: Phase,
    point: Point,
) {
    if let Some(action) =
        (Rail { app }).pointer_input(input, owner, phase, Some(point), BOUNDS)
        && let Some(message) = action.into_inner().0
    {
        let _ = update(app, message);
    }
}

#[test]
fn moving_source_becomes_a_full_card_gap_at_both_dpi_scales() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    pointer(
        &mut app,
        &mut input,
        Pointer::Mouse,
        Phase::Press,
        Point::new(30.0, BOUNDS.y + 30.0),
    );
    pointer(
        &mut app,
        &mut input,
        Pointer::Mouse,
        Phase::Move,
        Point::new(500.0, 200.0),
    );
    for theme in [Theme::Dark, Theme::Light] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let pixels = crate::app::tests::render_themed_element(
                canvas(Rail { app: &app }).width(148).height(328).into(),
                Size::new(148, 328),
                scale,
                dpi,
                &theme,
                mouse::Cursor::Unavailable,
            );
            let scale = usize::try_from(scale).unwrap();
            let pixel = |x, y| {
                let offset = (y * scale * 148 * scale + x * scale) * 4;
                &pixels[offset..offset + 4]
            };
            // Source paper disappears; the next paper stays visible below it.
            assert_ne!(pixel(60, 30), [255, 255, 255, 255]);
            assert_eq!(pixel(60, 200), [255, 255, 255, 255]);
            let [r, g, b, _] = accent().into_rgba8();
            assert_eq!(pixel(1, 60), [r, g, b, 255]);
        }
    }
}

#[test]
fn grab_offset_survives_scrolling_and_ignores_other_pointers() {
    for owner in [Pointer::Mouse, Pointer::Touch(1)] {
        let mut app = three_pages();
        app.page_scroll = 45.0;
        let origin = (Rail { app: &app }).card_bounds(1, BOUNDS);
        let start = Point::new(origin.x + 50.0, origin.y + 60.0);
        let moved = Point::new(500.0, 200.0);
        let mut input = RailState::default();
        pointer(&mut app, &mut input, owner, Phase::Press, start);
        assert_eq!(app.dragged_page.unwrap().origin, origin);
        pointer(&mut app, &mut input, owner, Phase::Move, moved);
        let expected = moved - (start - origin.position());
        assert_eq!(app.dragged_page.unwrap().position, expected);
        for phase in [Phase::Press, Phase::Move, Phase::Release] {
            pointer(
                &mut app,
                &mut input,
                Pointer::Touch(2),
                phase,
                Point::new(30.0, 100.0),
            );
            assert_eq!(app.dragged_page.unwrap().position, expected);
        }
        pointer(&mut app, &mut input, owner, Phase::Release, moved);
        assert!(app.dragged_page.is_none());
        assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
    }
}

#[test]
fn click_jitter_does_not_reorder_and_escape_allows_an_immediate_new_grab() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let edge = Point::new(30.0, BOUNDS.y + SLOT_HEIGHT - 1.0);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Press, edge);
    let jitter = Point::new(edge.x, edge.y + 2.0);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Move, jitter);
    assert!(overlay(&app).is_none());
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Release, jitter);
    assert_eq!(app.pages.active_index(), 0);
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);

    let start = Point::new(30.0, BOUNDS.y + 30.0);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Press, start);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Move, jitter);
    let _ = update(&mut app, Message::CancelInteraction);
    assert!(overlay(&app).is_none());
    pointer(&mut app, &mut input, Pointer::Touch(3), Phase::Press, start);
    assert!(app.dragged_page.is_some());
    assert!(matches!(input.pointer, Some((Pointer::Touch(3), _, true))));
}

#[test]
fn interruption_and_late_release_never_change_the_document() {
    for interruption in [
        Message::CancelInteraction,
        Message::FocusLost,
        Message::ViewportResized,
    ] {
        let mut app = three_pages();
        app.page_scroll = 0.0;
        let mut input = RailState::default();
        let start = Point::new(30.0, BOUNDS.y + 30.0);
        let target = Point::new(30.0, start.y + SLOT_HEIGHT);
        pointer(&mut app, &mut input, Pointer::Mouse, Phase::Press, start);
        pointer(&mut app, &mut input, Pointer::Mouse, Phase::Move, target);
        assert!(overlay(&app).is_some());
        let identity = app.pages.identity();
        let _ = update(&mut app, interruption);
        assert!(app.dragged_page.is_none());
        assert!(overlay(&app).is_none());
        pointer(&mut app, &mut input, Pointer::Mouse, Phase::Release, target);
        assert_eq!(app.pages.identity(), identity);
    }
}

#[test]
fn wheel_moves_the_insertion_gap_not_the_ghost_and_scrollbar_drop_cancels() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let start = Point::new(30.0, BOUNDS.y + 30.0);
    let target = Point::new(30.0, BOUNDS.y + 2.0 * SLOT_HEIGHT - 20.0);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Press, start);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Move, target);
    let position = app.dragged_page.unwrap().position;
    assert_eq!(app.dragged_page.unwrap().target, 1);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: -2.0 },
        }),
        target,
    );
    assert_eq!(app.page_scroll, 72.0);
    assert_eq!(app.dragged_page.unwrap().target, 2);
    assert_eq!(app.dragged_page.unwrap().position, position);
    let track = Point::new(BOUNDS.x + BOUNDS.width - 4.0, target.y);
    pointer(&mut app, &mut input, Pointer::Mouse, Phase::Release, track);
    assert!(app.dragged_page.is_none());
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
}

#[test]
fn queued_native_events_keep_the_owner_until_the_start_is_acknowledged() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let start = Point::new(30.0, BOUNDS.y + 30.0);
    let target = Point::new(30.0, start.y + SLOT_HEIGHT);
    let mut messages = Vec::new();
    // Native backends may batch these before rebuilding the application view.
    for (owner, phase, position) in [
        (Pointer::Touch(1), Phase::Press, start),
        (Pointer::Touch(2), Phase::Press, start),
        (Pointer::Touch(2), Phase::Move, target),
        (Pointer::Touch(1), Phase::Move, target),
        (Pointer::Touch(1), Phase::Release, target),
    ] {
        if let Some(action) = (Rail { app: &app }).pointer_input(
            &mut input,
            owner,
            phase,
            Some(position),
            BOUNDS,
        ) && let Some(message) = action.into_inner().0
        {
            messages.push(message);
        }
    }
    assert_eq!(messages.len(), 3);
    for message in messages {
        let _ = update(&mut app, message);
    }
    assert!(app.dragged_page.is_none());
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
    let _ = update(
        &mut app,
        Message::PageRail(Event::Finish {
            pointer: Some(target),
        }),
    );
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
}

#[test]
fn native_overlay_passes_pointer_events_to_the_captured_rail() {
    use iced::advanced::renderer::Headless;
    use iced::advanced::{Layout, Shell, layout, widget};
    use iced::widget::{Space, container};
    use iced::{Element, Fill, Padding};

    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut tree = None;
    let start = Point::new(30.0, BOUNDS.y + 30.0);
    let outside = Point::new(500.0, 200.0);
    for (event, cursor) in [
        (
            canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            start,
        ),
        (
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: outside,
            }),
            outside,
        ),
        (
            canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            outside,
        ),
    ] {
        let mut element: Element<'_, Message> = iced::widget::stack![
            Space::new().width(Fill).height(Fill),
            container(canvas(Rail { app: &app }).width(Fill).height(Fill))
                .padding(Padding {
                    left: BOUNDS.x,
                    top: BOUNDS.y,
                    ..Padding::ZERO
                })
                .width(BOUNDS.x + BOUNDS.width)
                .height(BOUNDS.y + BOUNDS.height),
        ]
        .extend(overlay(&app))
        .width(Fill)
        .height(Fill)
        .into();
        let tree =
            tree.get_or_insert_with(|| widget::Tree::new(element.as_widget()));
        tree.diff(element.as_widget());
        let size = Size::new(1_200.0, 800.0);
        let node = element.as_widget_mut().layout(
            tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        element.as_widget_mut().update(
            tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Available(cursor),
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut shell,
            &node.bounds(),
        );
        assert!(shell.is_event_captured());
        drop(element);
        assert_eq!(messages.len(), 1);
        for message in messages {
            let _ = update(&mut app, message);
        }
    }
    assert!(app.dragged_page.is_none());
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
}
