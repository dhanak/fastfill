use fastfill_rust::annotations::{
    AnnotationColor, AnnotationRef, Annotations, MAXIMUM_TEXT_LENGTH, Pointer,
    TextAnnotation, TextEditHit, TextFormat, TextTypeface, Tool,
};
use fastfill_rust::geometry::{Point, Rect, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);

#[test]
fn text_draft_rejects_overlong_unicode_without_losing_prior_content() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    assert!(annotations.begin_text(Point::new(100.0, 100.0), PAGE, 1.0));
    let size = Size::new(100.0, 31.2);
    annotations.update_text("safe", size);
    annotations.update_text("🙂".repeat(MAXIMUM_TEXT_LENGTH / 2 + 1), size);
    assert_eq!(annotations.text_preview().unwrap().text, "safe");
    annotations.update_text("x".repeat(MAXIMUM_TEXT_LENGTH), size);
    assert_eq!(
        annotations.text_preview().unwrap().text.len(),
        MAXIMUM_TEXT_LENGTH
    );
}

#[test]
fn plain_text_stays_pending_until_non_empty_acceptance() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    let anchor = Point::new(300.0, 400.0);

    assert!(annotations.begin_text(anchor, PAGE, 1.0));
    let id = annotations.text_preview().unwrap().id;
    assert_eq!(
        annotations.text_preview(),
        Some(TextAnnotation {
            id,
            text: String::new(),
            bounds: Rect::new(300.0, 384.4, 48.0, 24.0 * 1.3),
            format: TextFormat::default(),
            rotation: 0.0,
        }),
    );
    assert!(!annotations.can_undo());

    annotations.update_text("  Applicant name  ", Size::new(180.0, 31.2));
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), []);
    assert!(annotations.accept_text());
    assert_eq!(
        annotations.texts().cloned().collect::<Vec<_>>(),
        [TextAnnotation {
            id,
            text: "Applicant name".into(),
            bounds: Rect::new(300.0, 384.4, 180.0, 31.2),
            format: TextFormat::default(),
            rotation: 0.0,
        }],
    );
    assert!(annotations.can_undo());
}

#[test]
fn text_near_page_edge_keeps_its_clicked_origin() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);

    assert!(annotations.begin_text(Point::new(790.0, 1.0), PAGE, 1.0));
    let preview = annotations.text_preview().unwrap();

    assert_eq!(
        preview.bounds,
        Rect::new(790.0, 1.0 - 24.0 * 1.3 / 2.0, 48.0, 24.0 * 1.3),
    );
}

#[test]
fn measured_text_bounds_can_extend_beyond_the_page() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    assert!(annotations.begin_text(Point::new(790.0, 1_090.0), PAGE, 1.0));

    let size = Size::new(240.0, 93.6);
    annotations.update_text("One\nTwo\nThree", size);

    let bounds = annotations.text_preview().unwrap().bounds;
    assert_eq!(Size::new(bounds.width, bounds.height), size);
    assert!(bounds.x + bounds.width > PAGE.width);
    assert!(bounds.y + bounds.height > PAGE.height);
}

#[test]
fn text_placement_is_independent_of_zoom() {
    let point = Point::new(300.0, 400.0);
    let mut first = Annotations::default();
    first.set_tool(Tool::Text);
    assert!(first.begin_text(point, PAGE, 1.0));

    let mut second = Annotations::default();
    second.set_tool(Tool::Text);
    assert!(second.begin_text(point, PAGE, 2.0));

    let first = first.text_preview().unwrap();
    let mut second = second.text_preview().unwrap();
    assert_ne!(first.id, second.id);
    second.id = first.id;
    assert_eq!(first, second);
}

#[test]
fn empty_acceptance_and_cancel_leave_no_annotation_or_history() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);

    assert!(annotations.begin_text(Point::new(100.0, 200.0), PAGE, 1.0));
    annotations.update_text("   ", Size::new(48.0, 31.2));
    assert!(annotations.accept_text());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), []);
    assert!(!annotations.can_undo());

    assert!(annotations.begin_text(Point::new(200.0, 300.0), PAGE, 1.0));
    annotations.update_text("Discard me", Size::new(120.0, 31.2));
    annotations.cancel();
    assert!(annotations.text_preview().is_none());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), []);
    assert!(!annotations.can_undo());
}

#[test]
fn accepted_text_participates_in_undo_and_redo() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Date", Size::new(60.0, 31.2));
    annotations.accept_text();
    let accepted = annotations.texts().cloned().collect::<Vec<_>>();

    assert!(annotations.undo());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), []);
    assert!(annotations.redo());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), accepted);
}

#[test]
fn delete_all_removes_text_and_undo_restores_it() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Date", Size::new(60.0, 31.2));
    annotations.accept_text();

    assert!(annotations.delete_all());
    assert!(annotations.is_empty());
    assert!(annotations.undo());
    assert_eq!(annotations.texts().next().unwrap().text, "Date");
}

#[test]
fn multiline_layout_grows_without_moving_its_origin() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    let origin = annotations.text_preview().unwrap().bounds;

    annotations.update_text("First\nSecond", Size::new(100.0, 62.4));
    let updated = annotations.text_preview().unwrap();

    assert_eq!(updated.bounds.x, origin.x);
    assert_eq!(updated.bounds.y, origin.y);
    assert_eq!(updated.bounds.width, 100.0);
    assert!((updated.bounds.height - 62.4).abs() < 0.001);
}

#[test]
fn existing_text_can_be_reopened_and_canceled_without_data_loss() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Original", Size::new(100.0, 31.2));
    annotations.accept_text();
    let original = annotations.texts().next().unwrap().clone();

    assert!(annotations.begin_text_edit(Point::new(320.0, 400.0), PAGE, 1.0,));
    annotations.update_text("Changed", Size::new(110.0, 31.2));
    annotations.cancel();

    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), [original]);
}

#[test]
fn accepted_text_edit_is_one_undoable_change() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Original", Size::new(100.0, 31.2));
    annotations.accept_text();

    assert!(annotations.begin_text_edit(Point::new(320.0, 400.0), PAGE, 1.0,));
    annotations.update_text("Changed", Size::new(110.0, 31.2));
    assert!(annotations.accept_text());
    assert_eq!(annotations.texts().next().unwrap().text, "Changed");

    assert!(annotations.undo());
    assert_eq!(annotations.texts().next().unwrap().text, "Original");
}

#[test]
fn pending_text_edit_cannot_delete_its_committed_source() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Original", Size::new(100.0, 31.2));
    annotations.accept_text();

    assert!(annotations.begin_text_edit(Point::new(320.0, 400.0), PAGE, 1.0,));
    assert!(!annotations.delete_selected());
    annotations.cancel();
    assert_eq!(annotations.texts().next().unwrap().text, "Original");
}

#[test]
fn text_drag_moves_as_one_undoable_edit() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Move", Size::new(80.0, 31.2));
    annotations.accept_text();
    let original = annotations.texts().next().unwrap().clone();
    annotations.set_tool(Tool::Select);

    assert!(annotations.begin_text_move(
        fastfill_rust::annotations::Pointer::Mouse,
        Point::new(320.0, 400.0),
        PAGE,
        1.0,
    ));
    annotations.drag(
        fastfill_rust::annotations::Pointer::Mouse,
        Point::new(350.0, 420.0),
        false,
    );
    assert!(annotations.finish(
        fastfill_rust::annotations::Pointer::Mouse,
        Point::new(350.0, 420.0),
        false,
    ));
    assert_eq!(
        annotations.texts().next().unwrap().bounds.x,
        original.bounds.x + 30.0
    );
    assert_eq!(
        annotations.texts().next().unwrap().bounds.y,
        original.bounds.y + 20.0
    );
    assert!(annotations.undo());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), [original]);
}

#[test]
fn select_click_keeps_text_selected_without_opening_the_editor() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Select", Size::new(90.0, 31.2));
    annotations.accept_text();
    annotations.set_tool(Tool::Select);
    let point = Point::new(320.0, 400.0);

    assert!(annotations.begin_text_move(Pointer::Mouse, point, PAGE, 1.0));
    assert!(annotations.finish(Pointer::Mouse, point, false));

    assert!(!annotations.has_text_edit());
    assert!(annotations.selected_text_annotation().is_some());
}

#[test]
fn inactive_selected_text_format_is_immediate_and_undoable() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Format", Size::new(90.0, 31.2));
    annotations.accept_text();
    annotations.set_tool(Tool::Select);
    let original = annotations.texts().next().unwrap().clone();
    let mut format = original.format;
    format.bold = true;
    format.font_size = 40.0;

    assert_eq!(annotations.current_text_format(), Some(original.format));
    assert!(
        annotations.set_selected_text_format(format, Size::new(160.0, 52.0),)
    );
    assert_eq!(annotations.texts().next().unwrap().format, format);
    assert_eq!(annotations.texts().next().unwrap().bounds.width, 160.0);
    assert!(!annotations.has_text_edit());

    assert!(annotations.undo());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), [original]);
}

#[test]
fn text_format_is_pending_and_escape_restores_the_committed_text() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Original", Size::new(100.0, 31.2));
    annotations.accept_text();
    let original = annotations.texts().next().unwrap().clone();

    assert!(annotations.begin_text_edit(Point::new(320.0, 400.0), PAGE, 1.0));
    let format = TextFormat {
        typeface: TextTypeface::Serif,
        font_size: 32.0,
        bold: true,
        italic: true,
        underline: true,
        color: AnnotationColor::Blue,
    };
    assert!(annotations.set_text_format(format));
    assert_eq!(annotations.text_preview().unwrap().format, format);
    assert_eq!(
        annotations.texts().cloned().collect::<Vec<_>>(),
        std::slice::from_ref(&original)
    );

    annotations.cancel();
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), [original]);
}

#[test]
fn accepted_text_format_is_one_undoable_change_and_becomes_default() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Original", Size::new(100.0, 31.2));
    annotations.accept_text();
    let original = annotations.texts().next().unwrap().clone();

    assert!(annotations.begin_text_edit(Point::new(320.0, 400.0), PAGE, 1.0));
    let format = TextFormat {
        typeface: TextTypeface::Monospace,
        font_size: 40.0,
        bold: true,
        italic: false,
        underline: true,
        color: AnnotationColor::Red,
    };
    assert!(annotations.set_text_format(format));
    annotations.update_text("Original", Size::new(180.0, 52.0));
    assert!(annotations.accept_text());
    assert_eq!(annotations.texts().next().unwrap().format, format);

    assert!(annotations.undo());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), [original]);
    assert!(annotations.redo());
    assert_eq!(annotations.texts().next().unwrap().format, format);

    annotations.set_tool(Tool::Text);
    assert!(annotations.begin_text(Point::new(100.0, 100.0), PAGE, 1.0));
    assert_eq!(annotations.text_preview().unwrap().format, format);
}

#[test]
fn text_rotation_uses_a_handle_ctrl_snap_and_history() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(300.0, 400.0), PAGE, 1.0);
    annotations.update_text("Rotate", Size::new(120.0, 31.2));
    annotations.accept_text();
    annotations.set_tool(Tool::Select);

    let original = annotations.texts().next().unwrap().clone();
    let handle = original.rotation_handle(1.0).unwrap();
    assert_eq!(
        annotations.hit_selected_text(handle, Pointer::Mouse, 1.0),
        Some(TextEditHit::Rotate),
    );
    assert!(annotations.begin_text_move(Pointer::Mouse, handle, PAGE, 1.0,));
    let center = original.bounds.center();
    let end = Point::new(center.x + 100.0, center.y - 100.0);
    annotations.drag(Pointer::Mouse, end, true);
    assert!(annotations.finish(Pointer::Mouse, end, true));
    assert!(
        (annotations.texts().next().unwrap().rotation
            - std::f32::consts::FRAC_PI_4)
            .abs()
            < 0.001,
    );
    assert_eq!(annotations.text_hit(center), Some(0));

    assert!(annotations.undo());
    assert_eq!(annotations.texts().cloned().collect::<Vec<_>>(), [original]);
    assert!(annotations.redo());
    assert!(annotations.can_reset_rotation());
    assert!(annotations.reset_rotation());
    assert_eq!(annotations.texts().next().unwrap().rotation, 0.0);
}

#[test]
fn text_participates_in_shared_layer_order_and_history() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(Pointer::Mouse, Point::new(10.0, 10.0), PAGE);
    annotations.finish(Pointer::Mouse, Point::new(100.0, 100.0), false);
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(30.0, 30.0), PAGE, 1.0);
    annotations.update_text("Middle", Size::new(90.0, 31.2));
    annotations.accept_text();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(Pointer::Mouse, Point::new(20.0, 20.0), PAGE);
    annotations.finish(Pointer::Mouse, Point::new(110.0, 110.0), false);
    annotations.set_tool(Tool::Select);
    assert!(!annotations.begin_text_move(
        Pointer::Mouse,
        Point::new(40.0, 40.0),
        PAGE,
        1.0,
    ));
    annotations.set_tool(Tool::Text);
    assert!(annotations.begin_text_edit(Point::new(40.0, 40.0), PAGE, 1.0,));
    assert!(annotations.accept_text());
    annotations.set_tool(Tool::Select);

    assert_eq!(layer_kinds(&annotations), ["shape", "text", "shape"]);
    assert!(annotations.send_selected_to_back());
    assert_eq!(layer_kinds(&annotations), ["text", "shape", "shape"]);
    assert!(annotations.undo());
    assert_eq!(layer_kinds(&annotations), ["shape", "text", "shape"]);
    assert!(annotations.bring_selected_to_front());
    assert_eq!(layer_kinds(&annotations), ["shape", "shape", "text"]);
}

#[test]
fn text_tool_edits_the_topmost_text_layer() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    let point = Point::new(300.0, 400.0);
    for content in ["First", "Second"] {
        assert!(annotations.begin_text(point, PAGE, 1.0));
        annotations.update_text(content, Size::new(90.0, 31.2));
        assert!(annotations.accept_text());
    }
    annotations.set_tool(Tool::Select);
    assert!(annotations.send_selected_to_back());

    annotations.set_tool(Tool::Text);
    assert!(annotations.begin_text_edit(point, PAGE, 1.0));
    assert_eq!(annotations.text_preview().unwrap().text, "First");
}

fn layer_kinds(annotations: &Annotations) -> Vec<&'static str> {
    annotations
        .ordered_annotations()
        .map(|annotation| match annotation {
            AnnotationRef::Shape(_, _) => "shape",
            AnnotationRef::Text(_, _) => "text",
        })
        .collect()
}
