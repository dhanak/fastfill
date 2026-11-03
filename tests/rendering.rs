use fastfill::annotations::{
    AnnotationColor, AnnotationGeometry, AnnotationRef, AnnotationStyle,
    InkKind, LineKind, ShapeAnnotation, ShapeKind, TextAnnotation, TextFormat,
};
use fastfill::geometry::{Point, Rect};
use fastfill::rendering::{
    RenderAnnotation, SceneLineCap, SceneLineJoin, layout_annotation,
};

#[test]
fn unfilled_rectangle_layout_insets_its_stroke_inside_the_frame() {
    let shape = bounded_shape(ShapeKind::Rectangle, false);
    let RenderAnnotation::Paths(paths) =
        layout_annotation(AnnotationRef::Shape(0, &shape))
    else {
        panic!("rectangle must produce paths");
    };

    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].fill, None);
    assert_eq!(paths[0].stroke.as_ref().unwrap().width, 4.0);
    assert_eq!(paths[0].subpaths.len(), 1);
    assert!(paths[0].subpaths[0].closed);
    assert_eq!(
        paths[0].subpaths[0].points,
        [
            Point::new(12.0, 22.0),
            Point::new(108.0, 22.0),
            Point::new(108.0, 98.0),
            Point::new(12.0, 98.0),
        ],
    );
}

#[test]
fn arrows_and_filled_ink_expose_complete_backend_neutral_paths() {
    let arrow = ShapeAnnotation {
        id: uuid::Uuid::new_v4(),
        geometry: AnnotationGeometry::Linear {
            kind: LineKind::Arrow,
            start: Point::new(10.0, 20.0),
            end: Point::new(110.0, 20.0),
        },
        stroke_width: 3.0,
        color: AnnotationColor::Blue,
        filled: false,
        rotation: 0.0,
    };
    let RenderAnnotation::Paths(arrow_paths) =
        layout_annotation(AnnotationRef::Shape(0, &arrow))
    else {
        panic!("arrow must produce paths");
    };
    let arrow_path = &arrow_paths[0];
    assert_eq!(arrow_path.subpaths.len(), 2);
    assert_eq!(
        arrow_path.subpaths[0].points,
        [Point::new(10.0, 20.0), Point::new(110.0, 20.0),]
    );
    assert_eq!(arrow_path.subpaths[1].points.len(), 3);
    assert_eq!(
        arrow_path.stroke.as_ref().unwrap().join,
        SceneLineJoin::Round
    );

    let ink = ShapeAnnotation {
        id: uuid::Uuid::new_v4(),
        geometry: AnnotationGeometry::Ink {
            kind: InkKind::Pen,
            points: vec![
                Point::new(10.0, 10.0),
                Point::new(30.0, 10.0),
                Point::new(20.0, 30.0),
            ],
        },
        stroke_width: 6.0,
        color: AnnotationColor::Red,
        filled: true,
        rotation: 0.0,
    };
    let RenderAnnotation::Paths(ink_paths) =
        layout_annotation(AnnotationRef::Shape(0, &ink))
    else {
        panic!("ink must produce paths");
    };
    assert!(ink_paths[0].fill.is_some());
    assert!(ink_paths[0].subpaths[0].closed);
    assert_eq!(
        ink_paths[0].stroke.as_ref().unwrap().cap,
        SceneLineCap::Round
    );
}

#[test]
fn mark_layout_fills_its_inset_frame() {
    let check = bounded_shape(ShapeKind::Checkmark, false);
    let RenderAnnotation::Paths(paths) =
        layout_annotation(AnnotationRef::Shape(0, &check))
    else {
        panic!("checkmark must produce paths");
    };
    assert_eq!(
        paths[0].subpaths[0].points,
        [
            Point::new(12.0, 56.2),
            Point::new(46.56, 98.0),
            Point::new(108.0, 22.0),
        ],
    );
}

#[test]
fn thick_marks_stop_growing_before_their_arms_collapse() {
    for kind in [ShapeKind::Checkmark, ShapeKind::Cross] {
        for side in [1.0, 6.0, 19.125, 80.0] {
            let mut shape = bounded_shape(kind, false);
            shape.geometry = AnnotationGeometry::Bounded {
                kind,
                bounds: Rect::new(10.0, 20.0, side, side),
            };
            let mut previous_width = 0.0;
            let mut previous = None;
            for requested in 1_u16..=20 {
                shape.stroke_width = f32::from(requested);
                let RenderAnnotation::Paths(paths) =
                    layout_annotation(AnnotationRef::Shape(0, &shape))
                else {
                    panic!("mark must produce paths");
                };
                assert_eq!(paths.len(), 1);
                let width = paths[0].stroke.unwrap().width;
                assert!(width >= previous_width);
                assert!(width <= side / 3.0);
                for path in &paths[0].subpaths {
                    assert!(path.points.len() >= 2);
                    for point in &path.points {
                        assert!(point.x >= 10.0 + width / 2.0 - 0.0001);
                        assert!(point.x <= 10.0 + side - width / 2.0 + 0.0001);
                        assert!(point.y >= 20.0 + width / 2.0 - 0.0001);
                        assert!(point.y <= 20.0 + side - width / 2.0 + 0.0001);
                    }
                }
                if (width - previous_width).abs() < f32::EPSILON {
                    assert_eq!(Some(&paths), previous.as_ref());
                }
                previous_width = width;
                previous = Some(paths);
            }
        }
    }
}

#[test]
fn text_layout_preserves_the_measured_model_object() {
    let text = TextAnnotation {
        id: uuid::Uuid::new_v4(),
        text: "Shared layout".into(),
        bounds: Rect::new(20.0, 30.0, 140.0, 31.2),
        format: TextFormat::default(),
        rotation: 0.25,
    };

    let RenderAnnotation::Text(layout) =
        layout_annotation(AnnotationRef::Text(0, &text))
    else {
        panic!("text must remain text");
    };
    assert_eq!(layout, &text);
}

#[test]
fn custom_alpha_applies_once_and_combines_with_highlighter_opacity() {
    let mut shape = bounded_shape(ShapeKind::Rectangle, true);
    shape.color = AnnotationColor::from_argb(0x8000_66cc);
    let RenderAnnotation::Paths(paths) =
        layout_annotation(AnnotationRef::Shape(0, &shape))
    else {
        panic!("rectangle must produce paths");
    };
    let opacity = 128.0 / 255.0;
    assert!((paths[0].fill.unwrap().opacity - opacity).abs() < f32::EPSILON);
    shape.geometry = AnnotationGeometry::Ink {
        kind: InkKind::Highlighter,
        points: vec![Point::new(10.0, 20.0), Point::new(110.0, 20.0)],
    };
    shape.filled = false;
    let RenderAnnotation::Paths(paths) =
        layout_annotation(AnnotationRef::Shape(0, &shape))
    else {
        panic!("highlighter must produce paths");
    };
    assert!(
        (paths[0].stroke.as_ref().unwrap().paint.opacity - opacity * 0.35)
            .abs()
            < f32::EPSILON
    );
}

fn bounded_shape(kind: ShapeKind, filled: bool) -> ShapeAnnotation {
    let style = AnnotationStyle {
        color: AnnotationColor::Black,
        stroke_width: 4.0,
        filled,
    };
    ShapeAnnotation {
        id: uuid::Uuid::new_v4(),
        geometry: AnnotationGeometry::Bounded {
            kind,
            bounds: Rect::new(10.0, 20.0, 100.0, 80.0),
        },
        stroke_width: style.stroke_width,
        color: style.color,
        filled: style.filled,
        rotation: 0.0,
    }
}
