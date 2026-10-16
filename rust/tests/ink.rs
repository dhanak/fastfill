use fastfill_rust::annotations::{
    AnnotationColor, AnnotationGeometry, Annotations, DragModifiers, InkKind,
    Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const TOUCH: Pointer = Pointer::Touch(7);

fn ink_points(annotations: &Annotations) -> (InkKind, &[Point]) {
    match &annotations.shapes()[0].geometry {
        AnnotationGeometry::Ink { kind, points } => (*kind, points),
        AnnotationGeometry::Bounded { .. }
        | AnnotationGeometry::Linear { .. } => panic!("expected ink"),
    }
}

#[test]
fn pen_collects_distinct_samples_from_its_pointer() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Pen);
    assert!(annotations.begin(TOUCH, Point::new(100.0, 100.0), PAGE));
    annotations.drag(TOUCH, Point::new(120.0, 130.0), false);
    annotations.drag(TOUCH, Point::new(120.0, 130.0), false);
    annotations.drag(Pointer::Touch(8), Point::new(500.0, 500.0), false);
    assert!(annotations.finish(TOUCH, Point::new(160.0, 140.0), false));

    let (kind, points) = ink_points(&annotations);
    assert_eq!(kind, InkKind::Pen);
    assert_eq!(
        points,
        [
            Point::new(100.0, 100.0),
            Point::new(120.0, 130.0),
            Point::new(160.0, 140.0),
        ],
    );
}

#[test]
fn pen_and_highlighter_have_independent_defaults_and_fill_rules() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Pen);
    assert_eq!(annotations.current_style().unwrap().stroke_width, 3.0);
    assert!(annotations.fill_available());
    assert!(annotations.set_filled(true));

    annotations.set_tool(Tool::Highlighter);
    let highlighter = annotations.current_style().unwrap();
    assert_eq!(highlighter.color, AnnotationColor::Yellow);
    assert_eq!(highlighter.stroke_width, 12.0);
    assert!(!highlighter.filled);
    assert!(!annotations.fill_available());
    assert!(!annotations.set_filled(true));
}

#[test]
fn ink_moves_and_resizes_without_scaling_its_stroke() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Pen);
    annotations.begin(TOUCH, Point::new(100.0, 100.0), PAGE);
    annotations.drag(TOUCH, Point::new(150.0, 200.0), false);
    annotations.finish(TOUCH, Point::new(200.0, 100.0), false);
    let original_bounds = annotations.shapes()[0].bounds();
    let stroke = annotations.shapes()[0].stroke_width;

    annotations.set_tool(Tool::Select);
    assert!(annotations.begin_interaction(
        TOUCH,
        Point::new(125.0, 150.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(annotations.finish(TOUCH, Point::new(225.0, 250.0), false));
    let moved_bounds = annotations.shapes()[0].bounds();
    assert!((moved_bounds.x - original_bounds.x - 100.0).abs() < 0.001);
    assert!((moved_bounds.y - original_bounds.y - 100.0).abs() < 0.001);

    let bottom_right = Point::new(
        moved_bounds.x + moved_bounds.width,
        moved_bounds.y + moved_bounds.height,
    );
    assert!(annotations.begin_interaction(
        TOUCH,
        bottom_right,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(annotations.finish(
        TOUCH,
        Point::new(bottom_right.x + 80.0, bottom_right.y + 40.0),
        false,
    ));
    let resized = &annotations.shapes()[0];
    assert_eq!(resized.stroke_width, stroke);
    assert!(resized.bounds().width > moved_bounds.width + 70.0);
    assert!(resized.bounds().height > moved_bounds.height + 30.0);
    assert_eq!(ink_points(&annotations).1.len(), 3);
}

#[test]
fn filled_pen_path_can_be_selected_from_its_interior() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Pen);
    annotations.set_filled(true);
    annotations.begin(TOUCH, Point::new(100.0, 100.0), PAGE);
    annotations.drag(TOUCH, Point::new(200.0, 100.0), false);
    annotations.finish(TOUCH, Point::new(150.0, 220.0), false);

    annotations.set_tool(Tool::Rectangle);
    annotations.begin(TOUCH, Point::new(300.0, 300.0), PAGE);
    annotations.finish(TOUCH, Point::new(400.0, 400.0), false);
    assert_eq!(annotations.selected_index(), Some(1));

    annotations.set_tool(Tool::Select);
    assert!(annotations.begin_interaction(
        TOUCH,
        Point::new(150.0, 140.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(annotations.finish(TOUCH, Point::new(150.0, 140.0), false));
    assert_eq!(annotations.selected_index(), Some(0));
}

#[test]
fn ink_cancel_delete_and_history_restore_whole_paths() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Pen);
    annotations.begin(TOUCH, Point::new(100.0, 100.0), PAGE);
    annotations.drag(TOUCH, Point::new(150.0, 180.0), false);
    annotations.finish(TOUCH, Point::new(220.0, 120.0), false);
    let original = annotations.shapes()[0].clone();

    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        TOUCH,
        Point::new(125.0, 140.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    );
    annotations.drag(TOUCH, Point::new(175.0, 190.0), false);
    annotations.cancel();
    assert_eq!(annotations.shapes(), std::slice::from_ref(&original));

    annotations.begin_interaction(
        TOUCH,
        Point::new(125.0, 140.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    );
    annotations.finish(TOUCH, Point::new(175.0, 190.0), false);
    let moved = annotations.shapes()[0].clone();
    assert_ne!(moved, original);
    assert!(annotations.delete_selected());
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), std::slice::from_ref(&moved));
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), std::slice::from_ref(&original));
}
