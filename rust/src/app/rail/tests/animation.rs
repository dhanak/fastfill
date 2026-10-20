use super::*;
use iced::time::{Duration, Instant};

const DURATION: Duration = Duration::from_millis(180);

fn tick(app: &FastFill, time: Instant, duration: Duration) -> bool {
    app.rail_motion
        .borrow_mut()
        .sync(app, BOUNDS, time, duration)
}

fn drag(app: &mut FastFill, input: &mut RailState, to: Point) {
    let start = Point::new(30.0, BOUNDS.y + 30.0);
    event(
        app,
        input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    event(
        app,
        input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: to }),
        to,
    );
}

#[test]
fn animated_gap_retargets_from_the_visible_position_and_stops_redrawing() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let time = Instant::now();
    assert!(!tick(&app, time, DURATION));
    let id = app.pages.iter().nth(1).unwrap().metadata.id;
    let mut input = RailState::default();
    let target = Point::new(30.0, BOUNDS.y + SLOT_HEIGHT + 30.0);
    drag(&mut app, &mut input, target);
    assert!(tick(&app, time, DURATION));
    assert_eq!(app.rail_motion.borrow().card_y(id, -1.0), SLOT_HEIGHT);
    assert!(tick(&app, time + DURATION / 2, DURATION));
    let midway = app.rail_motion.borrow().card_y(id, -1.0);
    assert!(midway > 0.0 && midway < SLOT_HEIGHT);
    let _ = update(&mut app, Message::CancelInteraction);
    assert!(tick(&app, time + DURATION / 2, DURATION));
    assert_eq!(app.rail_motion.borrow().card_y(id, -1.0), midway);
    assert!(!tick(&app, time + DURATION * 2, DURATION));
    assert_eq!(app.rail_motion.borrow().card_y(id, -1.0), SLOT_HEIGHT);
    assert!(app.rail_motion.borrow().ghost.is_none());
    assert!(!tick(&app, time + DURATION * 3, DURATION));
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
}

#[test]
fn valid_drop_commits_once_then_settles_to_the_visible_gap() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let time = Instant::now();
    let mut input = RailState::default();
    tick(&app, time, DURATION);
    let target = Point::new(30.0, BOUNDS.y + SLOT_HEIGHT + 90.0);
    drag(&mut app, &mut input, target);
    tick(&app, time, DURATION);
    let position = app.rail_motion.borrow().ghost.as_ref().unwrap().position;
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        target,
    );
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
    assert_eq!(app.page_scroll, 0.0);
    assert!(tick(&app, time, DURATION));
    assert_eq!(
        app.rail_motion.borrow().ghost.as_ref().unwrap().position,
        position,
    );
    assert!(tick(&app, time + DURATION / 2, DURATION));
    let destination = (Rail { app: &app }).card_bounds(1, BOUNDS).position();
    let halfway = app.rail_motion.borrow().ghost.as_ref().unwrap().position;
    assert!(
        (halfway.y - destination.y).abs() < (position.y - destination.y).abs()
    );
    assert!(!tick(&app, time + DURATION, DURATION));
    assert!(app.rail_motion.borrow().ghost.is_none());
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
}

#[test]
fn reduced_motion_settles_immediately_but_keeps_the_held_thumbnail() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let time = Instant::now();
    let mut input = RailState::default();
    let target = Point::new(30.0, BOUNDS.y + SLOT_HEIGHT + 90.0);
    drag(&mut app, &mut input, target);
    assert!(!tick(&app, time, Duration::ZERO));
    assert!(app.rail_motion.borrow().ghost.is_some());
    let id = app.pages.iter().nth(1).unwrap().metadata.id;
    assert_eq!(app.rail_motion.borrow().card_y(id, -1.0), 0.0);
    // Re-enabling motion must not resurrect an already completed transition.
    assert!(!tick(&app, time + DURATION / 2, DURATION));
    assert_eq!(app.rail_motion.borrow().card_y(id, -1.0), 0.0);
    let _ = update(&mut app, Message::CancelInteraction);
    assert!(!tick(&app, time, Duration::ZERO));
    assert!(app.rail_motion.borrow().ghost.is_none());
    assert_eq!(app.rail_motion.borrow().card_y(id, -1.0), SLOT_HEIGHT);
}

#[test]
fn interrupted_settling_uses_the_visible_card_for_hit_testing_and_grabbing() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let time = Instant::now();
    let mut input = RailState::default();
    tick(&app, time, DURATION);
    let target = Point::new(30.0, BOUNDS.y + SLOT_HEIGHT + 90.0);
    drag(&mut app, &mut input, target);
    tick(&app, time, DURATION);
    tick(&app, time + DURATION / 2, DURATION);
    let _ = update(&mut app, Message::CancelInteraction);
    tick(&app, time + DURATION / 2, DURATION);
    let id = app.pages.iter().nth(1).unwrap().metadata.id;
    let y = app.rail_motion.borrow().card_y(id, -1.0);
    // Page 2 is still moving through page 1's original slot.
    let point = Point::new(30.0, BOUNDS.y + y + 30.0);
    assert_eq!((Rail { app: &app }).index_at(point, BOUNDS), Some(1));
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        point,
    );
    let origin = app.dragged_page.unwrap().origin;
    assert_eq!(origin.y, BOUNDS.y + y + 6.0);
    let moved = Point::new(point.x + 10.0, point.y);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: moved }),
        moved,
    );
    tick(&app, time + DURATION / 2, DURATION);
    assert_eq!(app.rail_motion.borrow().ghost.as_ref().unwrap().id, id);
    assert_eq!(app.dragged_page.unwrap().position.y, origin.y);
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
}

#[test]
fn scrolling_during_settling_retargets_without_a_position_jump() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let time = Instant::now();
    let mut input = RailState::default();
    let target = Point::new(500.0, 200.0);
    drag(&mut app, &mut input, target);
    tick(&app, time, DURATION);
    let _ = update(&mut app, Message::CancelInteraction);
    tick(&app, time, DURATION);
    tick(&app, time + DURATION / 2, DURATION);
    let midway = app.rail_motion.borrow().ghost.as_ref().unwrap().position;
    app.page_scroll = 72.0;
    tick(&app, time + DURATION / 2, DURATION);
    assert_eq!(
        app.rail_motion.borrow().ghost.as_ref().unwrap().position,
        midway,
    );
    assert!(!tick(&app, time + DURATION * 2, DURATION));
    assert!(app.rail_motion.borrow().ghost.is_none());
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
}

#[test]
fn resize_modal_or_project_replacement_clears_obsolete_animation() {
    for reason in 0..3 {
        let mut app = three_pages();
        app.page_scroll = 0.0;
        let time = Instant::now();
        let mut input = RailState::default();
        drag(&mut app, &mut input, Point::new(500.0, 200.0));
        tick(&app, time, DURATION);
        let _ = update(&mut app, Message::CancelInteraction);
        let mut bounds = BOUNDS;
        match reason {
            0 => bounds.height += 100.0,
            1 => app.confirm_delete_all = true,
            _ => app.pages = three_pages().pages,
        }
        assert!(
            !app.rail_motion
                .borrow_mut()
                .sync(&app, bounds, time, DURATION,)
        );
        assert!(app.rail_motion.borrow().ghost.is_none());
    }
}

#[test]
fn native_redraws_request_frames_only_while_moving_without_app_messages() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let time = Instant::now();
    let redraw = |app: &FastFill, input: &mut RailState, time| {
        (Rail { app }).update(
            input,
            &canvas::Event::Window(iced::window::Event::RedrawRequested(time)),
            BOUNDS,
            mouse::Cursor::Unavailable,
        )
    };
    assert!(redraw(&app, &mut input, time).is_none());
    drag(
        &mut app,
        &mut input,
        Point::new(30.0, BOUNDS.y + SLOT_HEIGHT + 90.0),
    );
    let (message, redraw_request, status) =
        redraw(&app, &mut input, time).unwrap().into_inner();
    assert!(message.is_none());
    assert_eq!(redraw_request, iced::window::RedrawRequest::NextFrame);
    assert_eq!(status, iced::event::Status::Ignored);
    assert!(redraw(&app, &mut input, time + DURATION).is_none());
    let _ = update(&mut app, Message::CancelInteraction);
    assert!(redraw(&app, &mut input, time + DURATION).is_some());
    assert!(redraw(&app, &mut input, time + DURATION * 2).is_none());
}

#[test]
fn canceled_thumbnail_renders_above_the_ui_without_a_duplicate_source() {
    for theme in [Theme::Dark, Theme::Light] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let mut app = three_pages();
            app.page_scroll = 0.0;
            app.pages.select(0);
            app.pages.current_mut().image.as_mut().unwrap().handle =
                Handle::from_rgba(
                    300,
                    400,
                    [240, 230, 220, 255].repeat(120_000),
                );
            let render = |app: &FastFill| {
                crate::app::tests::render_themed_element(
                    crate::app::view::view(app),
                    Size::new(1_200, 800),
                    scale,
                    dpi,
                    &theme,
                    mouse::Cursor::Unavailable,
                )
            };
            let baseline = render(&app);
            let pixel_scale = usize::try_from(scale).unwrap();
            let source = baseline
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
                .find(|(index, pixel)| {
                    index % (1_200 * pixel_scale) < 164 * pixel_scale
                        && **pixel == [240, 230, 220, 255]
                })
                .map(|(index, _)| index * 4)
                .expect("colored source paper in the rail");
            let bounds = app.rail_motion.borrow().bounds.unwrap();
            let start = Point::new(bounds.x + 20.0, bounds.y + 30.0);
            let outside = Point::new(1_100.0, 200.0);
            let mut input = RailState::default();
            for (event, point) in [
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
            ] {
                event_in_bounds(&mut app, &mut input, &event, point, bounds);
            }
            render(&app);
            let _ = update(&mut app, Message::CancelInteraction);
            assert!(app.dragged_page.is_none());
            assert!(overlay(&app).is_some());
            let pixels = render(&app);
            let scale = usize::try_from(scale).unwrap();
            let pixel = |x, y| {
                let offset = (y * scale * 1_200 * scale + x * scale) * 4;
                &pixels[offset..offset + 4]
            };
            assert!(pixel(1_150, 220)[..3].iter().all(|v| *v > 180));
            // The source position stays empty until the floating card lands.
            assert_ne!(&pixels[source..source + 4], [240, 230, 220, 255]);
            assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
        }
    }
}
