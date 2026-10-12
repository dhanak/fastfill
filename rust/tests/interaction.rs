use fastfill_rust::geometry::Point;
use fastfill_rust::interaction::{TouchEvent, TouchNavigation};

const EPSILON: f32 = 0.001;

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < EPSILON);
}

#[test]
fn one_finger_move_pans_without_zooming() {
    let mut touch = TouchNavigation::default();
    touch.handle(TouchEvent::Pressed(1, Point::new(10.0, 20.0)), true);

    let delta = touch
        .handle(TouchEvent::Moved(1, Point::new(25.0, 45.0)), true)
        .expect("gesture delta");

    assert_eq!(delta.pan, Point::new(15.0, 25.0));
    close(delta.zoom_factor, 1.0);
}

#[test]
fn two_finger_move_pans_and_zooms_around_previous_center() {
    let mut touch = TouchNavigation::default();
    touch.handle(TouchEvent::Pressed(1, Point::new(0.0, 0.0)), true);
    touch.handle(TouchEvent::Pressed(2, Point::new(100.0, 0.0)), true);

    let delta = touch
        .handle(TouchEvent::Moved(2, Point::new(120.0, 0.0)), true)
        .expect("gesture delta");

    assert_eq!(delta.pan, Point::new(10.0, 0.0));
    assert_eq!(delta.zoom_anchor, Point::new(50.0, 0.0));
    close(delta.zoom_factor, 1.2);
}

#[test]
fn lifting_one_finger_does_not_make_the_remaining_finger_jump() {
    let mut touch = TouchNavigation::default();
    touch.handle(TouchEvent::Pressed(1, Point::new(10.0, 10.0)), true);
    touch.handle(TouchEvent::Pressed(2, Point::new(30.0, 10.0)), true);
    touch.handle(TouchEvent::Released(2, Point::new(30.0, 10.0)), true);

    let delta = touch
        .handle(TouchEvent::Moved(1, Point::new(15.0, 13.0)), true)
        .expect("gesture delta");

    assert_eq!(delta.pan, Point::new(5.0, 3.0));
    close(delta.zoom_factor, 1.0);
}

#[test]
fn drawing_mode_clears_and_ignores_navigation_gestures() {
    let mut touch = TouchNavigation::default();
    touch.handle(TouchEvent::Pressed(1, Point::new(0.0, 0.0)), true);

    assert!(
        touch
            .handle(TouchEvent::Moved(1, Point::new(20.0, 0.0)), false,)
            .is_none()
    );
    assert!(!touch.is_active());
    assert!(
        touch
            .handle(TouchEvent::Moved(1, Point::new(40.0, 0.0)), true,)
            .is_none()
    );
}
