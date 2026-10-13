use fastfill_rust::annotations::{
    AnnotationGeometry, Annotations, DragModifiers, EditHit, LineEndpoint,
    LineKind, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;
const PLAIN: DragModifiers = DragModifiers {
    square: false,
    force_move: false,
    force_resize: false,
};

fn line() -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Line);
    assert!(annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE));
    assert!(annotations.finish(MOUSE, Point::new(300.0, 200.0), false,));
    annotations
}

fn endpoints(annotations: &Annotations) -> (Point, Point) {
    match annotations.shapes()[0].geometry {
        AnnotationGeometry::Linear {
            kind: LineKind::Line,
            start,
            end,
        } => (start, end),
        AnnotationGeometry::Linear {
            kind: LineKind::Arrow,
            ..
        } => panic!("expected line, found arrow"),
        AnnotationGeometry::Bounded { .. } => panic!("expected line"),
    }
}

#[test]
fn lines_keep_explicit_endpoints_in_every_direction() {
    for end in [
        Point::new(300.0, 100.0),
        Point::new(100.0, 300.0),
        Point::new(300.0, 200.0),
        Point::new(50.0, 25.0),
    ] {
        let mut annotations = Annotations::default();
        annotations.set_tool(Tool::Line);
        annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
        annotations.drag(MOUSE, end, false);
        let preview = annotations.preview().expect("line preview");
        assert_eq!(
            preview.geometry,
            AnnotationGeometry::Linear {
                kind: LineKind::Line,
                start: Point::new(100.0, 100.0),
                end,
            },
        );
        assert!(annotations.finish(MOUSE, end, false));
        assert_eq!(annotations.shapes()[0], preview);
    }
}

#[test]
fn ctrl_snaps_line_to_nearest_45_degree_angle_and_toggles_live() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Line);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.drag(MOUSE, Point::new(190.0, 140.0), false);
    annotations.constrain_square(true);
    let AnnotationGeometry::Linear {
        kind: LineKind::Line,
        end: snapped,
        ..
    } = annotations.preview().unwrap().geometry
    else {
        panic!("expected line");
    };
    assert!((snapped.x - 169.64).abs() < 0.02);
    assert!((snapped.y - 169.64).abs() < 0.02);

    annotations.constrain_square(false);
    assert_eq!(
        annotations.preview().unwrap().geometry,
        AnnotationGeometry::Linear {
            kind: LineKind::Line,
            start: Point::new(100.0, 100.0),
            end: Point::new(190.0, 140.0),
        },
    );
}

#[test]
fn horizontal_and_vertical_lines_commit_but_zero_length_does_not() {
    for end in [Point::new(300.0, 100.0), Point::new(100.0, 300.0)] {
        let mut annotations = Annotations::default();
        annotations.set_tool(Tool::Line);
        annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
        assert!(annotations.finish(MOUSE, end, false));
    }

    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Line);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    assert!(!annotations.finish(MOUSE, Point::new(100.0, 100.0), false,));
}

#[test]
fn line_body_moves_and_each_endpoint_edits_independently() {
    let mut annotations = line();
    let stroke = annotations.shapes()[0].stroke_width;
    annotations.set_tool(Tool::Select);
    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(200.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert_eq!(
        annotations.hit_selected(Point::new(200.0, 150.0), MOUSE, 1.0, PLAIN,),
        Some(EditHit::Move),
    );
    annotations.finish(MOUSE, Point::new(220.0, 180.0), false);
    assert_eq!(
        endpoints(&annotations),
        (Point::new(120.0, 130.0), Point::new(320.0, 230.0)),
    );

    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(120.0, 130.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert_eq!(
        annotations.hit_selected(Point::new(120.0, 130.0), MOUSE, 1.0, PLAIN,),
        Some(EditHit::Endpoint(LineEndpoint::Start)),
    );
    annotations.finish(MOUSE, Point::new(80.0, 90.0), false);
    assert_eq!(
        endpoints(&annotations),
        (Point::new(80.0, 90.0), Point::new(320.0, 230.0)),
    );
    assert_eq!(annotations.shapes()[0].stroke_width, stroke);
}

#[test]
fn ctrl_snaps_endpoint_edit_around_the_fixed_endpoint() {
    let mut annotations = line();
    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(300.0, 200.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(180.0, 250.0), true);
    let (start, end) = endpoints(&annotations);
    assert_eq!(start, Point::new(100.0, 100.0));
    assert!((end.x - 220.21).abs() < 0.02);
    assert!((end.y - 220.21).abs() < 0.02);
}

#[test]
fn line_edits_participate_in_cancel_delete_and_history() {
    let mut annotations = line();
    let original = annotations.shapes()[0];
    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(200.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.drag(MOUSE, Point::new(250.0, 175.0), false);
    annotations.cancel();
    assert_eq!(annotations.shapes()[0], original);

    annotations.begin_interaction(
        MOUSE,
        Point::new(200.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(250.0, 175.0), false);
    let moved = annotations.shapes()[0];
    assert_ne!(moved, original);
    assert!(annotations.delete_selected());
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), [moved]);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes(), [original]);
}

#[test]
fn line_hit_width_is_stable_in_screen_pixels_across_zoom_levels() {
    for scale in [0.25, 1.0, 4.0] {
        let mut annotations = line();
        annotations.set_tool(Tool::Select);
        assert!(annotations.begin_interaction(
            MOUSE,
            Point::new(200.0, 150.0 + 6.0 / scale),
            PAGE,
            scale,
            PLAIN,
        ));
        assert_eq!(annotations.selected_index(), Some(0));
    }
}

#[test]
fn moving_a_line_off_page_keeps_a_reachable_part_without_scaling_it() {
    let mut annotations = line();
    let original = endpoints(&annotations);
    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(200.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(-1_000.0, -1_000.0), false);
    let moved = endpoints(&annotations);
    assert_eq!(
        Point::new(moved.1.x - moved.0.x, moved.1.y - moved.0.y),
        Point::new(original.1.x - original.0.x, original.1.y - original.0.y),
    );
    let bounds = annotations.shapes()[0].bounds();
    assert!(bounds.x < 0.0 && bounds.x + bounds.width > 0.0);
    assert!(bounds.y < 0.0 && bounds.y + bounds.height > 0.0);
}

#[test]
fn selected_line_moves_from_its_box_and_modifiers_disambiguate() {
    let mut annotations = line();
    annotations.set_tool(Tool::Select);
    let inside_box = Point::new(150.0, 190.0);
    assert_eq!(
        annotations.hit_selected(inside_box, MOUSE, 1.0, PLAIN),
        Some(EditHit::Move),
    );
    assert!(
        annotations.begin_interaction(MOUSE, inside_box, PAGE, 1.0, PLAIN,)
    );
    annotations.cancel();
    assert_eq!(annotations.selected_index(), Some(0));

    assert_eq!(
        annotations.hit_selected(
            Point::new(100.0, 100.0),
            MOUSE,
            1.0,
            DragModifiers {
                force_move: true,
                ..PLAIN
            },
        ),
        Some(EditHit::Move),
    );
    assert_eq!(
        annotations.hit_selected(
            Point::new(200.0, 150.0),
            MOUSE,
            1.0,
            DragModifiers {
                force_resize: true,
                ..PLAIN
            },
        ),
        Some(EditHit::Endpoint(LineEndpoint::Start)),
    );
}
