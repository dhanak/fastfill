use super::*;
use fastfill::annotations::{AnnotationColor, Pointer, TextFormat, Tool};
use fastfill::geometry::Point;

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
        let result = iced::futures::executor::block_on(export_pdf(
            pages,
            true,
            async |_, _, _| panic!("invalid document must not begin rendering"),
        ));
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
        true,
        async |done, total, stage| {
            progress.push((done, total, stage));
            true
        },
    ))
    .unwrap();
    assert_eq!(
        progress,
        [
            (0, 2, PdfStage::Rasterizing),
            (0, 2, PdfStage::Encoding),
            (1, 2, PdfStage::Ready),
            (1, 2, PdfStage::Rasterizing),
            (1, 2, PdfStage::Encoding),
            (2, 2, PdfStage::Ready),
        ]
    );
    let parsed = hayro::hayro_syntax::Pdf::new(pdf.clone()).unwrap();
    assert_eq!(parsed.pages()[0].render_dimensions(), (72.0, 144.0));
    assert_eq!(parsed.pages()[1].render_dimensions(), (144.0, 72.0));
    let pages = fastfill::pdf_import::import_pdf(pdf, 20, |_, _| true).unwrap();
    assert_eq!(pages[0].pixels.dimensions(), (300, 600));
    assert_eq!(pages[1].pixels.dimensions(), (600, 300));
    assert_eq!(pages[0].pixels.get_pixel(150, 300).0, [220, 32, 32, 255]);
    assert_eq!(pages[1].pixels.get_pixel(300, 150).0, [255; 4]);
}

#[test]
fn export_cancellation_discards_the_result_at_every_page_boundary() {
    for cancel_at in 0..=2 {
        let pages = (0..2).map(|_| blank(PageSize::new(72.0, 72.0))).collect();
        let result = iced::futures::executor::block_on(export_pdf(
            pages,
            true,
            async |done, _, _| done < cancel_at,
        ));
        assert!(result.unwrap_err().contains("canceled"));
    }
}

#[test]
fn export_waits_for_progress_and_honors_cancel_during_the_wait() {
    use std::cell::{Cell, RefCell};
    use std::future::{Future, poll_fn};
    use std::task::{Context, Poll, Waker};

    let checkpoints = [
        (0, 2, PdfStage::Rasterizing),
        (0, 2, PdfStage::Encoding),
        (1, 2, PdfStage::Ready),
        (1, 2, PdfStage::Rasterizing),
        (1, 2, PdfStage::Encoding),
        (2, 2, PdfStage::Ready),
    ];
    for cancel_at in 0..checkpoints.len() {
        let canceled = Cell::new(false);
        let observed = RefCell::new(Vec::new());
        let pages = (0..2).map(|_| blank(PageSize::new(72.0, 72.0))).collect();
        let future = export_pdf(pages, true, async |done, total, stage| {
            observed.borrow_mut().push((done, total, stage));
            let mut yielded = false;
            poll_fn(|context| {
                if std::mem::replace(&mut yielded, true) {
                    Poll::Ready(())
                } else {
                    context.waker().wake_by_ref();
                    Poll::Pending
                }
            })
            .await;
            !canceled.get()
        });
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Pending => {
                    let count = observed.borrow().len();
                    assert_eq!(*observed.borrow(), checkpoints[..count]);
                    canceled.set(count == cancel_at + 1);
                }
                Poll::Ready(result) => {
                    assert!(result.unwrap_err().contains("canceled"));
                    assert_eq!(
                        observed.borrow().last(),
                        Some(&checkpoints[cancel_at])
                    );
                    assert_eq!(observed.borrow().len(), cancel_at + 1);
                    break;
                }
            }
        }
    }
}

#[test]
fn export_filters_scan_rows_losslessly_in_gray_and_color() {
    for color in [false, true] {
        let source = image::RgbaImage::from_fn(300, 400, |x, y| {
            #[allow(clippy::cast_possible_truncation)]
            let value =
                (x.wrapping_mul(73) ^ (x / 7)).wrapping_add(y / 2) as u8;
            #[allow(clippy::cast_possible_truncation)]
            let pixel = if color {
                [
                    value,
                    value.wrapping_add((y % 11) as u8),
                    value.wrapping_add((x % 17) as u8),
                    255,
                ]
            } else {
                [value, value, value, 255]
            };
            image::Rgba(pixel)
        });
        let size = PageSize::new(72.0, 96.0);
        let mut annotations = Annotations::default();
        annotations.set_tool(Tool::Text);
        annotations.begin_text(Point::new(5.0, 5.0), size, 1.0);
        annotations.update_text("1234", PageSize::new(60.0, 30.0));
        annotations.accept_text();
        let page = ExportPage::new(
            size,
            Some(Handle::from_rgba(300, 400, source.into_raw())),
            &annotations,
        );
        let expected =
            rasterize(&mut software_renderer(), &page, (300, 400)).unwrap();
        let previous = unfiltered_pdf(&expected, size, (300, 400));
        let pdf = iced::futures::executor::block_on(export_pdf(
            vec![page],
            true,
            async |_, _, _| true,
        ))
        .unwrap();
        assert!(
            pdf.len() * 2 < previous.len(),
            "filtered={}, previous={}, color={color}",
            pdf.len(),
            previous.len(),
        );
        let pages =
            fastfill::pdf_import::import_pdf(pdf, 1, |_, _| true).unwrap();
        let error: u64 = pages[0]
            .pixels
            .as_raw()
            .iter()
            .zip(&expected)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        assert!(error < expected.len() as u64, "channel error={error}");
    }
}

fn unfiltered_pdf(
    pixels: &[u8],
    size: PageSize,
    (w, h): (u32, u32),
) -> Vec<u8> {
    let mut document = krilla::Document::new();
    let mut page = document.start_page_with(
        krilla::page::PageSettings::from_wh(size.width, size.height).unwrap(),
    );
    let mut surface = page.surface();
    surface.draw_image(
        krilla::image::Image::from_rgba8(pixels.to_vec(), w, h),
        krilla::geom::Size::from_wh(size.width, size.height).unwrap(),
    );
    surface.finish();
    page.finish();
    document.finish().unwrap()
}

#[test]
fn compact_export_uses_high_quality_jpeg_for_scans_and_png_for_line_art() {
    let mut pixels = image::load_from_memory(include_bytes!(
        "../../../tests/samples/datum-form.png"
    ))
    .unwrap()
    .resize_exact(600, 800, image::imageops::FilterType::Triangle)
    .into_rgba8();
    // Deterministic scanner noise: clean line art should prefer PNG instead.
    let mut noise = 1_u32;
    for pixel in pixels.pixels_mut() {
        noise = noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let shade = (noise >> 28) as u8;
        for channel in &mut pixel.0[..3] {
            *channel = channel.saturating_sub(shade);
        }
    }
    let handle = Handle::from_rgba(600, 800, pixels.into_raw());
    let size = PageSize::new(144.0, 192.0);
    let mut annotations = Annotations::default();
    annotations.set_tool(Tool::Text);
    annotations.begin_text(Point::new(5.0, 5.0), size, 1.0);
    annotations.update_text("1234", PageSize::new(100.0, 30.0));
    annotations.accept_text();
    let page = || ExportPage::new(size, Some(handle.clone()), &annotations);
    let lossless = iced::futures::executor::block_on(export_pdf(
        vec![page()],
        true,
        async |_, _, _| true,
    ))
    .unwrap();
    let compact = iced::futures::executor::block_on(export_pdf(
        vec![page()],
        false,
        async |_, _, _| true,
    ))
    .unwrap();
    assert!(compact.windows(10).any(|s| s == b"/DCTDecode"));
    assert!(
        compact.len() < lossless.len(),
        "compact={}, lossless={}",
        compact.len(),
        lossless.len()
    );
    let expected =
        fastfill::pdf_import::import_pdf(lossless, 1, |_, _| true).unwrap();
    let actual =
        fastfill::pdf_import::import_pdf(compact, 1, |_, _| true).unwrap();
    assert_eq!(
        expected[0].pixels.dimensions(),
        actual[0].pixels.dimensions()
    );
    let mut error = 0_u64;
    let mut black = 0;
    let mut retained = 0;
    for (expected, actual) in
        expected[0].pixels.pixels().zip(actual[0].pixels.pixels())
    {
        for (a, b) in expected.0.iter().zip(actual.0) {
            error += u64::from(a.abs_diff(b));
        }
        if expected.0[..3].iter().all(|v| *v < 32) {
            black += 1;
            retained += usize::from(actual.0[..3].iter().all(|v| *v < 64));
        }
    }
    assert!(
        error < expected[0].pixels.as_raw().len() as u64 * 2,
        "average channel error must stay below 2: {error}"
    );
    assert!(
        black > 500 && retained * 100 >= black * 99,
        "small black text must remain legible: {retained}/{black}"
    );
    let (_, gray, filter) =
        image_stream(vec![255; 64 * 64 * 4], (64, 64), false).unwrap();
    assert!(gray && matches!(filter, Filter::FlateDecode));
}

#[test]
#[ignore = "Explicit real-document import/export size and timing probe"]
fn real_document_export_size_and_timing() {
    let path = std::env::var_os("FASTFILL_PDF_INPUT").expect("PDF input path");
    let original = std::fs::read(path).unwrap();
    let started = std::time::Instant::now();
    let pages =
        fastfill::pdf_import::import_pdf(original.clone(), 20, |_, _| true)
            .unwrap();
    let imported = started.elapsed();
    let pages: Vec<_> = pages
        .into_iter()
        .map(|page| {
            #[allow(clippy::cast_precision_loss)]
            let size = PageSize::new(
                612.0,
                612.0 * page.info.height as f32 / page.info.width as f32,
            );
            ExportPage::new(
                size,
                Some(Handle::from_rgba(
                    page.info.width,
                    page.info.height,
                    page.pixels.into_raw(),
                )),
                &Annotations::default(),
            )
        })
        .collect();
    let compact_pages = pages
        .iter()
        .map(|page| ExportPage {
            size: page.size,
            image: page.image.clone(),
            annotations: page.annotations.clone(),
        })
        .collect();
    let started = std::time::Instant::now();
    let lossless = iced::futures::executor::block_on(export_pdf(
        pages,
        true,
        async |_, _, _| true,
    ))
    .unwrap();
    let encoded = started.elapsed();
    let started = std::time::Instant::now();
    let pdf = iced::futures::executor::block_on(export_pdf(
        compact_pages,
        false,
        async |_, _, _| true,
    ))
    .unwrap();
    eprintln!(
        "original={} lossless={} compact={} import={imported:?} \
         lossless_time={encoded:?} compact_time={:?}",
        original.len(),
        lossless.len(),
        pdf.len(),
        started.elapsed(),
    );
    if let Some(path) = std::env::var_os("FASTFILL_PDF_OUTPUT") {
        use std::io::Write as _;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&pdf)
            .unwrap();
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
fn page_clipping_preserves_visible_parts_of_edge_strokes() {
    use fastfill::annotations::{
        AnnotationGeometry, LineKind, ShapeAnnotation,
    };
    let mut page = blank(PageSize::new(144.0, 144.0));
    for (start, end) in [
        (Point::new(10.0, 2.0), Point::new(130.0, 2.0)),
        (Point::new(-2.0, 20.0), Point::new(-2.0, 100.0)),
        (Point::new(-10.0, 120.0), Point::new(30.0, 120.0)),
    ] {
        page.annotations
            .push(OwnedAnnotation::Shape(ShapeAnnotation {
                id: uuid::Uuid::new_v4(),
                geometry: AnnotationGeometry::Linear {
                    kind: LineKind::Line,
                    start,
                    end,
                },
                stroke_width: 10.0,
                color: AnnotationColor::Black,
                filled: false,
                rotation: 0.0,
            }));
    }
    let pixels =
        rasterize(&mut software_renderer(), &page, (600, 600)).unwrap();
    let pixel = |x: usize, y: usize| &pixels[(y * 600 + x) * 4..][..4];
    for (x, y) in [(200, 1), (200, 20), (1, 200), (0, 500), (100, 500)] {
        assert_eq!(pixel(x, y), [0, 0, 0, 255], "at {x},{y}");
    }
    for (x, y) in [(200, 40), (20, 200), (140, 500)] {
        assert_eq!(pixel(x, y), [255; 4], "at {x},{y}");
    }
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
    use fastfill::annotations::{
        AnnotationGeometry, InkKind, ShapeAnnotation, ShapeKind, TextAnnotation,
    };
    use fastfill::geometry::Rect;

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
    let pdf = iced::futures::executor::block_on(export_pdf(
        vec![page],
        true,
        async |_, _, _| true,
    ))
    .unwrap();
    if let Some(path) = std::env::var_os("FASTFILL_PDF_EXPORT_SAMPLE") {
        std::fs::write(path, &pdf).unwrap();
    }
    let pages = fastfill::pdf_import::import_pdf(pdf, 20, |_, _| true).unwrap();
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
    use fastfill::annotations::DragModifiers;
    use fastfill::geometry::Navigation;
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
