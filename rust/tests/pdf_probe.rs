#![cfg(feature = "pdf")]

use fastfill_rust::annotations::{
    AnnotationColor, AnnotationGeometry, AnnotationRef, InkKind,
    ShapeAnnotation, ShapeKind, TextAnnotation, TextFormat, TextTypeface,
};
use fastfill_rust::geometry::{Point, Rect, Size};
use fastfill_rust::pdf_probe::{PdfProbeError, render_page};

#[test]
fn vector_annotations_produce_one_pdf_page() {
    let rectangle = ShapeAnnotation {
        geometry: AnnotationGeometry::Bounded {
            kind: ShapeKind::Rectangle,
            bounds: Rect::new(10.0, 20.0, 100.0, 80.0),
        },
        stroke_width: 4.0,
        color: AnnotationColor::Blue,
        filled: true,
        rotation: 0.25,
    };
    let highlighter = ShapeAnnotation {
        geometry: AnnotationGeometry::Ink {
            kind: InkKind::Highlighter,
            points: vec![Point::new(20.0, 130.0), Point::new(100.0, 150.0)],
        },
        stroke_width: 12.0,
        color: AnnotationColor::Yellow,
        filled: false,
        rotation: 0.0,
    };

    let pdf = render_page(
        Size::new(200.0, 300.0),
        [
            AnnotationRef::Shape(0, &rectangle),
            AnnotationRef::Shape(1, &highlighter),
        ],
    )
    .expect("vector scene should serialize");

    assert!(pdf.starts_with(b"%PDF-"));
    assert!(pdf.windows(5).any(|window| window == b"%%EOF"));
}

#[test]
fn text_reports_that_exact_output_requires_flattening() {
    let text = TextAnnotation {
        text: "Rotated formatted text".into(),
        bounds: Rect::new(20.0, 30.0, 180.0, 40.0),
        format: TextFormat {
            typeface: TextTypeface::Serif,
            font_size: 32.0,
            bold: true,
            italic: true,
            underline: true,
            color: AnnotationColor::Red,
        },
        rotation: 0.5,
    };

    let error =
        render_page(Size::new(200.0, 300.0), [AnnotationRef::Text(0, &text)])
            .expect_err("probe must not silently reshape text");

    assert_eq!(error, PdfProbeError::TextRequiresFlattening);
}

#[test]
fn invalid_page_size_is_rejected() {
    let annotations = std::iter::empty::<AnnotationRef<'static>>();

    assert_eq!(
        render_page(Size::new(0.0, 300.0), annotations),
        Err(PdfProbeError::InvalidPageSize),
    );
}
