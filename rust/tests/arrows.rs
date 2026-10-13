use fastfill_rust::annotations::{
    AnnotationGeometry, Annotations, DragModifiers, EditHit, LineKind, Pointer,
    Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;

fn linear_geometry(annotations: &Annotations) -> (LineKind, Point, Point) {
    match annotations.shapes()[0].geometry {
        AnnotationGeometry::Linear { kind, start, end } => (kind, start, end),
        AnnotationGeometry::Bounded { .. } => panic!("expected arrow"),
    }
}

#[test]
fn arrow_tool_creates_directed_arrow_geometry() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Arrow);
    annotations.begin(MOUSE, Point::new(300.0, 200.0), PAGE);
    annotations.finish(MOUSE, Point::new(100.0, 400.0), false);

    assert_eq!(
        linear_geometry(&annotations),
        (
            LineKind::Arrow,
            Point::new(300.0, 200.0),
            Point::new(100.0, 400.0),
        ),
    );
}

#[test]
fn arrow_kind_survives_move_endpoint_edit_and_history() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Arrow);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 200.0), false);
    let original = annotations.shapes()[0];

    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(200.0, 150.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    );
    annotations.finish(MOUSE, Point::new(250.0, 175.0), false);
    assert_eq!(linear_geometry(&annotations).0, LineKind::Arrow);

    let (_, _, end) = linear_geometry(&annotations);
    annotations.begin_interaction(
        MOUSE,
        end,
        PAGE,
        1.0,
        DragModifiers::default(),
    );
    annotations.finish(MOUSE, Point::new(400.0, 400.0), true);
    assert_eq!(linear_geometry(&annotations).0, LineKind::Arrow);

    assert!(annotations.undo());
    assert_eq!(linear_geometry(&annotations).0, LineKind::Arrow);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), [original]);
}

#[test]
fn arrowhead_is_part_of_bounds_and_hit_testing() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Arrow);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 100.0), false);
    annotations.set_tool(Tool::Select);

    let bounds = annotations.shapes()[0].bounds();
    assert!(bounds.y < 100.0);
    assert!(bounds.y + bounds.height > 100.0);
    assert_eq!(
        annotations.hit_selected(
            Point::new(279.0, 109.0),
            MOUSE,
            1.0,
            DragModifiers::default(),
        ),
        Some(EditHit::Move),
    );
}
