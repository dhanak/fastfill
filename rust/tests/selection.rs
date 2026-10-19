use fastfill_rust::annotations::{
    Annotations, DragModifiers, EditHit, Pointer, ResizeCorner, ShapeKind, Tool,
};
use fastfill_rust::geometry::{Point, Rect, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;
const PLAIN: DragModifiers = DragModifiers {
    square: false,
    force_move: false,
    force_resize: false,
};

fn sample() -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 200.0), false);
    annotations.set_tool(Tool::Select);
    annotations
}

#[test]
fn creation_selects_the_object_and_body_hit_selects_topmost() {
    let mut annotations = sample();
    assert_eq!(annotations.selected_index(), Some(0));
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(200.0, 150.0), PAGE);
    annotations.finish(MOUSE, Point::new(400.0, 250.0), false);
    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(250.0, 175.0),
        PAGE,
        1.0,
        PLAIN,
    );
    assert_eq!(annotations.selected_index(), Some(1));
    annotations.cancel();
    annotations.begin_interaction(
        MOUSE,
        Point::new(10.0, 10.0),
        PAGE,
        1.0,
        PLAIN,
    );
    assert!(annotations.finish(MOUSE, Point::new(10.0, 10.0), false));
    assert_eq!(annotations.selected_index(), None);
}

#[test]
fn move_preview_does_not_modify_the_object_until_release() {
    let mut annotations = sample();
    let original = annotations.shapes().next().unwrap().clone();
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.drag(MOUSE, Point::new(230.0, 170.0), false);
    assert_eq!(annotations.shapes().next().unwrap().clone(), original);
    assert_eq!(
        annotations.preview().unwrap().bounds(),
        Rect::new(150.0, 120.0, 200.0, 100.0),
    );
    assert!(annotations.finish(MOUSE, Point::new(240.0, 175.0), false));
    assert_eq!(
        annotations.shapes().next().unwrap().bounds(),
        Rect::new(160.0, 125.0, 200.0, 100.0),
    );
}

#[test]
fn every_corner_resizes_freely_without_changing_stroke() {
    for (start, end, expected) in [
        ((100.0, 100.0), (80.0, 70.0), (80.0, 70.0, 220.0, 130.0)),
        ((300.0, 100.0), (330.0, 80.0), (100.0, 80.0, 230.0, 120.0)),
        ((300.0, 200.0), (350.0, 280.0), (100.0, 100.0, 250.0, 180.0)),
        ((100.0, 200.0), (80.0, 250.0), (80.0, 100.0, 220.0, 150.0)),
    ] {
        let mut annotations = sample();
        let stroke = annotations.shapes().next().unwrap().stroke_width;
        annotations.begin_interaction(
            MOUSE,
            Point::new(start.0, start.1),
            PAGE,
            1.0,
            PLAIN,
        );
        annotations.finish(MOUSE, Point::new(end.0, end.1), false);
        let object = &annotations.shapes().next().unwrap().clone();
        assert_eq!(object.stroke_width, stroke);
        assert_eq!(
            object.bounds(),
            Rect::new(expected.0, expected.1, expected.2, expected.3),
        );
    }
}

#[test]
fn resize_can_cross_the_opposite_corner_and_ctrl_can_toggle_live() {
    let mut annotations = sample();
    annotations.begin_interaction(
        MOUSE,
        Point::new(300.0, 200.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.drag(MOUSE, Point::new(80.0, 70.0), false);
    assert_eq!(
        annotations.preview().unwrap().bounds(),
        Rect::new(80.0, 70.0, 20.0, 30.0),
    );
    annotations.constrain_square(true);
    assert_eq!(
        annotations.preview().unwrap().bounds(),
        Rect::new(70.0, 70.0, 30.0, 30.0),
    );
    annotations.finish(MOUSE, Point::new(80.0, 70.0), false);
    assert_eq!(annotations.shapes().next().unwrap().bounds().width, 20.0);
}

#[test]
fn cancel_and_tool_change_restore_original_move_or_resize() {
    for start in [Point::new(180.0, 150.0), Point::new(300.0, 200.0)] {
        let mut annotations = sample();
        let original = annotations.shapes().next().unwrap().clone();
        annotations.begin_interaction(MOUSE, start, PAGE, 1.0, PLAIN);
        annotations.drag(MOUSE, Point::new(400.0, 400.0), false);
        annotations.cancel();
        assert_eq!(annotations.shapes().next().unwrap().clone(), original);
        assert_eq!(annotations.selected_index(), Some(0));
        annotations.begin_interaction(MOUSE, start, PAGE, 1.0, PLAIN);
        annotations.set_tool(Tool::Rectangle);
        assert!(annotations.preview().is_none());
        assert_eq!(annotations.shapes().next().unwrap().clone(), original);
    }
}

#[test]
fn invalid_release_reverts_and_other_pointers_cannot_take_over() {
    let mut annotations = sample();
    let original = annotations.shapes().next().unwrap().clone();
    let finger = Pointer::Touch(1);
    annotations.begin_interaction(
        finger,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    assert!(!annotations.begin_interaction(
        Pointer::Touch(2),
        Point::new(250.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    ));
    assert!(!annotations.finish(MOUSE, Point::new(400.0, 400.0), false));
    assert_eq!(annotations.active_pointer(), Some(finger));
    assert!(!annotations.finish(finger, Point::new(f32::NAN, 400.0), false));
    assert_eq!(annotations.shapes().next().unwrap().clone(), original);
    assert!(annotations.active_pointer().is_none());
}

#[test]
fn partial_off_page_moves_are_preserved_and_fully_off_page_stays_reachable() {
    let mut annotations = sample();
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(30.0, 150.0), false);
    assert_eq!(annotations.shapes().next().unwrap().bounds().x, -50.0);
    annotations.begin_interaction(
        MOUSE,
        Point::new(20.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(-1_000.0, 150.0), false);
    let bounds = annotations.shapes().next().unwrap().bounds();
    assert!(bounds.x < 0.0 && bounds.x + bounds.width > 0.0);
    assert_eq!(bounds.width, 200.0);
}

#[test]
fn reachability_limit_is_continuous_when_crossing_and_reversing() {
    for scale in [0.25, 1.0, 4.0] {
        let mut annotations = sample();
        annotations.begin_interaction(
            MOUSE,
            Point::new(180.0, 150.0),
            PAGE,
            scale,
            PLAIN,
        );

        annotations.drag(MOUSE, Point::new(-118.0, 150.0), false);
        assert_eq!(annotations.preview().unwrap().bounds().x, -198.0);
        annotations.drag(MOUSE, Point::new(-120.0, 150.0), false);
        assert_eq!(annotations.preview().unwrap().bounds().x, -199.0);
        annotations.drag(MOUSE, Point::new(-300.0, 150.0), false);
        assert_eq!(annotations.preview().unwrap().bounds().x, -199.0);

        annotations.drag(MOUSE, Point::new(-119.0, 150.0), false);
        assert_eq!(annotations.preview().unwrap().bounds().x, -199.0);
        annotations.drag(MOUSE, Point::new(-110.0, 150.0), false);
        assert_eq!(annotations.preview().unwrap().bounds().x, -190.0);
    }
}

#[test]
fn small_objects_move_until_zoom_reveals_their_resize_targets() {
    for scale in [0.25, 1.0, 4.0] {
        let mut annotations = Annotations::default();
        annotations.set_tool(Tool::Rectangle);
        annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
        annotations.finish(MOUSE, Point::new(110.0, 110.0), false);
        annotations.set_tool(Tool::Select);
        assert_eq!(
            annotations.hit_selected(
                Point::new(105.0, 105.0),
                MOUSE,
                scale,
                PLAIN
            ),
            Some(EditHit::Move),
        );
        let controls = annotations.selection_bounds(scale).unwrap();
        assert_eq!(controls, Rect::new(100.0, 100.0, 10.0, 10.0));
        let handle =
            Point::new(controls.x - 2.0 / scale, controls.y - 2.0 / scale);
        assert_eq!(
            annotations.hit_selected(handle, Pointer::Touch(1), scale, PLAIN),
            (scale >= 4.0).then_some(EditHit::Resize(ResizeCorner::TopLeft)),
        );
    }
}

#[test]
fn shift_forces_move_and_alt_forces_nearest_corner_resize() {
    let annotations = sample();
    assert_eq!(
        annotations.hit_selected(
            Point::new(300.0, 200.0),
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
            Point::new(270.0, 180.0),
            MOUSE,
            1.0,
            DragModifiers {
                force_resize: true,
                ..PLAIN
            },
        ),
        Some(EditHit::Resize(ResizeCorner::BottomRight)),
    );
}

#[test]
fn shift_can_select_and_move_another_object() {
    let mut annotations = sample();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(500.0, 500.0), PAGE);
    annotations.finish(MOUSE, Point::new(600.0, 600.0), false);
    annotations.set_tool(Tool::Select);
    assert!(annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        DragModifiers {
            force_move: true,
            ..PLAIN
        },
    ));
    annotations.finish(MOUSE, Point::new(190.0, 160.0), false);
    assert_eq!(annotations.selected_index(), Some(0));
    assert_eq!(annotations.shapes().next().unwrap().bounds().x, 110.0);
}

#[test]
fn rectangle_tool_edits_selected_object_and_draws_elsewhere() {
    let mut annotations = sample();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(190.0, 160.0), false);
    assert_eq!(annotations.shapes().count(), 1);
    annotations.begin_interaction(
        MOUSE,
        Point::new(500.0, 500.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(550.0, 550.0), false);
    assert_eq!(annotations.shapes().count(), 2);
    assert_eq!(annotations.selected_index(), Some(1));
}

#[test]
fn delete_removes_only_selected_object_and_is_disabled_during_a_drag() {
    let mut annotations = sample();
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    assert!(!annotations.delete_selected());
    annotations.cancel();
    assert!(annotations.delete_selected());
    assert_eq!(annotations.shapes().cloned().collect::<Vec<_>>(), []);
    assert_eq!(annotations.selected_index(), None);
    assert!(!annotations.delete_selected());
}

#[test]
fn create_move_and_delete_undo_and_redo_in_order() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 200.0), false);
    let original = annotations.shapes().next().unwrap().clone();

    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(230.0, 170.0), false);
    let moved = annotations.shapes().next().unwrap().clone();
    assert!(annotations.delete_selected());
    assert_eq!(annotations.shapes().cloned().collect::<Vec<_>>(), []);

    assert!(annotations.undo());
    assert_eq!(
        annotations.shapes().cloned().collect::<Vec<_>>(),
        std::slice::from_ref(&moved)
    );
    assert_eq!(annotations.selected_index(), Some(0));
    assert!(annotations.undo());
    assert_eq!(
        annotations.shapes().cloned().collect::<Vec<_>>(),
        std::slice::from_ref(&original)
    );
    assert!(annotations.undo());
    assert_eq!(annotations.shapes().cloned().collect::<Vec<_>>(), []);
    assert!(!annotations.undo());

    assert!(annotations.redo());
    assert_eq!(
        annotations.shapes().cloned().collect::<Vec<_>>(),
        [original]
    );
    assert!(annotations.redo());
    assert_eq!(annotations.shapes().cloned().collect::<Vec<_>>(), [moved]);
    assert!(annotations.redo());
    assert_eq!(annotations.shapes().cloned().collect::<Vec<_>>(), []);
    assert!(!annotations.redo());
}

#[test]
fn drafts_and_noop_edits_do_not_create_history() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.drag(MOUSE, Point::new(300.0, 200.0), false);
    let preview = annotations.preview();
    assert!(!annotations.undo());
    assert_eq!(annotations.preview(), preview);
    annotations.cancel();
    assert!(!annotations.can_undo());

    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 200.0), false);
    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(180.0, 150.0), false);

    assert!(annotations.undo());
    assert_eq!(annotations.shapes().cloned().collect::<Vec<_>>(), []);
    assert!(!annotations.can_undo());
}

#[test]
fn a_new_change_after_undo_clears_redo() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    for (start, end) in [
        (Point::new(100.0, 100.0), Point::new(200.0, 200.0)),
        (Point::new(300.0, 300.0), Point::new(400.0, 400.0)),
    ] {
        annotations.begin(MOUSE, start, PAGE);
        annotations.finish(MOUSE, end, false);
    }
    assert!(annotations.undo());
    assert!(annotations.can_redo());

    annotations.begin(MOUSE, Point::new(500.0, 500.0), PAGE);
    annotations.finish(MOUSE, Point::new(600.0, 600.0), false);
    assert!(!annotations.can_redo());
    assert!(!annotations.redo());
    assert_eq!(annotations.shapes().count(), 2);
}

#[test]
fn oval_edit_and_history_preserve_shape_kind() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Oval);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(300.0, 200.0), false);
    let original = annotations.shapes().next().unwrap().clone();
    assert_eq!(original.kind(), Some(ShapeKind::Oval));

    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        Point::new(180.0, 150.0),
        PAGE,
        1.0,
        PLAIN,
    );
    annotations.finish(MOUSE, Point::new(230.0, 170.0), false);
    assert_eq!(
        annotations.shapes().next().unwrap().kind(),
        Some(ShapeKind::Oval)
    );
    assert!(annotations.undo());
    assert_eq!(
        annotations.shapes().cloned().collect::<Vec<_>>(),
        [original]
    );
}
