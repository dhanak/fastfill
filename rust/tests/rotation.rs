use fastfill_rust::annotations::{
    Annotations, DragModifiers, EditHit, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;
const PLAIN: DragModifiers = DragModifiers {
    square: false,
    force_move: false,
    force_resize: false,
};

fn rectangle() -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 200.0), false);
    annotations.set_tool(Tool::Select);
    annotations
}

#[test]
fn rotation_handle_rotates_around_center_and_participates_in_history() {
    let mut annotations = rectangle();
    let handle = annotations.shapes()[0].rotation_handle(1.0).unwrap();
    assert_eq!(
        annotations.hit_selected(handle, MOUSE, 1.0, PLAIN),
        Some(EditHit::Rotate),
    );

    assert!(annotations.begin_interaction(MOUSE, handle, PAGE, 1.0, PLAIN));
    assert!(annotations.finish(MOUSE, Point::new(330.0, 150.0), false));
    let quarter_turn = annotations.shapes()[0].rotation;
    assert!((quarter_turn - std::f32::consts::FRAC_PI_2).abs() < 0.001);

    assert!(annotations.undo());
    assert_eq!(annotations.shapes()[0].rotation, 0.0);
    assert!(annotations.redo());
    assert_eq!(annotations.shapes()[0].rotation, quarter_turn);
    assert!(annotations.reset_rotation());
    assert_eq!(annotations.shapes()[0].rotation, 0.0);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes()[0].rotation, quarter_turn);
}

#[test]
fn ctrl_snaps_rotation_and_rotated_body_remains_selectable() {
    let mut annotations = rectangle();
    let handle = annotations.shapes()[0].rotation_handle(1.0).unwrap();
    let center = annotations.shapes()[0].bounds().center();
    let angle = -40.0_f32.to_radians();
    let end = Point::new(
        center.x + 100.0 * angle.cos(),
        center.y + 100.0 * angle.sin(),
    );

    assert!(annotations.begin_interaction(MOUSE, handle, PAGE, 1.0, PLAIN));
    assert!(annotations.finish(MOUSE, end, true));
    assert!(
        (annotations.shapes()[0].rotation - std::f32::consts::FRAC_PI_4).abs()
            < 0.001
    );

    let rotated_body = Point::new(200.0, 220.0);
    assert_eq!(
        annotations.hit_selected(rotated_body, MOUSE, 1.0, PLAIN),
        Some(EditHit::Move),
    );
}

#[test]
fn lines_and_arrows_do_not_offer_rotation() {
    for tool in [Tool::Line, Tool::Arrow] {
        let mut annotations = Annotations::default();
        annotations.set_tool(tool);
        annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
        annotations.finish(MOUSE, Point::new(300.0, 200.0), false);

        assert!(annotations.shapes()[0].rotation_handle(1.0).is_none());
        assert!(!annotations.reset_rotation());
    }
}

#[test]
fn ink_uses_the_same_rotation_handle_without_changing_stroke() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Pen);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.drag(MOUSE, Point::new(150.0, 200.0), false);
    annotations.finish(MOUSE, Point::new(200.0, 100.0), false);
    annotations.set_tool(Tool::Select);
    let stroke = annotations.shapes()[0].stroke_width;
    let handle = annotations.shapes()[0].rotation_handle(1.0).unwrap();

    assert!(annotations.begin_interaction(MOUSE, handle, PAGE, 1.0, PLAIN));
    assert!(annotations.finish(MOUSE, Point::new(250.0, 150.0), false));
    assert_ne!(annotations.shapes()[0].rotation, 0.0);
    assert_eq!(annotations.shapes()[0].stroke_width, stroke);
}

#[test]
fn rotated_resize_keeps_opposite_corner_fixed() {
    let mut annotations = rectangle();
    let handle = annotations.shapes()[0].rotation_handle(1.0).unwrap();
    assert!(annotations.begin_interaction(MOUSE, handle, PAGE, 1.0, PLAIN));
    assert!(annotations.finish(MOUSE, Point::new(330.0, 150.0), false));

    let before = annotations.shapes()[0].frame_corners().unwrap();
    let moving = before[2];
    let target = Point::new(moving.x + 30.0, moving.y + 20.0);
    assert!(annotations.begin_interaction(MOUSE, moving, PAGE, 1.0, PLAIN));
    assert!(annotations.finish(MOUSE, target, false));

    let after = annotations.shapes()[0].frame_corners().unwrap();
    assert!((after[0].x - before[0].x).abs() < 0.001);
    assert!((after[0].y - before[0].y).abs() < 0.001);
    assert!((after[2].x - target.x).abs() < 0.001);
    assert!((after[2].y - target.y).abs() < 0.001);
}
