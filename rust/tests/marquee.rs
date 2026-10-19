use fastfill_rust::annotations::{Annotations, DragModifiers, Pointer, Tool};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1100.0);
const MOUSE: Pointer = Pointer::Mouse;

fn drawing(tool: Tool, points: &[Point], filled: bool) -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(tool);
    if filled {
        annotations.set_filled(true);
    }
    assert!(annotations.begin(MOUSE, points[0], PAGE));
    for point in &points[1..points.len() - 1] {
        annotations.drag(MOUSE, *point, false);
    }
    assert!(annotations.finish(MOUSE, *points.last().unwrap(), false));
    annotations.set_tool(Tool::Select);
    annotations
}

fn select(annotations: &mut Annotations, start: Point, end: Point) -> bool {
    let empty = Point::new(750.0, 1000.0);
    let plain = DragModifiers::default();
    assert!(annotations.begin_interaction(MOUSE, empty, PAGE, 1.0, plain));
    assert!(annotations.finish(MOUSE, empty, false));
    assert!(!annotations.has_selection());
    let revision = annotations.revision();
    assert!(annotations.begin_interaction(MOUSE, start, PAGE, 1.0, plain));
    assert!(annotations.finish(MOUSE, end, false));
    assert_eq!(annotations.revision(), revision, "selection must not edit");
    annotations.has_selection()
}

#[test]
fn diagonal_line_and_arrow_require_a_stroke_intersection() {
    for tool in [Tool::Line, Tool::Arrow] {
        for (start, end) in [
            (Point::new(100.0, 100.0), Point::new(500.0, 500.0)),
            (Point::new(500.0, 500.0), Point::new(100.0, 100.0)),
            (Point::new(100.0, 500.0), Point::new(500.0, 100.0)),
        ] {
            let mut annotations = drawing(tool, &[start, end], false);
            assert!(!select(
                &mut annotations,
                Point::new(120.0, 280.0),
                Point::new(140.0, 300.0),
            ));
            assert!(select(
                &mut annotations,
                Point::new(80.0, 280.0),
                Point::new(520.0, 300.0),
            ));
        }
    }
}

#[test]
fn arrowhead_and_thick_stroke_edges_are_selectable() {
    let mut annotations = drawing(
        Tool::Arrow,
        &[Point::new(100.0, 300.0), Point::new(500.0, 300.0)],
        false,
    );
    assert!(select(
        &mut annotations,
        Point::new(450.0, 340.0),
        Point::new(478.0, 309.0),
    ));
    assert!(annotations.set_stroke_width(20.0));
    assert!(select(
        &mut annotations,
        Point::new(200.0, 330.0),
        Point::new(300.0, 308.0),
    ));
    assert!(!select(
        &mut annotations,
        Point::new(200.0, 330.0),
        Point::new(300.0, 312.0),
    ));
}

#[test]
fn high_zoom_clicks_can_pick_the_full_visible_line_width() {
    for tool in [Tool::Line, Tool::Arrow] {
        let mut annotations = drawing(
            tool,
            &[Point::new(100.0, 300.0), Point::new(500.0, 300.0)],
            false,
        );
        assert!(annotations.set_stroke_width(20.0));
        assert!(!select(
            &mut annotations,
            Point::new(600.0, 500.0),
            Point::new(620.0, 520.0),
        ));
        let point = Point::new(300.0, 309.0);
        assert!(annotations.begin_interaction(
            MOUSE,
            point,
            PAGE,
            4.0,
            DragModifiers::default(),
        ));
        assert!(annotations.finish(MOUSE, point, false));
        assert_eq!(annotations.selected_index(), Some(0));
    }
}

#[test]
fn diagonal_stroke_padding_does_not_grow_toward_frame_corners() {
    let mut annotations = drawing(
        Tool::Line,
        &[Point::new(100.0, 100.0), Point::new(500.0, 500.0)],
        false,
    );
    assert!(annotations.set_stroke_width(20.0));
    assert!(!select(
        &mut annotations,
        Point::new(250.0, 270.0),
        Point::new(254.0, 274.0),
    ));
    assert!(select(
        &mut annotations,
        Point::new(250.0, 270.0),
        Point::new(258.0, 274.0),
    ));
}

#[test]
fn open_pen_and_highlighter_do_not_select_their_empty_interiors() {
    for tool in [Tool::Pen, Tool::Highlighter] {
        let mut annotations = drawing(
            tool,
            &[
                Point::new(100.0, 100.0),
                Point::new(100.0, 400.0),
                Point::new(500.0, 400.0),
            ],
            false,
        );
        assert!(!select(
            &mut annotations,
            Point::new(280.0, 240.0),
            Point::new(320.0, 270.0),
        ));
        assert!(select(
            &mut annotations,
            Point::new(80.0, 240.0),
            Point::new(120.0, 270.0),
        ));
    }
}

#[test]
fn rotated_ink_uses_its_transformed_strokes() {
    let mut annotations = drawing(
        Tool::Pen,
        &[
            Point::new(100.0, 100.0),
            Point::new(100.0, 400.0),
            Point::new(500.0, 400.0),
        ],
        false,
    );
    let shape = &annotations.shapes().next().unwrap().clone();
    let handle = shape.rotation_handle(1.0).unwrap();
    let center = shape.bounds().center();
    let radius = (handle.y - center.y).abs();
    assert!(annotations.begin_interaction(
        MOUSE,
        handle,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(annotations.finish(
        MOUSE,
        Point::new(center.x + radius, center.y),
        false,
    ));
    assert!(!select(
        &mut annotations,
        Point::new(280.0, 240.0),
        Point::new(320.0, 270.0),
    ));
    assert!(select(
        &mut annotations,
        Point::new(120.0, 240.0),
        Point::new(170.0, 270.0),
    ));
}

#[test]
fn filled_pen_requires_overlap_with_its_polygon_not_its_frame() {
    let mut annotations = drawing(
        Tool::Pen,
        &[
            Point::new(100.0, 100.0),
            Point::new(500.0, 100.0),
            Point::new(300.0, 500.0),
        ],
        true,
    );
    assert!(!select(
        &mut annotations,
        Point::new(110.0, 400.0),
        Point::new(150.0, 440.0),
    ));
    assert!(select(
        &mut annotations,
        Point::new(60.0, 260.0),
        Point::new(240.0, 280.0),
    ));
}

#[test]
fn filled_pen_closing_stroke_is_clickable_outside_the_fill() {
    let mut annotations = drawing(
        Tool::Pen,
        &[
            Point::new(100.0, 100.0),
            Point::new(500.0, 100.0),
            Point::new(300.0, 500.0),
        ],
        true,
    );
    assert!(!select(
        &mut annotations,
        Point::new(600.0, 600.0),
        Point::new(620.0, 620.0),
    ));
    let point = Point::new(194.0, 303.0);
    assert!(annotations.begin_interaction(
        MOUSE,
        point,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(annotations.finish(MOUSE, point, false));
    assert_eq!(annotations.selected_index(), Some(0));
}

#[test]
fn rotated_frames_do_not_select_empty_corners_of_their_outer_bounds() {
    let mut annotations = drawing(
        Tool::Rectangle,
        &[Point::new(100.0, 200.0), Point::new(500.0, 250.0)],
        false,
    );
    let shape = &annotations.shapes().next().unwrap().clone();
    let handle = shape.rotation_handle(1.0).unwrap();
    let center = shape.bounds().center();
    let radius = (handle.y - center.y).abs();
    let delta = radius / 2.0_f32.sqrt();
    assert!(annotations.begin_interaction(
        MOUSE,
        handle,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(annotations.finish(
        MOUSE,
        Point::new(center.x + delta, center.y - delta),
        false,
    ));
    assert!(!select(
        &mut annotations,
        Point::new(150.0, 340.0),
        Point::new(175.0, 365.0),
    ));
    assert!(select(
        &mut annotations,
        Point::new(100.0, 200.0),
        Point::new(320.0, 250.0),
    ));
}
