use super::*;
use fastfill_rust::document_detection::DocumentDetection;

fn detected_crop() -> CropQuad {
    CropQuad {
        top_left: NormalizedPoint { x: 0.1, y: 0.1 },
        top_right: NormalizedPoint { x: 0.9, y: 0.1 },
        bottom_right: NormalizedPoint { x: 0.9, y: 0.9 },
        bottom_left: NormalizedPoint { x: 0.1, y: 0.9 },
    }
}

fn detection(confidence: f64) -> DocumentDetection {
    DocumentDetection {
        corners: (confidence >= 0.72).then_some(detected_crop()),
        candidate_corners: (confidence < 0.72).then_some(detected_crop()),
        confidence,
        area_ratio: 0.64,
        sharpness: 100.0,
        analysis_width: 120,
        analysis_height: 180,
    }
}

fn cached(state: &mut FastFill, result: Result<DocumentDetection, String>) {
    state
        .page_editor
        .draft
        .as_ref()
        .unwrap()
        .original
        .as_ref()
        .unwrap()
        .framing
        .set(result)
        .unwrap();
}

#[test]
fn opening_is_manual_and_auto_frame_is_an_explicit_worker_request() {
    let mut state = state();
    send(&mut state, Event::Begin);
    let initial = state.page_editor.running.as_ref().unwrap().id;
    send(&mut state, Event::AutoFrame);
    assert_eq!(state.page_editor.running.as_ref().unwrap().id, initial);
    settle(&mut state);
    let original = state
        .page_editor
        .draft
        .as_ref()
        .unwrap()
        .original
        .as_ref()
        .unwrap();
    assert!(original.framing.get().is_none(), "opening must not detect");
    assert_eq!(
        state.page_editor.draft.as_ref().unwrap().settings.crop,
        CropQuad::default()
    );
    cached(&mut state, Ok(detection(0.9)));
    let epoch = state.input_epoch;
    send(&mut state, Event::AutoFrame);
    assert!(state.page_editor.is_framing());
    assert_ne!(state.input_epoch, epoch);
    let id = state.page_editor.running.as_ref().unwrap().id;
    send(&mut state, Event::AutoFrame);
    assert_eq!(state.page_editor.running.as_ref().unwrap().id, id);
    send(&mut state, Event::Accept);
    assert!(state.page_editor.is_open(), "old preview must not commit");
    assert!(!state.project.is_dirty(&state.pages));
    settle(&mut state);
    assert_eq!(
        state.page_editor.draft.as_ref().unwrap().settings.crop,
        detected_crop()
    );
}

#[test]
fn manual_full_frame_and_changed_then_restored_settings_beat_inflight_result() {
    for restore_settings in [false, true] {
        let mut state = state();
        send(&mut state, Event::Begin);
        settle(&mut state);
        cached(&mut state, Ok(detection(0.9)));
        send(&mut state, Event::AutoFrame);
        if restore_settings {
            send(&mut state, Event::Brightness(10));
            send(&mut state, Event::Brightness(0));
        } else {
            // Explicit Full frame wins even though the crop was full already.
            send(&mut state, Event::FullFrame);
        }
        settle(&mut state);
        let draft = state.page_editor.draft.as_ref().unwrap();
        assert_eq!(draft.settings.crop, CropQuad::default());
        assert_eq!(draft.ready.as_ref().unwrap().settings, draft.settings);
        assert!(
            draft
                .notice
                .as_ref()
                .unwrap()
                .contains("Manual changes kept")
        );
        send(&mut state, Event::Accept);
        assert!(!state.project.is_dirty(&state.pages));
    }
}

#[test]
fn weak_missing_failed_or_invalid_hints_leave_crop_and_accept_available() {
    let mut invalid = detection(0.9);
    invalid.corners.as_mut().unwrap().top_left =
        NormalizedPoint { x: 1.0, y: 1.0 };
    for result in [
        Ok(detection(0.449)),
        Err("Test detector failure".into()),
        Ok(invalid),
    ] {
        let mut state = state();
        send(&mut state, Event::Begin);
        settle(&mut state);
        cached(&mut state, result);
        send(&mut state, Event::AutoFrame);
        settle(&mut state);
        let draft = state.page_editor.draft.as_ref().unwrap();
        assert_eq!(draft.settings.crop, CropQuad::default());
        assert!(draft.error.is_none());
        assert!(draft.notice.is_some());
        send(&mut state, Event::Accept);
        assert!(!state.page_editor.is_open());
        assert!(!state.project.is_dirty(&state.pages));
    }
}

#[test]
fn lower_confidence_manual_suggestion_is_labeled_at_the_legacy_threshold() {
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    cached(&mut state, Ok(detection(0.45)));
    send(&mut state, Event::AutoFrame);
    settle(&mut state);
    let draft = state.page_editor.draft.as_ref().unwrap();
    assert_eq!(draft.settings.crop, detected_crop());
    assert!(draft.notice.as_ref().unwrap().contains("Low-confidence"));
    assert!(!state.project.is_dirty(&state.pages));
}

#[test]
fn canceled_framing_completion_cannot_reframe_a_new_editor_session() {
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    cached(&mut state, Ok(detection(0.9)));
    send(&mut state, Event::AutoFrame);
    let job = state.page_editor.running.as_ref().unwrap();
    let (id, settings) = (job.id, job.settings);
    let original = state
        .page_editor
        .draft
        .as_ref()
        .unwrap()
        .original
        .as_ref()
        .unwrap()
        .clone();
    let old = preview::build(Input::Frame(original), settings).unwrap();
    send(&mut state, Event::Cancel);
    send(&mut state, Event::Begin);
    send(&mut state, Event::Ready(id, Ok(old)));
    assert!(state.page_editor.draft.as_ref().unwrap().original.is_none());
    settle(&mut state);
    let draft = state.page_editor.draft.as_ref().unwrap();
    assert_eq!(draft.settings.crop, CropQuad::default());
    assert!(draft.original.as_ref().unwrap().framing.get().is_none());
    assert!(!state.project.is_dirty(&state.pages));
}

#[test]
fn actual_detection_is_cached_in_original_space_across_filters_and_rotation() {
    let pixels = image::RgbaImage::from_fn(320, 240, |x, y| {
        let value = if (60..260).contains(&x) && (50..190).contains(&y) {
            240
        } else {
            40
        };
        image::Rgba([value, value, value, 255])
    });
    let original = preview::build(
        Input::Image(Handle::from_rgba(320, 240, pixels.as_raw().clone())),
        PageCorrection::default(),
    )
    .unwrap()
    .original
    .clone();
    assert!(original.framing.get().is_none());
    let settings = PageCorrection {
        rotation_quarter_turns: 1,
        brightness: 100,
        filter: fastfill_rust::project::PageFilterSettings {
            mode: PageFilterMode::BlackAndWhite,
            contrast: 20,
        },
        ..PageCorrection::default()
    };
    let first =
        preview::build(Input::Frame(original.clone()), settings).unwrap();
    let cached = original.framing.get().unwrap();
    let expected = cached.as_ref().unwrap().corners.unwrap();
    assert_eq!(first.settings.crop, expected);
    assert_eq!(first.settings.brightness, 100);
    assert_eq!(first.settings.filter, settings.filter);
    assert_eq!(first.settings.rotation_quarter_turns, 1);
    let mut changed = first.settings;
    changed.crop = CropQuad::default();
    changed.rotation_quarter_turns = 3;
    changed.brightness = -50;
    let second =
        preview::build(Input::Frame(original.clone()), changed).unwrap();
    assert_eq!(second.settings.crop, expected);
    assert_eq!(second.settings.brightness, -50);
    assert_eq!(second.settings.rotation_quarter_turns, 3);
    assert!(Arc::ptr_eq(&first.original, &second.original));
    assert!(std::ptr::eq(cached, original.framing.get().unwrap()));
    assert_eq!(original.pixels, pixels);
}

#[test]
fn worker_failure_is_nonblocking_and_pending_manual_changes_are_preserved() {
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    send(&mut state, Event::AutoFrame);
    let id = state.page_editor.running.as_ref().unwrap().id;
    send(&mut state, Event::Ready(id, Err("Worker stopped".into())));
    let draft = state.page_editor.draft.as_ref().unwrap();
    assert!(draft.error.is_none());
    assert!(draft.notice.as_ref().unwrap().contains("Worker stopped"));
    assert_eq!(draft.settings.crop, CropQuad::default());
    cached(&mut state, Ok(detection(0.9)));
    send(&mut state, Event::AutoFrame);
    send(
        &mut state,
        Event::Corner(0, NormalizedPoint { x: 0.2, y: 0.15 }),
    );
    send(&mut state, Event::Contrast(15));
    let expected = state.page_editor.draft.as_ref().unwrap().settings;
    finish_one(&mut state);
    assert!(!state.page_editor.is_framing());
    assert!(
        state.page_editor.running.is_some(),
        "manual preview is queued"
    );
    assert_eq!(state.page_editor.draft.as_ref().unwrap().settings, expected);
    settle(&mut state);
    assert_eq!(
        state
            .page_editor
            .draft
            .as_ref()
            .unwrap()
            .ready
            .as_ref()
            .unwrap()
            .settings,
        expected
    );
    send(&mut state, Event::Accept);
    assert_eq!(state.pages.current().metadata.crop, expected.crop);
    assert_eq!(state.pages.current().metadata.filter.contrast, 15);
}

#[test]
fn framing_preserves_annotations_original_bytes_and_one_step_history() {
    let mut state = state();
    let page = state.pages.current().size;
    state.annotations_mut().set_tool(Tool::Text);
    state
        .annotations_mut()
        .begin_text(PagePoint::new(250.0, 350.0), page, 1.0);
    state
        .annotations_mut()
        .update_text("Example", Size::new(100.0, 32.0));
    state.annotations_mut().accept_text();
    mark_saved(&mut state);
    let before = state.pages.current().metadata.clone();
    let shapes = state.annotations().shapes().to_vec();
    let texts = state.annotations().texts().to_vec();
    let original = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    send(&mut state, Event::Begin);
    settle(&mut state);
    cached(&mut state, Ok(detection(0.9)));
    send(&mut state, Event::Brightness(15));
    send(&mut state, Event::Rotate(1));
    settle(&mut state);
    send(&mut state, Event::AutoFrame);
    settle(&mut state);
    assert_eq!(state.pages.current().metadata, before);
    assert_eq!(state.annotations().shapes(), shapes);
    assert_eq!(state.annotations().texts(), texts);
    assert!(!state.project.is_dirty(&state.pages));
    send(&mut state, Event::Accept);
    let accepted = state.pages.current().metadata.clone();
    let changed = state.annotations().shapes()[0].clone();
    let text = state.annotations().texts()[0].clone();
    assert_eq!(accepted.crop, detected_crop());
    assert_eq!(accepted.brightness, 15);
    assert_eq!(accepted.rotation_quarter_turns, 1);
    assert_eq!(changed.id, shapes[0].id);
    assert_eq!(changed.stroke_width, shapes[0].stroke_width);
    assert_eq!(text.id, texts[0].id);
    let saved = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    assert_eq!(
        saved.assets[&accepted.source_asset].bytes,
        original.assets[&before.source_asset].bytes
    );
    let bytes = save_archive(&saved, Cursor::new(Vec::new()))
        .unwrap()
        .into_inner();
    let opened =
        decode_project(Cursor::new(bytes), None, "autoframed.docscan".into())
            .unwrap();
    assert_eq!(opened.project.pages[0].crop, accepted.crop);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.pages.current().metadata, before);
    assert_eq!(state.annotations().shapes(), shapes);
    assert_eq!(state.annotations().texts(), texts);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::Redo);
    assert_eq!(state.pages.current().metadata, accepted);
    assert_eq!(state.annotations().shapes()[0], changed);
    assert_eq!(state.annotations().texts()[0], text);
}

#[test]
fn auto_frame_then_escape_does_not_commit_or_leave_a_cached_session() {
    let mut state = state();
    let identity = state.pages.identity();
    send(&mut state, Event::Begin);
    settle(&mut state);
    cached(&mut state, Ok(detection(0.9)));
    send(&mut state, Event::AutoFrame);
    settle(&mut state);
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.page_editor.is_open());
    assert_eq!(state.pages.identity(), identity);
    assert!(!state.project.is_dirty(&state.pages));
    send(&mut state, Event::AutoFrame);
    assert!(state.page_editor.running.is_none());
    send(&mut state, Event::Begin);
    settle(&mut state);
    assert!(
        state
            .page_editor
            .draft
            .as_ref()
            .unwrap()
            .original
            .as_ref()
            .unwrap()
            .framing
            .get()
            .is_none()
    );
}

#[test]
fn auto_framed_page_exports_with_corrected_dimensions_without_mutation() {
    use crate::page_canvas::export::{ExportPage, export_pdf};
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    cached(&mut state, Ok(detection(0.9)));
    send(&mut state, Event::Rotate(1));
    settle(&mut state);
    send(&mut state, Event::AutoFrame);
    settle(&mut state);
    send(&mut state, Event::Accept);
    let identity = state.pages.identity();
    let page = state.pages.current();
    let size = page.size;
    let output = ExportPage::new(
        size,
        Some(page.image.as_ref().unwrap().display_handle().clone()),
        &page.annotations,
    );
    let bytes =
        iced::futures::executor::block_on(export_pdf(vec![output], |_, _| {
            true
        }))
        .unwrap();
    let parsed = hayro::hayro_syntax::Pdf::new(bytes).unwrap();
    let (width, height) = parsed.pages()[0].render_dimensions();
    assert!((width - size.width).abs() < 0.01);
    assert!((height - size.height).abs() < 0.01);
    assert_eq!(state.pages.identity(), identity);
    assert!(state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::Undo);
    assert!(!state.project.is_dirty(&state.pages));
}
