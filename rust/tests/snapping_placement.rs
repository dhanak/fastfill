use fastfill_rust::annotations::{
    AnnotationColor, Annotations, DragModifiers, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Rect, Size};

const PAGE: Size = Size::new(612.0, 800.0);
const MOUSE: Pointer = Pointer::Mouse;
const CLICK: Point = Point::new(300.0, 300.0);
const TARGET: Rect = Rect::new(100.0, 100.0, 20.0, 20.0);

#[test]
fn snapped_marks_preview_and_commit_as_one_styled_undoable_creation() {
    for tool in [Tool::Checkmark, Tool::Cross] {
        let mut annotations = Annotations::default();
        annotations.set_tool(tool);
        annotations.set_color(AnnotationColor::Green);
        annotations.set_stroke_width(2.0);
        assert!(annotations.begin(MOUSE, CLICK, PAGE));
        assert!(annotations.set_mark_snap(MOUSE, TARGET, 8.0));
        assert_eq!(annotations.preview().unwrap().bounds(), TARGET);
        assert!(annotations.finish(MOUSE, Point::new(301.0, 300.0), false));
        let mark = annotations.shapes()[0].clone();
        assert_eq!(mark.bounds(), TARGET);
        assert_eq!(mark.color, AnnotationColor::Green);
        assert_eq!(mark.stroke_width, 2.0);
        assert!(annotations.undo());
        assert!(annotations.is_empty());
        assert!(!annotations.can_undo());
        assert!(annotations.redo());
        assert_eq!(annotations.shapes(), [mark]);
    }
}

#[test]
fn manual_mark_drags_and_existing_edits_do_not_snap() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Checkmark);
    annotations.begin(MOUSE, CLICK, PAGE);
    annotations.set_mark_snap(MOUSE, TARGET, 8.0);
    assert!(annotations.finish(MOUSE, Point::new(350.0, 350.0), false));
    assert_eq!(
        annotations.shapes()[0].bounds(),
        Rect::new(250.0, 250.0, 100.0, 100.0)
    );
    annotations.set_tool(Tool::Select);
    annotations.begin_interaction(
        MOUSE,
        CLICK,
        PAGE,
        1.0,
        DragModifiers::default(),
    );
    assert!(!annotations.set_mark_snap(MOUSE, TARGET, 8.0));
    assert!(annotations.finish(MOUSE, Point::new(310.0, 320.0), false));
    assert_eq!(
        annotations.shapes()[0].bounds(),
        Rect::new(260.0, 270.0, 100.0, 100.0)
    );
}

#[test]
fn invalid_or_wrong_pointer_mark_hints_leave_the_draft_unchanged() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Cross);
    annotations.begin(MOUSE, CLICK, PAGE);
    let original = annotations.preview().unwrap();
    for (bounds, threshold) in [
        (Rect::new(f32::NAN, 0.0, 20.0, 20.0), 8.0),
        (Rect::new(0.0, 0.0, 0.0, 0.0), 8.0),
        (Rect::new(0.0, 0.0, 20.0, 30.0), 8.0),
        (TARGET, 0.0),
        (TARGET, f32::INFINITY),
    ] {
        assert!(!annotations.set_mark_snap(MOUSE, bounds, threshold));
        assert_eq!(annotations.preview(), Some(original.clone()));
    }
    assert!(!annotations.set_mark_snap(Pointer::Touch(1), TARGET, 8.0));
    annotations.cancel();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, CLICK, PAGE);
    assert!(!annotations.set_mark_snap(MOUSE, TARGET, 8.0));
    annotations.cancel();
    assert!(annotations.is_empty());
}

#[test]
fn new_text_stays_above_its_line_as_content_and_format_change() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    assert!(annotations.begin_text(CLICK, PAGE, 2.0));
    assert!(annotations.set_text_line_anchor(Point::new(100.0, 400.0)));
    annotations.update_text("Dátum", Size::new(80.0, 32.0));
    assert_eq!(
        annotations.text_preview().unwrap().bounds,
        Rect::new(100.0, 368.0, 80.0, 32.0)
    );
    let mut format = annotations.current_text_format().unwrap();
    format.bold = true;
    format.font_size = 32.0;
    assert!(annotations.set_text_format(format));
    annotations.update_text("Dátum\nSecond line", Size::new(900.0, 84.0));
    assert_eq!(
        annotations.text_preview().unwrap().bounds,
        Rect::new(100.0, 316.0, 900.0, 84.0)
    );
    assert!(annotations.accept_text());
    let text = annotations.texts()[0].clone();
    assert!(annotations.undo());
    assert!(annotations.is_empty());
    assert!(annotations.redo());
    assert_eq!(annotations.texts(), std::slice::from_ref(&text));

    assert!(annotations.begin_text_edit(text.bounds.center(), PAGE, 1.0));
    assert!(!annotations.set_text_line_anchor(Point::new(200.0, 200.0)));
    annotations.update_text("Canceled", Size::new(80.0, 32.0));
    annotations.cancel();
    assert_eq!(annotations.texts(), [text]);
}

#[test]
fn line_reuse_matches_anchors_not_overflow_and_respects_layer_order() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    let anchor = Point::new(200.0, 400.0);
    for (label, position, width) in [
        ("Target", anchor, 100.0),
        (
            "Overflow from adjacent cell",
            Point::new(20.0, 400.0),
            400.0,
        ),
        ("Top field", Point::new(200.3, 400.4), 150.0),
    ] {
        assert!(annotations.begin_text(CLICK, PAGE, 1.0));
        assert!(annotations.set_text_line_anchor(position));
        annotations.update_text(label, Size::new(width, 30.0));
        assert!(annotations.accept_text());
    }
    let saved = annotations.texts().to_vec();
    let revision = annotations.revision();
    for scale in [0.25, 1.0, 4.0] {
        assert!(
            annotations.begin_text_edit_on_line(CLICK, anchor, PAGE, scale,)
        );
        assert_eq!(annotations.text_preview().unwrap(), saved[2]);
        assert_eq!(annotations.revision(), revision);
        annotations.cancel();
        assert_eq!(annotations.texts(), saved);
    }
    assert!(annotations.send_selected_to_back());
    assert!(annotations.begin_text_edit_on_line(CLICK, anchor, PAGE, 1.0));
    assert_eq!(annotations.text_preview().unwrap(), saved[0]);
}

#[test]
fn line_reuse_rejects_other_fields_invalid_input_and_pending_drafts() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    let anchor = Point::new(200.0, 400.0);
    annotations.begin_text(CLICK, PAGE, 1.0);
    annotations.set_text_line_anchor(anchor);
    annotations.update_text("Existing", Size::new(200.0, 30.0));
    annotations.accept_text();
    let saved = annotations.texts().to_vec();
    let revision = annotations.revision();
    for (point, target, scale) in [
        (CLICK, Point::new(202.0, 400.0), 1.0),
        (CLICK, Point::new(200.0, 402.0), 1.0),
        (CLICK, Point::new(f32::NAN, 400.0), 1.0),
        (Point::new(f32::NAN, 200.0), anchor, 1.0),
        (Point::new(PAGE.width, 200.0), anchor, 1.0),
        (CLICK, anchor, 0.0),
        (CLICK, anchor, f32::INFINITY),
    ] {
        assert!(
            !annotations.begin_text_edit_on_line(point, target, PAGE, scale,)
        );
        assert!(!annotations.has_text_edit());
        assert_eq!(annotations.texts(), saved);
        assert_eq!(annotations.revision(), revision);
    }
    assert!(annotations.begin_text(CLICK, PAGE, 1.0));
    let draft = annotations.text_preview();
    assert!(!annotations.begin_text_edit_on_line(CLICK, anchor, PAGE, 1.0));
    assert_eq!(annotations.text_preview(), draft);
    annotations.cancel();
    annotations.set_tool(Tool::Rectangle);
    assert!(!annotations.begin_text_edit_on_line(CLICK, anchor, PAGE, 1.0));
    assert_eq!(annotations.texts(), saved);
}
