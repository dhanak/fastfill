use fastfill_rust::annotations::{
    AnnotationColor, AnnotationRef, Annotations, DragModifiers, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;
const PLAIN: DragModifiers = DragModifiers {
    square: false,
    force_move: false,
    force_resize: false,
};
const CTRL: DragModifiers = DragModifiers {
    square: true,
    ..PLAIN
};

fn rectangles(xs: &[f32]) -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    for x in xs {
        annotations.begin(MOUSE, Point::new(*x, 100.0), PAGE);
        annotations.finish(MOUSE, Point::new(*x + 50.0, 150.0), false);
    }
    annotations.set_tool(Tool::Select);
    annotations
}

fn ctrl_click(annotations: &mut Annotations, point: Point) {
    assert!(annotations.begin_interaction(MOUSE, point, PAGE, 1.0, CTRL));
    assert!(annotations.active_pointer().is_none());
}

fn marquee(annotations: &mut Annotations, start: Point, end: Point) {
    assert!(annotations.begin_interaction(MOUSE, start, PAGE, 1.0, PLAIN));
    assert!(annotations.finish(MOUSE, end, false));
}

#[test]
fn ctrl_click_toggles_objects_without_starting_a_drag() {
    let mut annotations = rectangles(&[100.0, 200.0, 300.0]);
    assert_eq!(annotations.selected_indices(), [2]);

    ctrl_click(&mut annotations, Point::new(125.0, 125.0));
    assert_eq!(annotations.selected_indices(), [0, 2]);
    ctrl_click(&mut annotations, Point::new(325.0, 125.0));
    assert_eq!(annotations.selected_indices(), [0]);

    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(225.0, 125.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert_eq!(annotations.selected_indices(), [1]);
    annotations.cancel();
}

#[test]
fn empty_area_drag_selects_every_intersecting_object() {
    let mut annotations = rectangles(&[100.0, 200.0, 400.0]);
    marquee(
        &mut annotations,
        Point::new(80.0, 80.0),
        Point::new(280.0, 180.0),
    );

    assert_eq!(annotations.selected_indices(), [0, 1]);
}

#[test]
fn moving_a_selection_is_one_undoable_change() {
    let mut annotations = rectangles(&[100.0, 200.0, 400.0]);
    marquee(
        &mut annotations,
        Point::new(80.0, 80.0),
        Point::new(280.0, 180.0),
    );
    let original = annotations.shapes().to_vec();

    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(125.0, 125.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert!(annotations.finish(MOUSE, Point::new(175.0, 165.0), false));
    assert_eq!(annotations.shapes()[0].bounds().x, 150.0);
    assert_eq!(annotations.shapes()[1].bounds().x, 250.0);
    assert_eq!(annotations.shapes()[2], original[2]);

    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), original);
    assert_eq!(annotations.selected_indices(), [0, 1]);
    assert!(annotations.redo());
    assert_eq!(annotations.shapes()[0].bounds().x, 150.0);
    assert_eq!(annotations.shapes()[1].bounds().x, 250.0);
}

#[test]
fn group_move_keeps_every_selected_object_reachable() {
    let mut annotations = rectangles(&[100.0, 400.0]);
    marquee(
        &mut annotations,
        Point::new(80.0, 80.0),
        Point::new(480.0, 180.0),
    );

    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(125.0, 125.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert!(annotations.finish(MOUSE, Point::new(-875.0, 125.0), false));

    assert_eq!(annotations.shapes()[0].bounds().x, -49.0);
    assert_eq!(annotations.shapes()[1].bounds().x, 251.0);
}

#[test]
fn group_moves_from_inside_a_selected_lines_frame() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Line);
    for offset in [0.0, 200.0] {
        assert!(annotations.begin(
            MOUSE,
            Point::new(100.0 + offset, 100.0),
            PAGE,
        ));
        assert!(annotations.finish(
            MOUSE,
            Point::new(150.0 + offset, 150.0),
            false,
        ));
    }
    annotations.set_tool(Tool::Select);
    marquee(
        &mut annotations,
        Point::new(80.0, 80.0),
        Point::new(380.0, 180.0),
    );
    let before: Vec<_> = annotations
        .shapes()
        .iter()
        .map(|shape| shape.bounds().x)
        .collect();

    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(105.0, 145.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert!(annotations.finish(MOUSE, Point::new(125.0, 145.0), false));

    let after: Vec<_> = annotations
        .shapes()
        .iter()
        .map(|shape| shape.bounds().x)
        .collect();
    assert_eq!(after, before.iter().map(|x| x + 20.0).collect::<Vec<_>>());
}

#[test]
fn bulk_styles_apply_once_and_fill_only_compatible_objects() {
    let mut annotations = rectangles(&[100.0, 200.0]);
    annotations.set_tool(Tool::Line);
    annotations.begin(MOUSE, Point::new(300.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(350.0, 150.0), false);
    annotations.set_tool(Tool::Select);
    marquee(
        &mut annotations,
        Point::new(80.0, 80.0),
        Point::new(380.0, 180.0),
    );

    assert!(annotations.set_color(AnnotationColor::Red));
    assert!(annotations.set_stroke_width(7.0));
    assert!(annotations.set_filled(true));
    assert!(
        annotations
            .shapes()
            .iter()
            .all(|shape| shape.color == AnnotationColor::Red)
    );
    assert!(
        annotations
            .shapes()
            .iter()
            .all(|shape| (shape.stroke_width - 7.0).abs() < f32::EPSILON)
    );
    assert!(annotations.shapes()[0].filled);
    assert!(annotations.shapes()[1].filled);
    assert!(!annotations.shapes()[2].filled);
}

#[test]
fn selected_and_all_deletion_are_single_undoable_changes() {
    let mut annotations = rectangles(&[100.0, 200.0, 300.0]);
    marquee(
        &mut annotations,
        Point::new(80.0, 80.0),
        Point::new(280.0, 180.0),
    );
    assert!(annotations.delete_selected());
    assert_eq!(annotations.shapes().len(), 1);
    assert_eq!(annotations.selected_indices(), &[] as &[usize]);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes().len(), 3);
    assert_eq!(annotations.selected_indices(), [0, 1]);

    assert!(annotations.delete_all());
    assert_eq!(annotations.shapes(), []);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes().len(), 3);
}

#[test]
fn layer_commands_keep_relative_order_and_selection_through_history() {
    let mut annotations = rectangles(&[100.0, 200.0, 300.0, 400.0]);
    ctrl_click(&mut annotations, Point::new(225.0, 125.0));
    assert_eq!(annotations.selected_indices(), [1, 3]);

    assert!(annotations.send_selected_to_back());
    let xs = ordered_shape_xs(&annotations);
    assert_eq!(xs, [200.0, 400.0, 100.0, 300.0]);
    assert_eq!(annotations.selected_indices(), [1, 3]);

    assert!(annotations.undo());
    assert_eq!(annotations.selected_indices(), [1, 3]);
    assert!(annotations.bring_selected_to_front());
    let xs = ordered_shape_xs(&annotations);
    assert_eq!(xs, [100.0, 300.0, 200.0, 400.0]);
    assert_eq!(annotations.selected_indices(), [1, 3]);
}

fn ordered_shape_xs(annotations: &Annotations) -> Vec<f32> {
    annotations
        .ordered_annotations()
        .filter_map(|annotation| match annotation {
            AnnotationRef::Shape(_, shape) => Some(shape.bounds().x),
            AnnotationRef::Text(_, _) => None,
        })
        .collect()
}
