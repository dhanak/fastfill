use fastfill_rust::annotations::{
    AnnotationGeometry, Annotations, DragModifiers, Pointer, ShapeKind, Tool,
};
use fastfill_rust::geometry::{Point, Rect, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;

fn mark(tool: Tool) -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(tool);
    annotations
}

#[test]
fn click_places_each_mark_centered_with_a_small_default_size() {
    for (tool, kind) in [
        (Tool::Checkmark, ShapeKind::Checkmark),
        (Tool::Cross, ShapeKind::Cross),
    ] {
        let mut annotations = mark(tool);
        let center = Point::new(300.0, 400.0);
        assert!(annotations.begin(MOUSE, center, PAGE));
        let preview = annotations.preview().unwrap();
        assert_eq!(preview.kind(), Some(kind));
        assert_eq!(preview.bounds(), Rect::new(287.5, 387.5, 25.0, 25.0));
        assert!((preview.stroke_width - 2.0).abs() < f32::EPSILON);

        assert!(annotations.finish(MOUSE, center, false));
        assert_eq!(annotations.shapes(), [preview]);
    }
}

#[test]
fn dragging_outward_creates_a_centered_square_in_every_direction() {
    for end in [
        Point::new(340.0, 420.0),
        Point::new(260.0, 420.0),
        Point::new(340.0, 380.0),
        Point::new(260.0, 380.0),
    ] {
        let mut annotations = mark(Tool::Checkmark);
        annotations.begin(MOUSE, Point::new(300.0, 400.0), PAGE);
        annotations.finish(MOUSE, end, false);
        assert_eq!(
            annotations.shapes()[0].bounds(),
            Rect::new(260.0, 360.0, 80.0, 80.0),
        );
    }
}

#[test]
fn resizing_a_mark_stays_square_without_ctrl_and_keeps_its_stroke() {
    let mut annotations = mark(Tool::Cross);
    annotations.begin(MOUSE, Point::new(300.0, 400.0), PAGE);
    annotations.finish(MOUSE, Point::new(340.0, 420.0), false);
    let original = annotations.shapes()[0].clone();
    annotations.set_tool(Tool::Select);

    let corner = Point::new(340.0, 440.0);
    assert!(annotations.begin_interaction(
        MOUSE,
        corner,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    annotations.finish(MOUSE, Point::new(380.0, 460.0), false);
    let resized = annotations.shapes()[0].clone();
    assert_eq!(resized.bounds().width, resized.bounds().height);
    assert!(resized.bounds().width > original.bounds().width);
    assert_eq!(resized.stroke_width, original.stroke_width);
    assert_eq!(resized.kind(), Some(ShapeKind::Cross));
}

#[test]
fn mark_movement_and_deletion_participate_in_history() {
    let mut annotations = mark(Tool::Checkmark);
    annotations.begin(MOUSE, Point::new(300.0, 400.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 400.0), false);
    let original = annotations.shapes()[0].clone();
    annotations.set_tool(Tool::Select);

    annotations.begin_interaction(
        MOUSE,
        Point::new(300.0, 400.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    );
    annotations.finish(MOUSE, Point::new(350.0, 450.0), false);
    let moved = annotations.shapes()[0].clone();
    assert!(annotations.delete_selected());
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), [moved]);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), [original]);
}

#[test]
fn marks_use_bounded_geometry() {
    let mut annotations = mark(Tool::Checkmark);
    annotations.begin(MOUSE, Point::new(300.0, 400.0), PAGE);
    let AnnotationGeometry::Bounded { kind, .. } =
        annotations.preview().unwrap().geometry
    else {
        panic!("expected bounded mark");
    };
    assert_eq!(kind, ShapeKind::Checkmark);
}
