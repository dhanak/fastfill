#![cfg(feature = "pdf-import")]

use fastfill_rust::pdf_import::{
    MAX_PDF_BYTES, PdfStage, import_pdf, import_pdf_with_progress,
    import_pdf_with_progress_limited, raster_size, validate_pdf_length,
};
use std::fmt::Write;

#[test]
fn input_and_raster_limits_reject_invalid_values_before_allocation() {
    assert!(validate_pdf_length(0).is_err());
    assert!(validate_pdf_length(MAX_PDF_BYTES + 1).is_err());
    assert!(validate_pdf_length(1).is_ok());
    assert_eq!(raster_size(612.0, 792.0).unwrap(), (2_550, 3_300));
    assert_eq!(raster_size(72.01, 144.01).unwrap(), (301, 601));
    for (width, height) in [
        (0.0, 72.0),
        (-1.0, 72.0),
        (f32::NAN, 72.0),
        (72.0, f32::INFINITY),
        (1e8, 72.0),
        (10_000.0, 10_000.0),
    ] {
        assert!(raster_size(width, height).is_err());
    }
}

#[test]
fn reports_work_stages_and_returns_the_pixels_used_for_encoding() {
    let mut stages = Vec::new();
    let pages = import_pdf_with_progress(
        pdf(&[(72.0, 72.0, "", "")]),
        20,
        |done, total, stage| {
            stages.push((done, total, stage));
            true
        },
    )
    .unwrap();
    assert_eq!(
        stages,
        [
            (0, 0, PdfStage::Parsing),
            (0, 1, PdfStage::Ready),
            (0, 1, PdfStage::Interpreting),
            (0, 1, PdfStage::Rasterizing),
            (0, 1, PdfStage::Encoding),
            (1, 1, PdfStage::Ready),
        ]
    );
    let decoded = image::load_from_memory(&pages[0].bytes)
        .unwrap()
        .into_rgba8();
    assert_eq!(decoded, pages[0].pixels);
    assert!(
        import_pdf_with_progress(
            pdf(&[(72.0, 72.0, "", "")]),
            20,
            |_, _, stage| stage != PdfStage::Rasterizing
        )
        .is_err()
    );
}

#[test]
fn rejects_corrupt_empty_and_over_capacity_documents() {
    assert!(import_pdf(b"not a PDF".to_vec(), 20, |_, _| true).is_err());
    assert!(import_pdf(pdf(&[]), 20, |_, _| true).is_err());
    let pages = [(72.0, 72.0, "", ""); 21];
    let error = import_pdf(pdf(&pages), 20, |_, _| true).unwrap_err();
    assert!(error.to_string().contains("room"));
    assert!(import_pdf(pdf(&pages[..1]), 0, |_, _| true).is_err());
}

#[test]
fn remaining_project_budget_is_checked_before_rasterization() {
    let error = import_pdf_with_progress_limited(
        pdf(&[(72.0, 72.0, "", "")]),
        20,
        89_999,
        |_, _, _| true,
    )
    .unwrap_err();
    assert!(error.to_string().contains("remaining project image budget"));
}

#[test]
fn imports_pages_in_order_with_white_backgrounds_at_300_dpi() {
    let source = pdf(&[
        (72.0, 144.0, "", "1 0 0 rg 0 0 36 36 re f"),
        (144.0, 72.0, "", "0 0 1 rg 0 0 36 36 re f"),
    ]);
    let pages = import_pdf(source, 20, |_, _| true).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!((pages[0].info.width, pages[0].info.height), (300, 600));
    assert_eq!((pages[1].info.width, pages[1].info.height), (600, 300));
    for (page, expected) in pages.iter().zip([[255, 0, 0], [0, 0, 255]]) {
        let image = image::load_from_memory(&page.bytes).unwrap().to_rgb8();
        assert_eq!(image.get_pixel(20, image.height() - 20).0, expected);
        assert_eq!(image.get_pixel(image.width() - 20, 20).0, [255; 3]);
    }
}

#[test]
fn honors_pdf_crop_and_rotation_without_stretching() {
    let pages = import_pdf(
        pdf(&[
            (144.0, 72.0, "/Rotate 90", "1 0 0 rg 0 0 144 72 re f"),
            (
                144.0,
                144.0,
                "/CropBox [36 36 108 108]",
                "0 1 0 rg 0 0 144 144 re f",
            ),
        ]),
        20,
        |_, _| true,
    )
    .unwrap();
    assert_eq!((pages[0].info.width, pages[0].info.height), (300, 600));
    assert_eq!((pages[1].info.width, pages[1].info.height), (300, 300));
    let image = image::load_from_memory(&pages[1].bytes).unwrap().to_rgb8();
    assert_eq!(image.get_pixel(150, 150).0, [0, 255, 0]);
}

#[test]
fn preflights_all_dimensions_before_rendering_any_page() {
    let mut progress = Vec::new();
    let result = import_pdf(
        pdf(&[(72.0, 72.0, "", ""), (1e8, 72.0, "", "")]),
        20,
        |done, total| {
            progress.push((done, total));
            true
        },
    );
    assert!(result.is_err());
    assert_eq!(progress, []);
    let result =
        import_pdf(pdf(&[(1_000.0, 1_000.0, "", ""); 12]), 20, |_, _| {
            panic!("over-budget batch must not begin rendering")
        });
    assert!(result.unwrap_err().to_string().contains("200-megapixel"));
}

#[test]
fn honors_user_units_and_rejects_invalid_crop_geometry() {
    let pages = import_pdf(
        pdf(&[(72.0, 72.0, "/UserUnit 2", "1 0 0 rg 0 0 72 72 re f")]),
        20,
        |_, _| true,
    )
    .unwrap();
    assert_eq!((pages[0].info.width, pages[0].info.height), (600, 600));
    let image = image::load_from_memory(&pages[0].bytes).unwrap().to_rgb8();
    assert_eq!(image.get_pixel(580, 580).0, [255, 0, 0]);
    for extra in ["/UserUnit 0", "/CropBox [0 0 0 72]"] {
        assert!(
            import_pdf(pdf(&[(72.0, 72.0, extra, "")]), 20, |_, _| true)
                .is_err()
        );
    }
}

#[test]
fn renderer_warnings_fail_the_whole_batch_instead_of_losing_content() {
    let result = import_pdf(
        pdf(&[
            (72.0, 72.0, "", ""),
            (
                72.0,
                72.0,
                "/Annots [<< /Subtype /Widget /Rect [0 0 36 36] /AP << >> >>]",
                "",
            ),
        ]),
        20,
        |_, _| true,
    );
    assert!(result.unwrap_err().to_string().contains("page 2"));
}

#[test]
fn cancellation_discards_the_batch_and_progress_is_monotonic() {
    let mut progress = Vec::new();
    let result =
        import_pdf(pdf(&[(72.0, 72.0, "", ""); 3]), 20, |done, total| {
            progress.push((done, total));
            done < 1
        });
    assert!(result.unwrap_err().to_string().contains("canceled"));
    assert_eq!(progress, [(0, 3), (1, 3)]);
    for cancel_at in [0, 3] {
        let result =
            import_pdf(pdf(&[(72.0, 72.0, "", ""); 3]), 20, |done, _| {
                done < cancel_at
            });
        assert!(result.unwrap_err().to_string().contains("canceled"));
    }
}

#[test]
fn standard_pdf_fonts_produce_visible_text_without_system_dependencies() {
    let pages = import_pdf(
        pdf(&[(
            144.0,
            72.0,
            "",
            "0 g BT /F1 16 Tf 10 36 Td (Printed form) Tj ET",
        )]),
        20,
        |_, _| true,
    )
    .unwrap();
    let image = image::load_from_memory(&pages[0].bytes).unwrap().to_rgb8();
    let dark = image
        .pixels()
        .filter(|p| p.0.iter().all(|c| *c < 64))
        .count();
    assert!(dark > 500, "standard font text is missing: {dark}");
}

#[test]
fn imports_the_printable_sample_form() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/samples/fastfill-camera-sample-form.pdf"
    ))
    .unwrap();
    let pages = import_pdf(bytes, 20, |_, _| true).unwrap();
    assert_eq!(pages.len(), 1);
    let image = image::load_from_memory(&pages[0].bytes).unwrap().to_rgb8();
    assert!(image.width() > 2_000 && image.height() > 3_000);
    let dark = image
        .pixels()
        .filter(|p| p.0.iter().all(|c| *c < 64))
        .count();
    assert!(dark > 20_000, "form text/lines missing: {dark}");
}

// Minimal standards-compliant fixture builder; no extra test dependencies.
fn pdf(pages: &[(f32, f32, &str, &str)]) -> Vec<u8> {
    let kids = (0..pages.len())
        .map(|i| format!("{} 0 R", 4 + 2 * i))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", pages.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ];
    for (index, (width, height, extra, content)) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R \
            /MediaBox [0 0 {width} {height}] {extra} \
            /Resources << /Font << /F1 3 0 R >> >> \
            /Contents {} 0 R >>",
            5 + 2 * index
        ));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    let mut file = "%PDF-1.4\n".to_owned();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(file.len());
        write!(file, "{} 0 obj\n{object}\nendobj\n", index + 1).unwrap();
    }
    let xref = file.len();
    write!(file, "xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1)
        .unwrap();
    for offset in offsets {
        writeln!(file, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        file,
        "trailer\n<< /Size {} /Root 1 0 R >>\n\
        startxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    )
    .unwrap();
    file.into_bytes()
}
