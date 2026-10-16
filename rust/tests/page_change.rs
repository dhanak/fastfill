use fastfill_rust::annotations::{Annotations, Pointer, Tool};
use fastfill_rust::geometry::{Point, Size};

fn annotated() -> Annotations {
    let mut state = Annotations::default();
    let page = Size::new(612.0, 900.0);
    state.set_tool(Tool::Rectangle);
    state.begin(Pointer::Mouse, Point::new(100.0, 500.0), page);
    state.finish(Pointer::Mouse, Point::new(200.0, 600.0), false);
    state.set_tool(Tool::Text);
    state.begin_text(Point::new(300.0, 800.0), page, 1.0);
    state.update_text("Keep me", Size::new(120.0, 30.0));
    state.accept_text();
    state
}

#[test]
fn page_changes_stage_without_mutation_and_undo_as_one_step() {
    let mut state = annotated();
    let shapes = state.shapes().to_vec();
    let texts = state.texts().to_vec();
    let revision = state.revision();
    let staged = state.prepare_page_change(Size::new(612.0, 400.0)).unwrap();
    assert_eq!(state.shapes(), shapes);
    assert_eq!(state.texts(), texts);
    assert_eq!(state.revision(), revision);
    assert_eq!(staged.annotations().shapes()[0].id, shapes[0].id);
    assert!(staged.annotations().shapes()[0].bounds().y < 400.0);
    assert!(staged.annotations().texts()[0].bounds.y < 400.0);
    assert_eq!(state.commit_page_change(staged, 1), Some(2));
    assert_eq!(state.page_context(), 1);
    assert_ne!(state.revision(), revision);
    let moved_shapes = state.shapes().to_vec();
    let moved_texts = state.texts().to_vec();
    assert!(state.undo());
    assert_eq!(state.page_context(), 0);
    assert_eq!(state.revision(), revision);
    assert_eq!(state.shapes(), shapes);
    assert_eq!(state.texts(), texts);
    assert!(state.redo());
    assert_eq!(state.page_context(), 1);
    assert_eq!(state.shapes(), moved_shapes);
    assert_eq!(state.texts(), moved_texts);
}

#[test]
fn partly_visible_objects_are_not_clamped_or_resized() {
    let mut state = annotated();
    let shapes = state.shapes().to_vec();
    let staged = state.prepare_page_change(Size::new(612.0, 550.0)).unwrap();
    assert_eq!(staged.annotations().shapes(), shapes);
    assert_eq!(state.commit_page_change(staged, 1), Some(1));
    assert_eq!(state.shapes(), shapes);
    assert_eq!(state.texts()[0].format.font_size, 24.0);
}

#[test]
fn empty_page_changes_still_have_history_but_invalid_or_pending_edits_do_not() {
    let mut state = Annotations::default();
    for size in [
        Size::new(0.0, 20.0),
        Size::new(20.0, f32::NAN),
        Size::new(f32::INFINITY, 20.0),
    ] {
        assert!(state.prepare_page_change(size).is_none());
    }
    let page = Size::new(612.0, 400.0);
    let staged = state.prepare_page_change(page).unwrap();
    assert_eq!(state.commit_page_change(staged, 1), Some(0));
    assert!(state.can_undo());
    assert!(state.undo());
    assert_eq!(state.page_context(), 0);
    state.set_tool(Tool::Rectangle);
    state.begin(Pointer::Mouse, Point::new(50.0, 50.0), page);
    assert!(state.prepare_page_change(page).is_none());
}

#[test]
fn scaling_is_independent_for_shapes_and_keeps_text_proportions_and_strokes() {
    let state = annotated();
    let staged = state
        .prepare_mapped_page_change(Size::new(1200.0, 2000.0), |point| {
            Some((
                Point::new(point.x * 1.25, point.y * 0.75),
                Size::new(1.5, 0.8),
            ))
        })
        .unwrap();
    let shape = &staged.annotations().shapes()[0];
    let bounds = shape.bounds();
    assert_eq!(
        bounds,
        fastfill_rust::geometry::Rect::new(112.5, 372.5, 150.0, 80.0)
    );
    assert_eq!(shape.stroke_width, state.shapes()[0].stroke_width);
    assert_eq!(shape.rotation, state.shapes()[0].rotation);
    let text = &staged.annotations().texts()[0];
    assert!((text.format.font_size - 19.2).abs() < 0.001);
    assert!((text.bounds.width - 96.0).abs() < 0.001);
    assert!((text.bounds.height - 24.0).abs() < 0.001);
}

#[test]
fn invalid_mapping_or_out_of_range_text_size_cannot_change_the_original() {
    let mut state = annotated();
    let revision = state.revision();
    let page = Size::new(1200.0, 2000.0);
    for scale in [
        Size::new(f32::NAN, 1.0),
        Size::new(0.0, 1.0),
        Size::new(1.0, 20.0),
    ] {
        assert!(
            state
                .prepare_mapped_page_change(page, |p| Some((p, scale)))
                .is_none()
        );
        assert_eq!(state.revision(), revision);
    }
    let staged = state.prepare_page_change(page).unwrap();
    state.delete_selected();
    assert_eq!(
        state.commit_page_change(staged, 1),
        None,
        "reject stale staged content"
    );
}
