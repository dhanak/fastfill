use fastfill_rust::annotations::{
    AnnotationColor, AnnotationStyle, Annotations, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;

#[test]
fn tool_capabilities_agree_with_creation_and_option_availability() {
    for (tool, fill, rotation, endpoints, text) in [
        (Tool::Navigate, false, false, false, false),
        (Tool::Select, false, false, false, false),
        (Tool::Text, false, true, false, true),
        (Tool::Rectangle, true, true, false, false),
        (Tool::Oval, true, true, false, false),
        (Tool::Line, false, false, true, false),
        (Tool::Arrow, false, false, true, false),
        (Tool::Checkmark, false, true, false, false),
        (Tool::Cross, false, true, false, false),
        (Tool::Pen, true, true, false, false),
        (Tool::Highlighter, false, true, false, false),
    ] {
        let stroke = tool.creates_annotations() && !text;
        assert_eq!(tool.supports_fill(), fill, "{tool:?}");
        assert_eq!(tool.supports_rotation(), rotation, "{tool:?}");
        assert_eq!(tool.uses_endpoints(), endpoints, "{tool:?}");
        assert_eq!(tool.supports_text_format(), text, "{tool:?}");
        assert_eq!(tool.supports_stroke_width(), stroke, "{tool:?}");
        let mut editor = Annotations::default();
        editor.set_tool(tool);
        assert_eq!(editor.fill_available(), fill, "{tool:?}");
        assert_eq!(editor.stroke_available(), stroke, "{tool:?}");
        if stroke {
            assert!(editor.begin(MOUSE, Point::new(100.0, 100.0), PAGE));
            assert!(editor.finish(MOUSE, Point::new(200.0, 180.0), false));
            let shape = editor.shapes().next().unwrap();
            assert_eq!(shape.supports_rotation(), rotation, "{tool:?}");
            assert_eq!(shape.frame_corners().is_none(), endpoints, "{tool:?}");
            assert_eq!(editor.set_filled(true), fill, "{tool:?}");
            assert!(editor.set_color(AnnotationColor::Green));
            assert!(editor.set_stroke_width(5.0));
        }
    }
}

#[test]
fn style_no_ops_and_tool_defaults_preserve_history_and_redo() {
    let mut editor = Annotations::default();
    editor.set_tool(Tool::Rectangle);
    assert!(editor.set_color(AnnotationColor::Green));
    assert!(editor.set_stroke_width(5.0));
    assert!(editor.set_filled(true));
    assert_eq!(editor.revision(), 0);
    assert!(editor.begin(MOUSE, Point::new(100.0, 100.0), PAGE));
    assert!(editor.finish(MOUSE, Point::new(200.0, 180.0), false));
    let created = editor.shapes().next().unwrap().clone();
    let revision = editor.revision();
    assert!(!editor.set_color(AnnotationColor::Green));
    assert!(!editor.set_stroke_width(5.0));
    assert!(!editor.set_filled(true));
    assert_eq!(editor.revision(), revision);
    assert!(editor.undo());
    assert!(editor.is_empty());
    assert!(editor.set_color(AnnotationColor::Red));
    assert_eq!(editor.revision(), 0);
    assert!(editor.can_redo());
    assert!(editor.redo());
    assert_eq!(editor.shapes().next(), Some(&created));
    assert_eq!(editor.revision(), revision);
}

#[test]
fn drawing_tools_remember_independent_compatible_styles() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    assert_eq!(
        annotations.current_style(),
        Some(AnnotationStyle {
            color: AnnotationColor::Black,
            stroke_width: 3.0,
            filled: false,
        }),
    );
    assert!(annotations.set_color(AnnotationColor::Blue));
    assert!(annotations.set_stroke_width(7.0));
    assert!(annotations.set_filled(true));

    annotations.set_tool(Tool::Line);
    assert_eq!(annotations.current_style().unwrap().stroke_width, 3.0);
    assert!(!annotations.set_filled(true));
    assert!(annotations.set_color(AnnotationColor::Red));

    annotations.set_tool(Tool::Rectangle);
    assert_eq!(
        annotations.current_style(),
        Some(AnnotationStyle {
            color: AnnotationColor::Blue,
            stroke_width: 7.0,
            filled: true,
        }),
    );
}

#[test]
fn newly_created_shape_remains_the_style_target() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Oval);
    annotations.set_color(AnnotationColor::Green);
    annotations.set_stroke_width(5.0);
    annotations.set_filled(true);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(200.0, 150.0), false);
    assert!(annotations.set_color(AnnotationColor::Yellow));
    assert!(annotations.set_stroke_width(8.0));
    assert!(annotations.set_filled(false));
    assert_eq!(
        annotations.shapes().next().unwrap().color,
        AnnotationColor::Yellow
    );
    assert_eq!(annotations.shapes().next().unwrap().stroke_width, 8.0);
    assert!(!annotations.shapes().next().unwrap().filled);

    annotations.begin(MOUSE, Point::new(300.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(400.0, 150.0), false);
    assert_eq!(
        annotations.shapes().nth(1).unwrap().style(),
        annotations.shapes().next().unwrap().style()
    );
}

#[test]
fn selected_style_changes_are_undoable_and_validate_thickness() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(200.0, 150.0), false);
    let original = annotations.shapes().next().unwrap().clone();
    annotations.set_tool(Tool::Select);

    assert!(annotations.set_color(AnnotationColor::White));
    assert_eq!(
        annotations.shapes().next().unwrap().color,
        AnnotationColor::White
    );
    assert!(annotations.undo());
    assert_eq!(annotations.shapes().next().unwrap().clone(), original);

    assert!(!annotations.set_stroke_width(0.0));
    assert!(!annotations.set_stroke_width(21.0));
    assert!(!annotations.set_stroke_width(f32::NAN));
    assert_eq!(annotations.shapes().next().unwrap().clone(), original);
}
