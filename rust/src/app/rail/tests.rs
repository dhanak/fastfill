use super::*;
use crate::app::PickPurpose;
use crate::app::controller::update;
use crate::picker::ImportedImage;
use fastfill_rust::geometry::Size as PageSize;
use iced::widget::canvas::Program;
use iced::widget::image::Handle;

mod animation;
mod appearance;
mod feedback;
mod layout;

// Tall portrait cards in three_pages() reach the 164px maximum pitch.
const SLOT_HEIGHT: f32 = 164.0;

const BOUNDS: Rectangle =
    Rectangle::new(Point::new(10.0, 50.0), Size::new(148.0, SLOT_HEIGHT * 2.0));

#[test]
fn mouse_thumbnail_drop_uses_the_release_position_without_scrolling() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let start = Point::new(30.0, BOUNDS.y + 140.0);
    let target = Point::new(30.0, BOUNDS.y + SLOT_HEIGHT + 140.0);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: start }),
        start,
    );
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
    assert_eq!(app.page_scroll, 0.0);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        target,
    );
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
    assert!(app.dragged_page.is_none());
}

#[test]
fn touch_scrollbar_and_thumbnail_drag_use_separate_actions() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let start = Point::new(BOUNDS.x + BOUNDS.width - 4.0, BOUNDS.y + 100.0);
    let moved = Point::new(start.x, start.y + 40.0);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(1),
            position: start,
        }),
        start,
    );
    event(
        &mut app,
        &mut input,
        &canvas::Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(1),
            position: moved,
        }),
        moved,
    );
    assert!((app.page_scroll - 60.0).abs() < 0.01);
    assert!(app.dragged_page.is_none());
    event(
        &mut app,
        &mut input,
        &canvas::Event::Touch(touch::Event::FingerLifted {
            id: touch::Finger(1),
            position: moved,
        }),
        moved,
    );

    app.page_scroll = 0.0;
    let grip = Point::new(30.0, BOUNDS.y + 140.0);
    let target = Point::new(30.0, grip.y + SLOT_HEIGHT);
    for (id, phase, point) in [
        (1, Phase::Press, grip),
        (2, Phase::Move, target),
        (1, Phase::Move, target),
        (1, Phase::Release, target),
    ] {
        let id = touch::Finger(id);
        let touch = match phase {
            Phase::Press => touch::Event::FingerPressed {
                id,
                position: point,
            },
            Phase::Move => touch::Event::FingerMoved {
                id,
                position: point,
            },
            _ => touch::Event::FingerLifted {
                id,
                position: point,
            },
        };
        event(&mut app, &mut input, &canvas::Event::Touch(touch), point);
    }
    assert_eq!(app.pages.names(), ["2.png", "1.png", "3.png"]);
}

#[test]
fn lost_touch_or_release_outside_the_rail_cancels_reorder() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let grip = Point::new(30.0, BOUNDS.y + 140.0);
    let target = Point::new(30.0, grip.y + SLOT_HEIGHT);
    for lost in [false, true] {
        event(
            &mut app,
            &mut input,
            &canvas::Event::Touch(touch::Event::FingerPressed {
                id: touch::Finger(1),
                position: grip,
            }),
            grip,
        );
        event(
            &mut app,
            &mut input,
            &canvas::Event::Touch(touch::Event::FingerMoved {
                id: touch::Finger(1),
                position: target,
            }),
            target,
        );
        let end = if lost {
            touch::Event::FingerLost {
                id: touch::Finger(1),
                position: target,
            }
        } else {
            touch::Event::FingerLifted {
                id: touch::Finger(1),
                position: Point::ORIGIN,
            }
        };
        event(&mut app, &mut input, &canvas::Event::Touch(end), target);
        assert!(app.dragged_page.is_none());
        assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
    }
}

#[test]
fn the_whole_thumbnail_can_start_a_mouse_or_touch_reorder() {
    for touch in [false, true] {
        let mut app = three_pages();
        app.page_scroll = 0.0;
        let mut input = RailState::default();
        let start = Point::new(30.0, BOUNDS.y + 30.0);
        let event_value = if touch {
            canvas::Event::Touch(touch::Event::FingerPressed {
                id: touch::Finger(1),
                position: start,
            })
        } else {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))
        };
        event(&mut app, &mut input, &event_value, start);
        assert!(app.dragged_page.is_some());
    }
}

#[test]
fn empty_space_below_the_last_page_is_an_end_of_list_drop() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let bounds = Rectangle {
        height: SLOT_HEIGHT * 4.0,
        ..BOUNDS
    };
    let below = Point::new(30.0, bounds.y + SLOT_HEIGHT * 3.5);
    let rail = Rail { app: &app };
    assert_eq!(rail.drop_index_at(below, bounds), Some(2));
    assert_eq!(rail.drop_index_at(Point::ORIGIN, bounds), None);
    let mut input = RailState::default();
    let start = Point::new(30.0, bounds.y + 30.0);
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
        bounds,
    );
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: below }),
        below,
        bounds,
    );
    assert_eq!(app.dragged_page.unwrap().target, 2);
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        below,
        bounds,
    );
    assert_eq!(app.pages.names(), ["2.png", "3.png", "1.png"]);
}

#[test]
fn thumbnail_follows_pointer_above_ui_and_cancels_outside_the_rail() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let mut input = RailState::default();
    let start = Point::new(30.0, BOUNDS.y + 30.0);
    let outside = Point::new(1_100.0, 200.0);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: outside }),
        outside,
    );
    assert!(app.dragged_page.unwrap().moving);
    assert_eq!(
        app.dragged_page.unwrap().position,
        Point::new(1_081.0, 176.0)
    );
    for theme in [Theme::Dark, Theme::Light] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let pixels = crate::app::tests::render_themed_element(
                crate::app::view::view(&app),
                Size::new(1_200, 800),
                scale,
                dpi,
                &theme,
                mouse::Cursor::Unavailable,
            );
            let scale = usize::try_from(scale).unwrap();
            let offset = (220 * scale * 1_200 * scale + 1_150 * scale) * 4;
            assert!(
                pixels[offset..offset + 3].iter().all(|v| *v > 180),
                "floating paper above sidebar: {:?}",
                &pixels[offset..offset + 4]
            );
        }
    }
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
    assert_eq!(app.page_scroll, 0.0);
    event(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        outside,
    );
    assert!(app.dragged_page.is_none());
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
}

#[test]
fn reorder_preview_moves_cards_without_changing_the_document() {
    let mut app = three_pages();
    app.page_scroll = 0.0;
    let bounds = Rectangle {
        height: SLOT_HEIGHT * 3.0,
        ..BOUNDS
    };
    let mut input = RailState::default();
    let start = Point::new(30.0, bounds.y + 30.0);
    let target = Point::new(30.0, bounds.y + SLOT_HEIGHT * 2.0 + 30.0);
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
        bounds,
    );
    assert_eq!((Rail { app: &app }).layout(bounds).visible_slot(0), Some(0));
    assert!(overlay(&app).is_none());
    event_in_bounds(
        &mut app,
        &mut input,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: target }),
        target,
        bounds,
    );
    let rail = Rail { app: &app };
    assert_eq!(rail.layout(bounds).visible_slot(0), None);
    assert_eq!(rail.layout(bounds).visible_slot(1), Some(0));
    assert_eq!(rail.layout(bounds).visible_slot(2), Some(1));
    assert_eq!(app.pages.names(), ["1.png", "2.png", "3.png"]);
    let _ = update(&mut app, Message::CancelInteraction);
    assert_eq!((Rail { app: &app }).layout(bounds).visible_slot(0), Some(0));
}

fn three_pages() -> FastFill {
    // Tall portraits reach the height cap used by the original input checks.
    pages_with_sizes(&[(300, 450); 3])
}

fn pages_with_sizes(sizes: &[(u16, u16)]) -> FastFill {
    let mut app = FastFill::default();
    for (index, &(width, height)) in sizes.iter().enumerate() {
        let _ = update(&mut app, Message::AddPage);
        let _ = update(
            &mut app,
            Message::ImagePicked(
                PickPurpose::Add,
                Ok(Some(ImportedImage {
                    // Avoid software-renderer rounding of enlarged 1px images.
                    handle: Handle::from_rgba(
                        u32::from(width),
                        u32::from(height),
                        vec![255; usize::from(width) * usize::from(height) * 4],
                    ),
                    display: None,
                    name: format!("{}.png", index + 1),
                    size: PageSize::new(f32::from(width), f32::from(height)),
                    info: fastfill_rust::image_input::ImageInfo {
                        width: u32::from(width),
                        height: u32::from(height),
                        format:
                            fastfill_rust::image_input::ImageFormatKind::Png,
                    },
                    directory: None,
                })),
            ),
        );
    }
    app
}

fn event(
    app: &mut FastFill,
    input: &mut RailState,
    event: &canvas::Event,
    cursor: Point,
) {
    event_in_bounds(app, input, event, cursor, BOUNDS);
}

fn event_in_bounds(
    app: &mut FastFill,
    input: &mut RailState,
    event: &canvas::Event,
    cursor: Point,
    bounds: Rectangle,
) {
    if let Some(action) = (Rail { app }).update(
        input,
        event,
        bounds,
        mouse::Cursor::Available(cursor),
    ) {
        let (message, _, _) = action.into_inner();
        if let Some(message) = message {
            let _ = update(app, message);
        }
    }
}
