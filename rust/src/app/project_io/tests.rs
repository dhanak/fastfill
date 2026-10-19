use super::*;
use crate::app::controller::update;
use crate::app::pages::Pages;
use crate::app::project::Operation;
use crate::app::{FastFill, Message, PagePoint, Pointer, Tool};
use fastfill_rust::correction::{PageCorrection, correct_image};
use fastfill_rust::geometry::Size;
use fastfill_rust::project::{PageFilterMode, save_archive};
use image::{Rgba, RgbaImage};

fn fixture() -> (LoadedProject, RgbaImage) {
    let pixels = RgbaImage::from_fn(9, 7, |x, y| {
        Rgba([
            u8::try_from(20 + x * 20).unwrap(),
            u8::try_from(30 + y * 25).unwrap(),
            90,
            255,
        ])
    });
    let bytes =
        encode_png(&image::DynamicImage::ImageRgba8(pixels.clone())).unwrap();
    let image =
        ImportedImage::from_bytes("color.png".into(), bytes, None).unwrap();
    let mut state = FastFill::default();
    assert!(state.pages.add_image(image));
    let mut archive =
        encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    let original = archive.project.pages[0].clone();
    for index in 1..4 {
        let mut page = original.clone();
        page.id = uuid::Uuid::new_v4();
        page.rotation_quarter_turns = index;
        page.filter.mode = match index {
            1 => PageFilterMode::Grayscale,
            2 => PageFilterMode::EnhancedColor,
            _ => PageFilterMode::BlackAndWhite,
        };
        if index == 1 {
            page.crop.top_right.x = 0.5;
            page.crop.bottom_right.x = 0.5;
            page.filter.contrast = 20;
        }
        if index == 2 {
            page.brightness = 15;
        }
        archive.project.pages.push(page);
    }
    archive.project.schema_version = 2;
    (archive, pixels)
}

fn reopen(archive: &LoadedProject) -> OpenedProject {
    let bytes = save_archive(archive, Cursor::new(Vec::new()))
        .unwrap()
        .into_inner();
    decode_project(Cursor::new(bytes), None, "corrected.docscan".into())
        .unwrap()
}

#[test]
fn shared_sources_stage_each_correction_without_changing_saved_pixels() {
    let (archive, source) = fixture();
    let opened = reopen(&archive);
    assert_eq!(opened.project, archive.project);
    assert_eq!(opened.images.len(), 4);
    for (page, image) in opened.project.pages.iter().zip(&opened.images) {
        let corrected =
            correct_image(&source, PageCorrection::from(page)).unwrap();
        let Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = image.display_handle()
        else {
            panic!("page pixels must be staged before installing the project")
        };
        assert_eq!((*width, *height), corrected.dimensions());
        assert_eq!(pixels.as_ref(), corrected.as_raw());
        #[allow(clippy::cast_precision_loss)]
        let expected_size = Size::new(*width as f32, *height as f32);
        assert_eq!(image.size, expected_size);
        let Handle::Bytes(_, bytes) = &image.handle else {
            panic!("original encoded image must remain available")
        };
        assert_eq!(bytes.as_ref(), archive.assets[&page.source_asset].bytes);
    }
    let pages = Pages::from_opened(&opened).unwrap();
    for page in pages.iter() {
        let image = page.image.as_ref().unwrap();
        assert_eq!(page.size.width, 612.0);
        assert!(
            (page.size.height - 612.0 * image.size.height / image.size.width)
                .abs()
                < 0.001
        );
    }
}

#[test]
fn corrected_projects_edit_and_round_trip_without_baking_in_corrections() {
    let (archive, _) = fixture();
    let opened = Arc::new(reopen(&archive));
    let mut state = FastFill::default();
    state.project.operation = Operation::Opening(1);
    let _ = update(&mut state, Message::ProjectOpened(1, Ok(Some(opened))));
    assert!(!state.status_is_error, "{}", state.status);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::SelectPage(1));
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(60.0, 70.0), 1.0),
    );
    let _ = update(
        &mut state,
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(160.0, 170.0),
            false,
        ),
    );
    assert!(state.project.is_dirty(&state.pages));
    let snapshot = state.project_snapshot().unwrap();
    let saved = encode_snapshot(snapshot).unwrap();
    assert_eq!(saved.project.schema_version, 2);
    for (old, new) in archive.project.pages.iter().zip(&saved.project.pages) {
        assert_eq!(old.crop, new.crop);
        assert_eq!(old.filter, new.filter);
        assert_eq!(old.rotation_quarter_turns, new.rotation_quarter_turns);
        assert_eq!(old.brightness, new.brightness);
        assert_eq!(
            archive.assets[&old.source_asset].bytes,
            saved.assets[&new.source_asset].bytes
        );
    }
    let reloaded = reopen(&saved);
    let pages = Pages::from_opened(&reloaded).unwrap();
    let annotations = &pages.iter().nth(1).unwrap().annotations;
    let expected = &state.annotations().shapes().next().unwrap().clone();
    let actual = &annotations.shapes().next().unwrap().clone();
    assert_eq!(actual.id, expected.id);
    assert_eq!(actual.color, expected.color);
    assert_eq!(actual.stroke_width, expected.stroke_width);
    let (a, b) = (actual.bounds(), expected.bounds());
    for difference in
        [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height]
    {
        assert!(difference.abs() < 0.001);
    }
    assert!(!annotations.can_undo());
    for (first, second) in reopen(&archive).images.iter().zip(reloaded.images) {
        let Handle::Rgba { pixels: first, .. } = first.display_handle() else {
            panic!("staged pixels")
        };
        let Handle::Rgba { pixels: second, .. } = second.display_handle()
        else {
            panic!("staged pixels")
        };
        assert_eq!(first.as_ref(), second.as_ref());
    }
}

#[test]
fn snapshot_upgrades_only_when_a_new_feature_requires_schema_v2() {
    let mut state = FastFill::default();
    assert_eq!(state.project_snapshot().unwrap().project.schema_version, 1);
    state.pages.current_mut().metadata.filter.contrast = 10;
    assert_eq!(state.project_snapshot().unwrap().project.schema_version, 1);
    state.pages.current_mut().metadata.brightness = 10;
    let snapshot = state.project_snapshot().unwrap();
    assert_eq!(snapshot.project.schema_version, 2);
    let saved = encode_snapshot(snapshot).unwrap();
    assert_eq!(reopen(&saved).project.pages[0].brightness, 10);
}

#[test]
fn corrected_pdf_uses_the_display_aspect_and_background_pixels() {
    use crate::page_canvas::export::{ExportPage, export_pdf};
    let (archive, _) = fixture();
    let opened = reopen(&archive);
    let pages = Pages::from_opened(&opened).unwrap();
    let page = pages.iter().nth(1).unwrap();
    let pdf = iced::futures::executor::block_on(export_pdf(
        vec![ExportPage::new(
            page.size,
            page.image
                .as_ref()
                .map(|image| image.display_handle().clone()),
            &page.annotations,
        )],
        |_, _| true,
    ))
    .unwrap();
    let parsed = hayro::hayro_syntax::Pdf::new(pdf.clone()).unwrap();
    let (width, height) = parsed.pages()[0].render_dimensions();
    assert!((width - page.size.width).abs() < 0.001);
    assert!((height - page.size.height).abs() < 0.001);
    let result =
        fastfill_rust::pdf_import::import_pdf(pdf, 20, |_, _| true).unwrap();
    let pixels = &result[0].pixels;
    let center = pixels.get_pixel(pixels.width() / 2, pixels.height() / 2);
    // The cropped grayscale page must stay gray, not use the original RGB.
    assert_eq!(center[0], center[1]);
    assert_eq!(center[1], center[2]);
    assert!(center[0] > 30 && center[0] < 160);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn external_corrected_project_uses_the_same_open_and_save_pipeline() {
    let Some(path) = std::env::var_os("FASTFILL_DOCSCAN_FIXTURE") else {
        return;
    };
    let opened = decode_project(
        std::fs::File::open(path).unwrap(),
        None,
        "external.docscan".into(),
    )
    .unwrap();
    let mut state = FastFill::default();
    state.project.operation = Operation::Opening(1);
    let _ = update(
        &mut state,
        Message::ProjectOpened(1, Ok(Some(Arc::new(opened)))),
    );
    assert!(!state.status_is_error, "{}", state.status);
    let saved = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    let reloaded = reopen(&saved);
    assert_eq!(reloaded.project.pages, saved.project.pages);
    assert_eq!(reloaded.images.len(), state.pages.len());
}
