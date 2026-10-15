use super::*;
use fastfill_rust::annotations::{AnnotationColor, Pointer, TextFormat, Tool};
use fastfill_rust::geometry::Point;

fn blank(size: PageSize) -> ExportPage {
    ExportPage::new(size, None, &Annotations::default())
}

#[test]
fn export_preflights_the_entire_document_before_rendering() {
    for pages in [
        vec![],
        (0..21).map(|_| blank(PageSize::new(72.0, 72.0))).collect(),
        vec![blank(PageSize::new(72.0, f32::NAN))],
        vec![blank(PageSize::new(10_000.0, 10_000.0))],
        (0..12)
            .map(|_| blank(PageSize::new(1_000.0, 1_000.0)))
            .collect(),
    ] {
        let result =
            iced::futures::executor::block_on(export_pdf(pages, |_, _| {
                panic!("invalid document must not begin rendering")
            }));
        assert!(result.is_err());
    }
}

#[test]
fn export_round_trips_page_order_dimensions_and_white_background() {
    let mut first = blank(PageSize::new(72.0, 144.0));
    first.image = Some(Handle::from_rgba(1, 1, vec![220, 32, 32, 255]));
    let second = blank(PageSize::new(144.0, 72.0));
    let mut progress = Vec::new();
    let pdf = iced::futures::executor::block_on(export_pdf(
        vec![first, second],
        |done, total| {
            progress.push((done, total));
            true
        },
    ))
    .unwrap();
    assert_eq!(progress, [(0, 2), (1, 2), (2, 2)]);
    let parsed = hayro::hayro_syntax::Pdf::new(pdf.clone()).unwrap();
    assert_eq!(parsed.pages()[0].render_dimensions(), (72.0, 144.0));
    assert_eq!(parsed.pages()[1].render_dimensions(), (144.0, 72.0));
    let pages =
        fastfill_rust::pdf_import::import_pdf(pdf, 20, |_, _| true).unwrap();
    assert_eq!(pages[0].pixels.dimensions(), (300, 600));
    assert_eq!(pages[1].pixels.dimensions(), (600, 300));
    assert_eq!(pages[0].pixels.get_pixel(150, 300).0, [220, 32, 32, 255]);
    assert_eq!(pages[1].pixels.get_pixel(300, 150).0, [255; 4]);
}

#[test]
fn export_cancellation_discards_the_result_at_every_page_boundary() {
    for cancel_at in 0..=2 {
        let pages = (0..2).map(|_| blank(PageSize::new(72.0, 72.0))).collect();
        let result =
            iced::futures::executor::block_on(export_pdf(pages, |done, _| {
                done < cancel_at
            }));
        assert!(result.unwrap_err().contains("canceled"));
    }
}

#[test]
fn export_uses_committed_layers_and_excludes_selection_and_drafts() {
    let size = PageSize::new(144.0, 144.0);
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Rectangle);
    annotations.set_color(AnnotationColor::Blue);
    annotations.set_filled(true);
    annotations.begin(Pointer::Mouse, Point::new(10.0, 10.0), size);
    annotations.finish(Pointer::Mouse, Point::new(100.0, 100.0), false);
    let committed = ExportPage::new(size, None, &annotations);
    // Selected object's handles and an unfinished draft must not leak out.
    annotations.begin(Pointer::Mouse, Point::new(105.0, 105.0), size);
    annotations.drag(Pointer::Mouse, Point::new(140.0, 140.0), false);
    let draft = ExportPage::new(size, None, &annotations);
    let mut renderer = software_renderer();
    let before = rasterize(&mut renderer, &committed, (600, 600)).unwrap();
    let after = rasterize(&mut renderer, &draft, (600, 600)).unwrap();
    assert_eq!(before, after);
    let pixel = |x: usize, y: usize| &before[(y * 600 + x) * 4..][..4];
    assert_eq!(pixel(200, 200), [0, 102, 204, 255]);
    assert_eq!(pixel(500, 500), [255; 4]);
}

#[test]
fn flattened_formatted_text_remains_visible_and_obeys_layer_order() {
    let size = PageSize::new(144.0, 144.0);
    let mut annotations = Annotations::default();
    let text = "Bold italic\nunderlined";
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(10.0, 10.0), size, 1.0);
    let format = TextFormat {
        bold: true,
        italic: true,
        underline: true,
        color: AnnotationColor::Red,
        ..TextFormat::default()
    };
    annotations.set_text_format(format);
    annotations.update_text(text, PageSize::new(180.0, 80.0));
    annotations.accept_text();
    let text_only = ExportPage::new(size, None, &annotations);
    annotations.set_tool(Tool::Rectangle);
    annotations.set_color(AnnotationColor::Yellow);
    annotations.set_filled(true);
    annotations.begin(Pointer::Mouse, Point::new(50.0, 10.0), size);
    annotations.finish(Pointer::Mouse, Point::new(140.0, 130.0), false);
    let covered = ExportPage::new(size, None, &annotations);
    let mut renderer = software_renderer();
    let pixels = rasterize(&mut renderer, &text_only, (600, 600)).unwrap();
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] > 150 && p[1] < 100)
            .count()
            > 500,
        "formatted red text must remain visible"
    );
    let pixels = rasterize(&mut renderer, &covered, (600, 600)).unwrap();
    let preview = canvas_reference(&mut renderer, size, &annotations);
    let difference: u64 = pixels
        .iter()
        .zip(preview)
        .map(|(a, b)| u64::from(a.abs_diff(b)))
        .sum();
    assert!(
        difference < u64::try_from(pixels.len()).unwrap(),
        "export must match the actual page canvas: {difference}"
    );
    for y in 50..520 {
        for x in 215..570 {
            assert_eq!(
                &pixels[(y * 600 + x) * 4..][..4],
                [245, 196, 0, 255],
                "shape must cover the older text"
            );
        }
    }
}

#[test]
fn lossless_pdf_round_trip_preserves_rotated_text_alpha_and_edge_clipping() {
    use fastfill_rust::annotations::{
        AnnotationGeometry, InkKind, ShapeAnnotation, ShapeKind, TextAnnotation,
    };
    use fastfill_rust::geometry::Rect;

    let mut page = blank(PageSize::new(144.0, 144.0));
    page.annotations = vec![
        OwnedAnnotation::Shape(ShapeAnnotation {
            id: uuid::Uuid::new_v4(),
            geometry: AnnotationGeometry::Bounded {
                kind: ShapeKind::Oval,
                bounds: Rect::new(-20.0, 10.0, 130.0, 100.0),
            },
            stroke_width: 6.0,
            color: AnnotationColor::Argb(0x8000_66cc),
            filled: true,
            rotation: 0.3,
        }),
        OwnedAnnotation::Text(TextAnnotation {
            id: uuid::Uuid::new_v4(),
            text: "Rotated\nÁrvíztűrő".into(),
            bounds: Rect::new(25.0, 35.0, 110.0, 55.0),
            format: TextFormat {
                font_size: 18.0,
                bold: true,
                italic: true,
                underline: true,
                color: AnnotationColor::Red,
                ..TextFormat::default()
            },
            rotation: 0.4,
        }),
        OwnedAnnotation::Shape(ShapeAnnotation {
            id: uuid::Uuid::new_v4(),
            geometry: AnnotationGeometry::Ink {
                kind: InkKind::Highlighter,
                points: vec![Point::new(5.0, 60.0), Point::new(140.0, 60.0)],
            },
            stroke_width: 12.0,
            color: AnnotationColor::Yellow,
            filled: false,
            rotation: 0.0,
        }),
    ];
    let expected =
        rasterize(&mut software_renderer(), &page, (600, 600)).unwrap();
    let pdf =
        iced::futures::executor::block_on(export_pdf(vec![page], |_, _| true))
            .unwrap();
    if let Some(path) = std::env::var_os("FASTFILL_PDF_EXPORT_SAMPLE") {
        std::fs::write(path, &pdf).unwrap();
    }
    let pages =
        fastfill_rust::pdf_import::import_pdf(pdf, 20, |_, _| true).unwrap();
    let actual = pages[0].pixels.as_raw();
    assert_eq!(expected.len(), actual.len());
    let total_error: u64 = expected
        .iter()
        .zip(actual)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    assert!(
        total_error < u64::try_from(expected.len()).unwrap(),
        "round-trip average channel error must stay below 1: {total_error}"
    );
    assert_eq!(pages[0].pixels.get_pixel(599, 599).0, [255; 4]);
}

fn software_renderer() -> iced::Renderer {
    iced::futures::executor::block_on(new_renderer()).unwrap()
}

fn canvas_reference(
    renderer: &mut Renderer,
    size: PageSize,
    annotations: &Annotations,
) -> Vec<u8> {
    use super::super::{CanvasLayer, CanvasState, PageCanvas};
    use fastfill_rust::annotations::DragModifiers;
    use fastfill_rust::geometry::Navigation;
    use iced::widget::canvas::Program;

    // 144-point page plus the UI's 36-point margin on each side. Rendering
    // the real canvas at the same DPI catches shaping/placement divergence.
    let bounds =
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(216.0, 216.0));
    renderer.reset(bounds);
    let canvas = PageCanvas {
        image: None,
        navigation: Navigation::default(),
        page_size: size,
        annotations,
        input_epoch: 0,
        input_enabled: false,
        modifiers: DragModifiers::default(),
        layer: CanvasLayer::Background,
    };
    for layer in std::iter::once(CanvasLayer::Background).chain(
        annotations
            .ordered_annotations()
            .map(CanvasLayer::Annotation),
    ) {
        renderer.with_layer(bounds, |renderer| {
            for geometry in (PageCanvas { layer, ..canvas }).draw(
                &CanvasState::default(),
                renderer,
                &iced::Theme::Light,
                bounds,
                iced::mouse::Cursor::Unavailable,
            ) {
                renderer.draw_geometry(geometry);
            }
        });
    }
    let pixels = renderer.screenshot(
        iced::Size::new(900, 900),
        300.0 / 72.0,
        Color::WHITE,
    );
    let image = image::RgbaImage::from_raw(900, 900, pixels).unwrap();
    image::imageops::crop_imm(&image, 150, 150, 600, 600)
        .to_image()
        .into_raw()
}
