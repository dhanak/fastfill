use fastfill_rust::annotations::{
    Annotations, DragModifiers, EditHit, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1100.0);
const PLAIN: DragModifiers = DragModifiers {
    square: false,
    force_move: false,
    force_resize: false,
};
const CTRL: DragModifiers = DragModifiers {
    square: true,
    ..PLAIN
};
const TOOLS: [Tool; 7] = [
    Tool::Rectangle,
    Tool::Oval,
    Tool::Line,
    Tool::Arrow,
    Tool::Pen,
    Tool::Highlighter,
    Tool::Text,
];

fn objects(tool: Tool) -> Annotations {
    let mut annotations = Annotations::default();
    annotations.set_tool(tool);
    for x in [100.0, 400.0] {
        if tool == Tool::Text {
            assert!(annotations.begin_text(Point::new(x, 100.0), PAGE, 1.0));
            annotations.update_text("Sample", Size::new(150.0, 100.0));
            assert!(annotations.accept_text());
        } else {
            assert!(annotations.begin(
                Pointer::Mouse,
                Point::new(x, 100.0),
                PAGE,
            ));
            assert!(annotations.finish(
                Pointer::Mouse,
                Point::new(x + 150.0, 200.0),
                false,
            ));
        }
    }
    annotations.set_tool(Tool::Select);
    annotations
}

#[test]
fn dragging_an_unselected_object_marquees_instead_of_editing() {
    for tool in TOOLS {
        for scale in [0.25, 1.0, 4.0] {
            for pointer in [Pointer::Mouse, Pointer::Touch(3)] {
                for start in
                    [Point::new(175.0, 150.0), Point::new(100.0, 100.0)]
                {
                    let mut annotations = objects(tool);
                    let shapes = annotations.shapes().to_vec();
                    let texts = annotations.texts().to_vec();
                    let revision = annotations.revision();
                    let end = Point::new(600.0, 250.0);
                    assert!(
                        annotations.begin_interaction(
                            pointer, start, PAGE, scale, PLAIN,
                        )
                    );
                    annotations.drag(pointer, end, false);
                    assert!(
                        annotations.selection_box().is_some(),
                        "marquee from {tool:?}, {pointer:?}, {scale}, {start:?}"
                    );
                    assert!(annotations.finish(pointer, end, false));
                    assert_eq!(annotations.selected_count(), 2);
                    assert_eq!(annotations.shapes(), shapes);
                    assert_eq!(annotations.texts(), texts);
                    assert_eq!(annotations.revision(), revision);
                }
            }
        }
    }
}

#[test]
fn unselected_corner_click_waits_for_release_and_never_resizes() {
    for tool in TOOLS {
        let mut annotations = objects(tool);
        let revision = annotations.revision();
        let start = Point::new(100.0, 100.0);
        assert!(annotations.begin_interaction(
            Pointer::Mouse,
            start,
            PAGE,
            1.0,
            PLAIN,
        ));
        assert_eq!(annotations.selected_count(), 1);
        if tool == Tool::Text {
            assert_eq!(annotations.selected_text_indices(), [1]);
        } else {
            assert_eq!(annotations.selected_indices(), [1]);
        }
        assert!(annotations.finish(
            Pointer::Mouse,
            Point::new(102.0, 101.0),
            false,
        ));
        if tool == Tool::Text {
            assert_eq!(annotations.selected_text_indices(), [0]);
        } else {
            assert_eq!(annotations.selected_indices(), [0]);
        }
        assert_eq!(annotations.revision(), revision);
    }
}

#[test]
fn ctrl_drag_forces_marquee_over_selected_bodies_and_handles() {
    for tool in TOOLS {
        for pointer in [Pointer::Mouse, Pointer::Touch(7)] {
            let initial = objects(tool);
            let mut starts =
                vec![Point::new(475.0, 150.0), Point::new(400.0, 100.0)];
            if let Some(shape) = initial.shapes().last() {
                starts.extend(shape.rotation_handle(1.0));
            } else {
                starts.extend(initial.texts()[1].rotation_handle(1.0));
            }
            for start in starts {
                let mut annotations = objects(tool);
                let revision = annotations.revision();
                let shapes = annotations.shapes().to_vec();
                let texts = annotations.texts().to_vec();
                assert!(
                    annotations
                        .begin_interaction(pointer, start, PAGE, 1.0, CTRL,)
                );
                assert_eq!(annotations.active_pointer(), Some(pointer));
                let end = Point::new(80.0, 250.0);
                annotations.drag(pointer, end, true);
                assert!(annotations.selection_box().is_some());
                assert!(annotations.finish(pointer, end, false));
                assert_eq!(annotations.selected_count(), 2);
                assert_eq!(annotations.shapes(), shapes);
                assert_eq!(annotations.texts(), texts);
                assert_eq!(annotations.revision(), revision);
            }
        }
    }
}

#[test]
fn ctrl_click_toggles_on_release_and_canceled_gestures_keep_selection() {
    let mut annotations = objects(Tool::Rectangle);
    let mouse = Pointer::Mouse;
    let point = Point::new(175.0, 150.0);
    assert!(annotations.begin_interaction(mouse, point, PAGE, 1.0, CTRL));
    assert_eq!(annotations.selected_indices(), [1]);
    assert!(!annotations.finish(Pointer::Touch(1), point, false));
    assert_eq!(annotations.active_pointer(), Some(mouse));
    annotations.cancel();
    assert_eq!(annotations.selected_indices(), [1]);

    assert!(annotations.begin_interaction(mouse, point, PAGE, 1.0, CTRL));
    assert!(!annotations.finish(mouse, Point::new(f32::NAN, 0.0), false));
    assert_eq!(annotations.selected_indices(), [1]);

    assert!(annotations.begin_interaction(mouse, point, PAGE, 1.0, CTRL));
    assert!(annotations.finish(mouse, point, false));
    assert_eq!(annotations.selected_indices(), [0, 1]);
    assert!(annotations.begin_interaction(mouse, point, PAGE, 1.0, CTRL));
    assert!(annotations.finish(mouse, point, false));
    assert_eq!(annotations.selected_indices(), [1]);

    assert!(annotations.begin_interaction(mouse, point, PAGE, 1.0, PLAIN));
    annotations.drag(mouse, Point::new(600.0, 250.0), false);
    annotations.cancel();
    assert_eq!(annotations.selected_indices(), [1]);
}

#[test]
fn ctrl_drag_returning_to_its_start_does_not_become_a_toggle_click() {
    let mut annotations = objects(Tool::Rectangle);
    let mouse = Pointer::Mouse;
    let start = Point::new(475.0, 150.0);
    assert!(annotations.begin_interaction(mouse, start, PAGE, 1.0, CTRL));
    annotations.drag(mouse, Point::new(600.0, 250.0), true);
    annotations.drag(mouse, start, true);
    assert!(annotations.selection_box().is_some());
    assert!(annotations.finish(mouse, start, true));
    assert_eq!(annotations.selected_indices(), [1]);
}

#[test]
fn ctrl_marquee_replaces_a_group_instead_of_toggling_or_moving_it() {
    let mut annotations = objects(Tool::Rectangle);
    let mouse = Pointer::Mouse;
    annotations.set_tool(Tool::Rectangle);
    assert!(annotations.begin(mouse, Point::new(650.0, 100.0), PAGE));
    assert!(annotations.finish(mouse, Point::new(750.0, 200.0), false));
    annotations.set_tool(Tool::Select);
    let start = Point::new(175.0, 150.0);
    assert!(annotations.begin_interaction(mouse, start, PAGE, 1.0, CTRL));
    assert!(annotations.finish(mouse, start, true));
    assert_eq!(annotations.selected_indices(), [0, 2]);
    let revision = annotations.revision();
    assert!(annotations.begin_interaction(mouse, start, PAGE, 1.0, CTRL));
    assert!(annotations.finish(mouse, Point::new(600.0, 250.0), true));
    assert_eq!(annotations.selected_indices(), [0, 1]);
    assert_eq!(annotations.revision(), revision);
}

#[test]
fn explicit_move_and_resize_overrides_win_over_ctrl_marquee() {
    for force_resize in [false, true] {
        let mut annotations = objects(Tool::Rectangle);
        let modifiers = DragModifiers {
            force_resize,
            force_move: !force_resize,
            ..CTRL
        };
        assert!(annotations.begin_interaction(
            Pointer::Mouse,
            Point::new(175.0, 150.0),
            PAGE,
            1.0,
            modifiers,
        ));
        assert!(annotations.preview().is_some());
        assert!(annotations.selection_box().is_none());
        assert!(annotations.finish(
            Pointer::Mouse,
            Point::new(195.0, 170.0),
            true,
        ));
        assert_eq!(annotations.selected_indices(), [0]);
        let bounds = annotations.shapes()[0].bounds();
        if force_resize {
            assert_eq!(bounds.width, bounds.height);
        } else {
            assert_eq!(bounds.x, 120.0);
            assert_eq!(bounds.y, 120.0);
        }
    }
}

#[test]
fn group_hit_targets_offer_move_for_members_but_not_empty_group_gaps() {
    for tool in TOOLS {
        let mut annotations = objects(tool);
        let first = Point::new(175.0, 150.0);
        annotations.begin_interaction(Pointer::Mouse, first, PAGE, 1.0, CTRL);
        annotations.finish(Pointer::Mouse, first, true);
        assert_eq!(annotations.selected_count(), 2);
        for scale in [0.25, 1.0, 4.0] {
            for pointer in [Pointer::Mouse, Pointer::Touch(4)] {
                for point in [first, Point::new(475.0, 150.0)] {
                    assert_eq!(
                        annotations.hit_selected(point, pointer, scale, PLAIN),
                        Some(EditHit::Move),
                        "group member: {tool:?}, {pointer:?}, {scale}"
                    );
                }
                assert_eq!(
                    annotations.hit_selected(
                        Point::new(325.0, 150.0),
                        pointer,
                        scale,
                        PLAIN,
                    ),
                    None
                );
            }
        }
        for scale in [0.0, f32::NAN] {
            assert_eq!(
                annotations.hit_selected(first, Pointer::Mouse, scale, PLAIN,),
                None
            );
        }
        annotations.begin_interaction(Pointer::Mouse, first, PAGE, 1.0, PLAIN);
        let end = Point::new(600.0, 450.0);
        annotations.drag(Pointer::Mouse, end, false);
        assert_eq!(
            annotations.hit_selected(end, Pointer::Mouse, 1.0, PLAIN,),
            Some(EditHit::Move)
        );
        annotations.cancel();
    }
}

#[test]
fn unselected_top_layer_is_not_a_group_move_target() {
    let mut annotations = objects(Tool::Rectangle);
    let mouse = Pointer::Mouse;
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(mouse, Point::new(160.0, 140.0), PAGE);
    annotations.finish(mouse, Point::new(190.0, 160.0), false);
    annotations.set_tool(Tool::Select);
    for point in [
        Point::new(110.0, 110.0),
        Point::new(475.0, 150.0),
        Point::new(175.0, 150.0),
    ] {
        annotations.begin_interaction(mouse, point, PAGE, 1.0, CTRL);
        annotations.finish(mouse, point, true);
    }
    assert_eq!(annotations.selected_indices(), [0, 1]);
    assert_eq!(
        annotations.hit_selected(Point::new(175.0, 150.0), mouse, 1.0, PLAIN,),
        None
    );
    assert_eq!(
        annotations.hit_selected(Point::new(110.0, 110.0), mouse, 1.0, PLAIN,),
        Some(EditHit::Move)
    );
}
