use fastfill_rust::annotations::{
    Annotations, DragModifiers, EditHit, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(612.0, 800.0);
const MOUSE: Pointer = Pointer::Mouse;

#[test]
fn tiny_shapes_move_instead_of_using_hidden_handles_even_with_alt() {
    for tool in [
        Tool::Rectangle,
        Tool::Oval,
        Tool::Checkmark,
        Tool::Cross,
        Tool::Line,
        Tool::Arrow,
        Tool::Pen,
        Tool::Highlighter,
    ] {
        for pointer in [MOUSE, Pointer::Touch(1)] {
            let mut state = Annotations::default();
            state.set_tool(tool);
            state.begin(MOUSE, Point::new(200.0, 200.0), PAGE);
            let end = if matches!(tool, Tool::Checkmark | Tool::Cross) {
                Point::new(200.0, 200.0)
            } else {
                Point::new(220.0, 215.0)
            };
            state.finish(MOUSE, end, false);
            state.set_tool(Tool::Select);
            let shape = state.shapes().next().unwrap().clone();
            let corner = shape
                .frame_corners()
                .map_or(Point::new(200.0, 200.0), |corners| corners[0]);
            for force_resize in [false, true] {
                let modifiers = DragModifiers {
                    force_resize,
                    ..DragModifiers::default()
                };
                assert_eq!(
                    state.hit_selected(corner, pointer, 0.25, modifiers),
                    Some(EditHit::Move),
                    "{tool:?}"
                );
                assert!(
                    state.begin_interaction(
                        pointer, corner, PAGE, 0.25, modifiers
                    )
                );
                state.finish(
                    pointer,
                    Point::new(corner.x + 10.0, corner.y + 12.0),
                    false,
                );
                assert_eq!(
                    state.shapes().next().unwrap().bounds().width,
                    shape.bounds().width
                );
                assert_eq!(
                    state.shapes().next().unwrap().bounds().height,
                    shape.bounds().height
                );
                assert!(state.undo());
            }
            assert_ne!(
                state.hit_selected(
                    corner,
                    pointer,
                    4.0,
                    DragModifiers::default()
                ),
                Some(EditHit::Move)
            );
        }
    }
}

#[test]
fn horizontal_lines_have_endpoint_handles_when_long_enough() {
    let mut state = Annotations::default();
    state.set_tool(Tool::Line);
    state.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    state.finish(MOUSE, Point::new(300.0, 100.0), false);
    let point = Point::new(100.0, 100.0);
    assert!(matches!(
        state.hit_selected(point, MOUSE, 1.0, DragModifiers::default()),
        Some(EditHit::Endpoint(_))
    ));
    assert_eq!(
        state.hit_selected(point, MOUSE, 0.1, DragModifiers::default()),
        Some(EditHit::Move)
    );
}

#[test]
fn visible_controls_never_overlap_even_on_rotated_narrow_shapes() {
    use fastfill_rust::annotations::{
        AnnotationColor, AnnotationGeometry, SELECTION_HANDLE_SIZE,
        SELECTION_STROKE, ShapeAnnotation, ShapeKind,
    };
    use fastfill_rust::geometry::Rect;

    let mut shape = ShapeAnnotation {
        id: uuid::Uuid::new_v4(),
        geometry: AnnotationGeometry::Bounded {
            kind: ShapeKind::Rectangle,
            bounds: Rect::new(100.0, 100.0, 32.0, 32.0),
        },
        color: AnnotationColor::Black,
        stroke_width: 2.0,
        filled: false,
        rotation: 0.0,
    };
    let extent = SELECTION_HANDLE_SIZE + SELECTION_STROKE;
    for width in [1.0, 16.0, 32.0, 64.0, 200.0] {
        for height in [1.0, 16.0, 32.0, 64.0, 200.0] {
            for rotation in [0.0, 0.3, std::f32::consts::FRAC_PI_4, 1.2] {
                shape.geometry = AnnotationGeometry::Bounded {
                    kind: ShapeKind::Rectangle,
                    bounds: Rect::new(100.0, 100.0, width, height),
                };
                shape.rotation = rotation;
                for scale in [0.25, 1.0, 4.0] {
                    if !shape.edit_handles_visible(scale) {
                        assert!(shape.rotation_handle(scale).is_none());
                        continue;
                    }
                    let mut handles = shape.frame_corners().unwrap().to_vec();
                    handles.push(shape.rotation_handle(scale).unwrap());
                    for (i, a) in handles.iter().enumerate() {
                        for b in &handles[i + 1..] {
                            assert!(
                                (a.x - b.x).abs() * scale >= extent
                                    || (a.y - b.y).abs() * scale >= extent
                            );
                        }
                    }
                    assert!(
                        extent * extent <= 0.5 * width * height * scale * scale
                    );
                }
            }
        }
    }
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(!shape.edit_handles_visible(scale));
    }
}
