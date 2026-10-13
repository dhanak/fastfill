use fastfill_rust::annotations::{Annotations, Pointer, ShapeKind, Tool};
use fastfill_rust::geometry::{Navigation, Point, Rect, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;

fn rectangle_tool() -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations
}

#[test]
fn bounded_shape_tools_commit_their_shape_kind() {
    for (tool, kind) in [
        (Tool::Rectangle, ShapeKind::Rectangle),
        (Tool::Oval, ShapeKind::Oval),
    ] {
        let mut annotations = Annotations::default();
        annotations.set_tool(tool);
        annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
        assert_eq!(annotations.preview().unwrap().kind, kind);
        annotations.finish(MOUSE, Point::new(200.0, 150.0), false);
        assert_eq!(annotations.shapes()[0].kind, kind);
    }
}

#[test]
fn preview_is_pending_until_release_in_each_drag_direction() {
    for end in [
        Point::new(120.0, 140.0),
        Point::new(80.0, 140.0),
        Point::new(120.0, 60.0),
        Point::new(80.0, 60.0),
    ] {
        let mut annotations = rectangle_tool();
        assert!(annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE));
        annotations.drag(MOUSE, end, false);

        let preview = annotations.preview().expect("pending rectangle");
        assert_eq!(
            preview.bounds,
            Rect::new(end.x.min(100.0), end.y.min(100.0), 20.0, 40.0),
        );
        assert_eq!(annotations.shapes(), []);
        assert!(annotations.finish(MOUSE, end, false));
        assert_eq!(annotations.shapes(), &[preview]);
        assert!(annotations.preview().is_none());
    }
}

#[test]
fn release_position_is_used_without_an_intermediate_move() {
    let mut annotations = rectangle_tool();
    annotations.begin(MOUSE, Point::new(10.0, 20.0), PAGE);

    assert!(annotations.finish(MOUSE, Point::new(50.0, 70.0), false));
    assert_eq!(
        annotations.shapes()[0].bounds,
        Rect::new(10.0, 20.0, 40.0, 50.0),
    );
}

#[test]
fn square_constraint_can_change_without_moving_the_pointer() {
    let mut annotations = rectangle_tool();
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.drag(MOUSE, Point::new(70.0, 150.0), false);
    annotations.constrain_square(true);
    assert_eq!(
        annotations.preview().unwrap().bounds,
        Rect::new(50.0, 100.0, 50.0, 50.0),
    );

    annotations.constrain_square(false);
    assert_eq!(
        annotations.preview().unwrap().bounds,
        Rect::new(70.0, 100.0, 30.0, 50.0),
    );
    annotations.finish(MOUSE, Point::new(70.0, 150.0), true);
    assert_eq!(
        annotations.shapes()[0].bounds,
        Rect::new(50.0, 100.0, 50.0, 50.0),
    );
}

#[test]
fn other_fingers_and_mouse_cannot_take_over_a_touch_draft() {
    let mut annotations = rectangle_tool();
    let finger = Pointer::Touch(1);
    annotations.begin(finger, Point::new(10.0, 20.0), PAGE);

    assert!(!annotations.begin(
        Pointer::Touch(2),
        Point::new(30.0, 40.0),
        PAGE
    ));
    annotations.drag(MOUSE, Point::new(100.0, 100.0), false);
    assert!(!annotations.finish(MOUSE, Point::new(100.0, 100.0), false));
    assert_eq!(annotations.active_pointer(), Some(finger));
    assert!(annotations.finish(finger, Point::new(30.0, 40.0), false));
    assert_eq!(annotations.shapes().len(), 1);
}

#[test]
fn cancel_and_tool_switch_discard_only_the_draft() {
    let mut annotations = rectangle_tool();
    annotations.begin(MOUSE, Point::new(10.0, 20.0), PAGE);
    annotations.finish(MOUSE, Point::new(50.0, 70.0), false);
    let committed = annotations.shapes().to_vec();

    annotations.begin(MOUSE, Point::new(80.0, 90.0), PAGE);
    annotations.cancel();
    assert!(annotations.active_pointer().is_none());
    assert_eq!(annotations.shapes(), committed);

    annotations.begin(MOUSE, Point::new(80.0, 90.0), PAGE);
    annotations.set_tool(Tool::Navigate);
    assert!(annotations.active_pointer().is_none());
    assert_eq!(annotations.shapes(), committed);
    assert!(!annotations.begin(MOUSE, Point::new(10.0, 20.0), PAGE));
}

#[test]
fn partial_off_page_rectangle_keeps_its_unclipped_geometry() {
    let mut annotations = rectangle_tool();
    annotations.begin(MOUSE, Point::new(10.0, 20.0), PAGE);
    assert!(annotations.finish(MOUSE, Point::new(-30.0, -40.0), false));
    assert_eq!(
        annotations.shapes()[0].bounds,
        Rect::new(-30.0, -40.0, 40.0, 60.0),
    );
}

#[test]
fn invalid_outside_and_degenerate_input_cannot_commit() {
    let mut annotations = rectangle_tool();
    for start in [
        Point::new(-1.0, 20.0),
        Point::new(PAGE.width + 1.0, 20.0),
        Point::new(f32::NAN, 20.0),
    ] {
        assert!(!annotations.begin(MOUSE, start, PAGE));
    }
    assert!(!annotations.begin(
        MOUSE,
        Point::new(10.0, 20.0),
        Size::new(f32::INFINITY, 100.0),
    ));
    for end in [
        Point::new(10.0, 20.0),
        Point::new(10.0, 80.0),
        Point::new(f32::INFINITY, 80.0),
    ] {
        annotations.begin(MOUSE, Point::new(10.0, 20.0), PAGE);
        assert!(!annotations.finish(MOUSE, end, false));
        assert!(annotations.active_pointer().is_none());
    }
    annotations.begin(MOUSE, Point::new(0.0, 0.0), PAGE);
    assert!(!annotations.finish(MOUSE, Point::new(-30.0, -40.0), false));
    assert_eq!(annotations.shapes(), []);
}

#[test]
fn drawing_maps_to_the_same_page_coordinates_at_different_zooms() {
    let viewport = Rect::new(36.0, 36.0, 1_000.0, 600.0);
    let start = Point::new(120.0, 180.0);
    let end = Point::new(270.0, 330.0);
    let mut navigation = Navigation::default();

    for factor in [1.0, 2.0, 0.75] {
        navigation.zoom_at(factor, viewport.center(), PAGE, viewport);
        navigation.pan_by(Point::new(12.0, -16.0));
        let view = navigation.view(PAGE, viewport).unwrap();
        let mut annotations = rectangle_tool();
        annotations.begin(
            MOUSE,
            view.view_to_page(view.page_to_view(start)),
            PAGE,
        );
        annotations.finish(
            MOUSE,
            view.view_to_page(view.page_to_view(end)),
            false,
        );
        let bounds = annotations.shapes()[0].bounds;
        assert!((bounds.x - start.x).abs() < 0.001);
        assert!((bounds.y - start.y).abs() < 0.001);
        assert!((bounds.width - 150.0).abs() < 0.001);
        assert!((bounds.height - 150.0).abs() < 0.001);
    }
}
