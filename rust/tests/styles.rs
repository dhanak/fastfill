use fastfill_rust::annotations::{
    AnnotationColor, AnnotationStyle, Annotations, Pointer, Tool,
};
use fastfill_rust::geometry::{Point, Size};

const PAGE: Size = Size::new(800.0, 1_100.0);
const MOUSE: Pointer = Pointer::Mouse;

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
    assert_eq!(annotations.shapes()[0].color, AnnotationColor::Yellow);
    assert_eq!(annotations.shapes()[0].stroke_width, 8.0);
    assert!(!annotations.shapes()[0].filled);

    annotations.begin(MOUSE, Point::new(300.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(400.0, 150.0), false);
    assert_eq!(
        annotations.shapes()[1].style(),
        annotations.shapes()[0].style()
    );
}

#[test]
fn selected_style_changes_are_undoable_and_validate_thickness() {
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(MOUSE, Point::new(100.0, 100.0), PAGE);
    annotations.finish(MOUSE, Point::new(200.0, 150.0), false);
    let original = annotations.shapes()[0].clone();
    annotations.set_tool(Tool::Select);

    assert!(annotations.set_color(AnnotationColor::White));
    assert_eq!(annotations.shapes()[0].color, AnnotationColor::White);
    assert!(annotations.undo());
    assert_eq!(annotations.shapes()[0], original);

    assert!(!annotations.set_stroke_width(0.0));
    assert!(!annotations.set_stroke_width(21.0));
    assert!(!annotations.set_stroke_width(f32::NAN));
    assert_eq!(annotations.shapes()[0], original);
}
