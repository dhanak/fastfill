use super::*;
use crate::app::tests::render_themed_element;

fn start(
    app: &mut FastFill,
    input: &mut RailState,
    index: usize,
    bounds: Rectangle,
) {
    let point = (Rail { app }).card_bounds(index, bounds).position()
        + Vector::new(19.0, 24.0);
    event_in_bounds(
        app,
        input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        point,
        bounds,
    );
}

#[test]
fn floating_page_number_matches_its_preview_slot_not_its_original_index() {
    let _ = crate::appearance::text_font(
        fastfill_rust::annotations::TextFormat::default(),
    );
    let bounds = Rectangle {
        height: SLOT_HEIGHT * 3.0,
        ..BOUNDS
    };
    let mut actual = three_pages();
    actual.page_scroll = 0.0;
    let mut input = RailState::default();
    start(&mut actual, &mut input, 0, bounds);
    let last = Point::new(30.0, bounds.y + SLOT_HEIGHT * 2.0 + 30.0);
    event_in_bounds(
        &mut actual,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: last }),
        last,
        bounds,
    );
    assert_eq!(actual.dragged_page.unwrap().target, 2);
    let outside = Point::new(300.0, 200.0);
    event_in_bounds(
        &mut actual,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: outside }),
        outside,
        bounds,
    );

    // Same floating card, but already page 3: only its label used to differ.
    let mut expected = three_pages();
    expected.page_scroll = 0.0;
    let mut input = RailState::default();
    start(&mut expected, &mut input, 2, bounds);
    event_in_bounds(
        &mut expected,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: outside }),
        outside,
        bounds,
    );
    let render = |app: &FastFill| {
        render_themed_element(
            overlay(app).unwrap(),
            Size::new(600, 400),
            1,
            1.0,
            &Theme::Dark,
            mouse::Cursor::Unavailable,
        )
    };
    let actual_pixels = render(&actual);
    let expected_pixels = render(&expected);
    let background =
        &actual_pixels[(230 * 600 + 298) * 4..(230 * 600 + 299) * 4];
    assert!(
        (183..310).any(|y| (290..306).any(|x| {
            &actual_pixels[(y * 600 + x) * 4..(y * 600 + x + 1) * 4]
                != background
        })),
        "floating page number must be visible"
    );
    assert!(
        actual_pixels == expected_pixels,
        "dragged page must show its insertion number"
    );
    assert_eq!(actual.pages.names(), ["1.png", "2.png", "3.png"]);
}

#[test]
fn landscape_cards_shrink_and_hit_targets_follow_mixed_heights() {
    let app = pages_with_sizes(&[(300, 450), (450, 300), (300, 300)]);
    let bounds = Rectangle {
        height: 600.0,
        ..BOUNDS
    };
    let rail = Rail { app: &app };
    let portrait = rail.card_bounds(0, bounds);
    let landscape = rail.card_bounds(1, bounds);
    let square = rail.card_bounds(2, bounds);
    assert!(landscape.height < square.height);
    assert!(square.height < portrait.height);
    assert!((landscape.y - portrait.y - portrait.height - 14.0).abs() < 0.01);
    assert!((square.y - landscape.y - landscape.height - 14.0).abs() < 0.01);
    for (index, card) in [portrait, landscape, square].iter().enumerate() {
        assert_eq!(rail.index_at(card.center(), bounds), Some(index));
    }
    if let Some(path) = std::env::var_os("FASTFILL_RAIL_SNAPSHOT") {
        let _ = crate::appearance::text_font(
            fastfill_rust::annotations::TextFormat::default(),
        );
        for theme in [Theme::Dark, Theme::Light] {
            let pixels = render_themed_element(
                canvas(Rail { app: &app }).width(148).height(600).into(),
                Size::new(148, 600),
                2,
                2.0,
                &theme,
                mouse::Cursor::Unavailable,
            );
            image::save_buffer(
                std::path::PathBuf::from(&path)
                    .join(format!("rail-mixed-{theme:?}.png")),
                &pixels,
                296,
                1_200,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}

#[test]
fn a_short_dragged_card_does_not_oscillate_around_a_tall_neighbor() {
    let mut app = pages_with_sizes(&[(450, 300), (300, 450), (300, 300)]);
    app.page_scroll = 0.0;
    let bounds = Rectangle {
        height: 600.0,
        ..BOUNDS
    };
    let rail = Rail { app: &app };
    let source = rail.card_bounds(0, bounds);
    let neighbor = rail.card_bounds(1, bounds);
    let target =
        neighbor.center() + Vector::new(0.0, 24.0 - source.height / 2.0 + 1.0);
    let mut input = RailState::default();
    start(&mut app, &mut input, 0, bounds);
    for _ in 0..5 {
        event_in_bounds(
            &mut app,
            &mut input,
            &canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: target,
            }),
            target,
            bounds,
        );
        assert_eq!(app.dragged_page.unwrap().target, 1);
    }
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        target,
        bounds,
    );
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
}

#[test]
fn all_mixed_preview_orders_keep_gap_height_spacing_and_numbering() {
    let bounds = Rectangle {
        height: 600.0,
        ..BOUNDS
    };
    for source in 0..4 {
        let mut app = pages_with_sizes(&[
            (300, 450),
            (450, 300),
            (300, 300),
            (4000, 100),
        ]);
        app.page_scroll = 0.0;
        let original = (Rail { app: &app }).layout(bounds);
        assert_eq!(original.cards[3].height, crate::appearance::CONTROL_HEIGHT);
        start(&mut app, &mut RailState::default(), source, bounds);
        for target in 0..4 {
            let drag = app.dragged_page.as_mut().unwrap();
            drag.moving = true;
            drag.target = target;
            let layout = (Rail { app: &app }).layout(bounds);
            assert!((layout.total - original.total).abs() < 0.01);
            assert_eq!(
                layout.cards[source].height,
                original.cards[source].height
            );
            assert_eq!(layout.order[target], source);
            let mut numbers = vec![target + 1];
            numbers.extend((0..4).filter_map(|index| {
                layout.visible_slot(index).map(|slot| slot + 1)
            }));
            numbers.sort_unstable();
            assert_eq!(numbers, [1, 2, 3, 4]);
            for pair in layout.order.windows(2) {
                let upper = layout.cards[pair[0]];
                let lower = layout.cards[pair[1]];
                assert!((lower.y - upper.y - upper.height - 14.0).abs() < 0.01);
            }
        }
        assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png", "4.png"]);
    }
}

#[test]
fn queued_mixed_drag_resolves_current_geometry_before_commit() {
    let mut app = pages_with_sizes(&[(450, 300), (300, 450), (300, 300)]);
    app.page_scroll = 0.0;
    let bounds = Rectangle {
        height: 600.0,
        ..BOUNDS
    };
    let rail = Rail { app: &app };
    let source = rail.card_bounds(0, bounds);
    let start = source.position() + Vector::new(19.0, 24.0);
    let target = rail.card_bounds(1, bounds).center()
        + Vector::new(0.0, 24.0 - source.height / 2.0 + 1.0);
    let mut input = RailState::default();
    let mut messages = Vec::new();
    for (event, point) in [
        (
            canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            start,
        ),
        (
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: target,
            }),
            target,
        ),
        (
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: target,
            }),
            target,
        ),
        (
            canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            target,
        ),
    ] {
        if let Some(action) = (Rail { app: &app }).update(
            &mut input,
            &event,
            bounds,
            mouse::Cursor::Available(point),
        ) && let Some(message) = action.into_inner().0
        {
            messages.push(message);
        }
    }
    assert_eq!(messages.len(), 4);
    for message in messages {
        let _ = update(&mut app, message);
    }
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
    assert!(app.dragged_page.is_none());
}

#[test]
fn mixed_height_motion_lands_at_the_same_prefix_offsets_as_the_gap() {
    use iced::time::{Duration, Instant};
    let mut app = pages_with_sizes(&[(300, 450), (450, 300), (300, 300)]);
    app.page_scroll = 0.0;
    let bounds = Rectangle {
        height: 600.0,
        ..BOUNDS
    };
    let time = Instant::now();
    let duration = Duration::from_millis(180);
    let id = app.pages.iter().nth(2).unwrap().metadata.id;
    let tick = |app: &FastFill, time| {
        app.rail_motion
            .borrow_mut()
            .sync(app, bounds, time, duration)
    };
    tick(&app, time);
    let rail = Rail { app: &app };
    let origin = rail.layout(bounds).top(2);
    let source = rail.card_bounds(0, bounds);
    let target = rail.card_bounds(2, bounds).center()
        + Vector::new(0.0, 24.0 - source.height / 2.0 + 1.0);
    let mut input = RailState::default();
    start(&mut app, &mut input, 0, bounds);
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: target }),
        target,
        bounds,
    );
    assert_eq!(app.dragged_page.unwrap().target, 2);
    let destination = (Rail { app: &app }).layout(bounds).top(2);
    tick(&app, time);
    tick(&app, time + duration / 2);
    let midway = app.rail_motion.borrow().card_y(id, -1.0);
    assert!(midway > destination && midway < origin);
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        target,
        bounds,
    );
    tick(&app, time + duration * 2);
    tick(&app, time + duration * 3);
    assert!(
        (app.rail_motion.borrow().card_y(id, -1.0) - destination).abs() < 0.01
    );
    assert!(app.rail_motion.borrow().ghost.is_none());
    assert_eq!(app.pages.names(), ["2.png", "3.png", "1.png"]);
}

#[test]
fn mixed_height_scroll_limits_and_page_targets_use_measured_heights() {
    let app = pages_with_sizes(&[(300, 450), (450, 300), (300, 300)]);
    let bounds = Rectangle {
        height: 200.0,
        ..BOUNDS
    };
    let rail = Rail { app: &app };
    let layout = rail.layout(bounds);
    assert!(layout.total < SLOT_HEIGHT * 3.0);
    assert!(
        (rail.max_offset(bounds) - (layout.total - bounds.height)).abs() < 0.01
    );
    for index in 0..3 {
        assert!((page_top(&app, index) - layout.top(index)).abs() < 0.01);
    }
    let expected = bounds.height * bounds.height / layout.total;
    assert!((rail.scroll_thumb_height(bounds) - expected).abs() < 0.01);
}
