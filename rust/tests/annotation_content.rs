use fastfill_rust::annotations::{
    AnnotationColor, AnnotationGeometry, AnnotationRef, Annotations,
    DragModifiers, OwnedAnnotation, Pointer, ShapeAnnotation, ShapeKind,
    TextAnnotation, TextFormat, Tool,
};
use fastfill_rust::geometry::{Point, Rect, Size};
use uuid::Uuid;

#[test]
fn content_identity_equality_and_ownership_ignore_borrowed_indices() {
    let id = Uuid::from_u128(17);
    let bounds = Rect::new(-4.0, 20.0, 80.0, 40.0);
    let color = AnnotationColor::Argb(0x7066_7788);
    let annotations = [
        OwnedAnnotation::Shape(ShapeAnnotation {
            id,
            geometry: AnnotationGeometry::Bounded {
                kind: ShapeKind::Rectangle,
                bounds,
            },
            stroke_width: 7.0,
            color,
            filled: true,
            rotation: 0.4,
        }),
        OwnedAnnotation::Text(TextAnnotation {
            id,
            text: "Some\ntext".into(),
            bounds,
            format: TextFormat {
                color,
                bold: true,
                ..TextFormat::default()
            },
            rotation: 0.4,
        }),
    ];
    for annotation in &annotations {
        let borrowed = match annotation {
            OwnedAnnotation::Shape(shape) => AnnotationRef::Shape(19, shape),
            OwnedAnnotation::Text(text) => AnnotationRef::Text(23, text),
        };
        assert_eq!(borrowed.id(), id);
        assert!(annotation.matches(borrowed));
        assert_eq!(OwnedAnnotation::from(borrowed), *annotation);
        let mut changed = annotation.clone();
        match &mut changed {
            OwnedAnnotation::Shape(shape) => shape.rotation = 0.5,
            OwnedAnnotation::Text(text) => text.format.bold = false,
        }
        assert!(!changed.matches(borrowed));
    }
    assert!(!annotations[0].matches(annotations[1].as_ref()));
    assert!(!annotations[1].matches(annotations[0].as_ref()));
}

const PAGE: Size = Size::new(800.0, 1_100.0);
const FINGER: Pointer = Pointer::Touch(1);

fn mixed_editor() -> Annotations {
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Rectangle);
    assert!(editor.begin(FINGER, Point::new(100.0, 100.0), PAGE));
    assert!(editor.finish(FINGER, Point::new(200.0, 200.0), false));
    editor.set_tool(Tool::Text);
    assert!(editor.begin_text(Point::new(300.0, 150.0), PAGE, 1.0));
    editor.update_text("Original", Size::new(100.0, 32.0));
    assert!(editor.accept_text());
    editor.set_tool(Tool::Select);
    editor
}

fn content(editor: &Annotations) -> Vec<OwnedAnnotation> {
    editor
        .ordered_annotations()
        .map(OwnedAnnotation::from)
        .collect()
}

#[test]
fn pending_text_content_cannot_start_another_geometric_gesture() {
    let mut editor = mixed_editor();
    let before = content(&editor);
    let revision = editor.revision();
    assert!(editor.begin_text_edit(Point::new(320.0, 150.0), PAGE, 1.0));
    editor.update_text("Pending", Size::new(120.0, 32.0));
    assert!(!editor.begin_interaction(
        FINGER,
        Point::new(600.0, 600.0),
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(editor.active_pointer().is_none());
    assert!(editor.has_text_edit());
    assert!(!editor.delete_all());
    assert!(!editor.set_color(AnnotationColor::Red));
    assert!(!editor.undo());
    assert_eq!(content(&editor), before);
    assert_eq!(editor.revision(), revision);
    editor.cancel();
    assert_eq!(content(&editor), before);
}

#[test]
fn every_geometric_draft_owns_its_pointer_and_blocks_content_commands() {
    for kind in ["create", "shape", "text", "group", "marquee"] {
        let mut editor = mixed_editor();
        let point = match kind {
            "create" => {
                editor.set_tool(Tool::Rectangle);
                Point::new(500.0, 500.0)
            }
            "text" => Point::new(320.0, 150.0),
            "shape" | "group" => {
                let start = Point::new(80.0, 80.0);
                let end = Point::new(
                    if kind == "shape" { 220.0 } else { 450.0 },
                    250.0,
                );
                assert!(editor.begin_interaction(
                    FINGER,
                    start,
                    PAGE,
                    1.0,
                    DragModifiers::default(),
                ));
                assert!(editor.finish(FINGER, end, false));
                Point::new(150.0, 150.0)
            }
            _ => Point::new(600.0, 600.0),
        };
        let before = content(&editor);
        let revision = editor.revision();
        assert!(editor.begin_interaction(
            FINGER,
            point,
            PAGE,
            1.0,
            DragModifiers::default(),
        ));
        assert!(!editor.delete_all(), "{kind}: delete all");
        assert!(!editor.delete_selected(), "{kind}: delete selected");
        assert!(!editor.set_color(AnnotationColor::Red), "{kind}: color");
        assert!(!editor.set_stroke_width(7.0), "{kind}: stroke");
        assert!(!editor.set_filled(true), "{kind}: fill");
        assert!(!editor.send_selected_to_back(), "{kind}: layers");
        assert!(!editor.reset_rotation(), "{kind}: rotation");
        assert!(!editor.undo());
        assert!(!editor.redo());
        assert!(!editor.begin_text(point, PAGE, 1.0));
        editor.drag(Pointer::Touch(2), Point::new(700.0, 700.0), false);
        assert!(!editor.finish(Pointer::Mouse, point, false));
        assert_eq!(editor.active_pointer(), Some(FINGER));
        assert_eq!(content(&editor), before);
        assert_eq!(editor.revision(), revision);
        editor.cancel();
        assert!(editor.active_pointer().is_none());
        assert_eq!(content(&editor), before);
        assert!(editor.can_undo());
    }
}

#[test]
fn text_geometry_no_op_cancel_and_commit_have_one_history_boundary() {
    let mut editor = mixed_editor();
    let before = content(&editor);
    let revision = editor.revision();
    let start = Point::new(320.0, 150.0);
    let end = Point::new(340.0, 180.0);
    assert!(editor.begin_text_move(FINGER, start, PAGE, 1.0));
    assert!(editor.finish(FINGER, start, false));
    assert_eq!(editor.revision(), revision);
    assert_eq!(content(&editor), before);

    assert!(editor.begin_text_move(FINGER, start, PAGE, 1.0));
    editor.drag(FINGER, end, false);
    let preview = editor
        .geometric_previews()
        .find_map(|a| a.text().cloned())
        .unwrap();
    assert_eq!(editor.selected_text_annotation(), Some(preview.clone()));
    assert_eq!(editor.selection_texts(), std::slice::from_ref(&preview));
    assert_eq!(editor.revision(), revision);
    editor.cancel();
    assert_eq!(content(&editor), before);

    assert!(editor.begin_text_move(FINGER, start, PAGE, 1.0));
    editor.drag(FINGER, end, false);
    // Preserve the old text release policy: use the last finite point.
    assert!(editor.finish(FINGER, Point::new(f32::NAN, 0.0), false));
    assert_eq!(editor.texts().next(), Some(&preview));
    let changed = content(&editor);
    assert!(editor.undo());
    assert_eq!(content(&editor), before);
    assert_eq!(editor.revision(), revision);
    assert!(editor.redo());
    assert_eq!(content(&editor), changed);
}

#[test]
fn layer_no_ops_preserve_revision_selection_and_redo() {
    let mut editor = mixed_editor();
    let before = content(&editor);
    let selected = editor.selected_ids().to_vec();
    let revision = editor.revision();
    assert!(!editor.bring_selected_to_front());
    assert_eq!(editor.revision(), revision);
    assert!(editor.send_selected_to_back());
    let reordered = content(&editor);
    let reordered_revision = editor.revision();
    assert!(!editor.send_selected_to_back());
    assert_eq!(editor.revision(), reordered_revision);
    assert!(editor.undo());
    assert_eq!(content(&editor), before);
    assert_eq!(editor.selected_ids(), selected);
    assert!(!editor.bring_selected_to_front());
    assert!(editor.can_redo());
    assert!(editor.redo());
    assert_eq!(content(&editor), reordered);
}

#[test]
fn mixed_geometric_previews_follow_layers_not_annotation_type() {
    let mut editor = mixed_editor();
    let shape = Point::new(150.0, 150.0);
    assert!(editor.begin_interaction(
        FINGER,
        shape,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    assert!(editor.finish(FINGER, shape, false));
    assert!(editor.bring_selected_to_front());
    let expected: Vec<_> = editor
        .ordered_annotations()
        .map(AnnotationRef::id)
        .collect();
    let text = Point::new(320.0, 150.0);
    assert!(editor.begin_interaction(
        FINGER,
        text,
        PAGE,
        1.0,
        DragModifiers {
            square: true,
            ..DragModifiers::default()
        },
    ));
    assert!(editor.finish(FINGER, text, true));
    assert_eq!(editor.selected_count(), 2);
    assert!(editor.begin_interaction(
        FINGER,
        text,
        PAGE,
        1.0,
        DragModifiers::default(),
    ));
    editor.drag(FINGER, Point::new(340.0, 180.0), false);
    let previews: Vec<_> = editor.geometric_previews().collect();
    assert!(matches!(previews[0], OwnedAnnotation::Text(_)));
    assert!(matches!(previews[1], OwnedAnnotation::Shape(_)));
    assert_eq!(
        previews.iter().map(OwnedAnnotation::id).collect::<Vec<_>>(),
        expected
    );
    assert!(editor.is_text_edited(0));
    assert!(editor.is_edited(1));
    assert_eq!(editor.selection_texts().len(), 1);
    assert_eq!(editor.selection_annotations(1.0).len(), 1);
    editor.cancel();
}
