use super::*;
use fastfill_rust::project::NormalizedPoint;

fn detection() -> DocumentDetection {
    DocumentDetection {
        corners: Some(CropQuad {
            top_left: NormalizedPoint { x: 0.2, y: 0.1 },
            top_right: NormalizedPoint { x: 0.8, y: 0.1 },
            bottom_right: NormalizedPoint { x: 0.8, y: 0.9 },
            bottom_left: NormalizedPoint { x: 0.2, y: 0.9 },
        }),
        candidate_corners: None,
        confidence: 0.85,
        area_ratio: 0.48,
        sharpness: 60.0,
        analysis_width: 640,
        analysis_height: 480,
    }
}

#[test]
fn live_tracking_uses_consensus_and_the_calibrated_one_second_countdown() {
    let mut tracking = Tracking::default();
    let start = Instant::now();
    for ms in [0, 100, 300, 600, 900] {
        let at = start + Duration::from_millis(ms);
        assert!(!tracking.update(&detection(), at, at, (640, 480)));
    }
    assert!(tracking.corners.is_some());
    assert!(
        (tracking.progress(start + Duration::from_millis(900)) - 0.8).abs()
            < 0.001
    );
    let at = start + Duration::from_millis(1100);
    assert!(tracking.update(&detection(), at, at, (640, 480)));
    assert_eq!(tracking.progress(at), 1.0);
    assert!(!tracking.update(&detection(), at, at, (640, 480)));
}

#[test]
fn slow_results_gaps_dimension_changes_and_current_blur_reset_capture() {
    for mode in ["slow", "gap", "dimensions", "blur", "missing", "jump"] {
        let mut tracking = Tracking::default();
        let start = Instant::now();
        for ms in [0, 100, 500, 900] {
            let at = start + Duration::from_millis(ms);
            assert!(!tracking.update(&detection(), at, at, (640, 480)));
        }
        let mut raw = detection();
        if mode == "blur" {
            raw.sharpness = 1.0;
        }
        if mode == "missing" {
            raw.corners = None;
        }
        if mode == "jump" {
            let quad = raw.corners.as_mut().unwrap();
            for point in [
                &mut quad.top_left,
                &mut quad.top_right,
                &mut quad.bottom_right,
                &mut quad.bottom_left,
            ] {
                point.x += 0.1;
            }
        }
        let at = start
            + Duration::from_millis(if mode == "gap" { 2500 } else { 1100 });
        let now = at
            + if mode == "slow" {
                Duration::from_secs(2)
            } else {
                Duration::ZERO
            };
        let dimensions = if mode == "dimensions" {
            (480, 640)
        } else {
            (640, 480)
        };
        assert!(!tracking.update(&raw, at, now, dimensions), "{mode}");
        assert_eq!(tracking.progress(now), 0.0, "{mode}");
    }
}

fn camera() -> FastFill {
    let mut state = FastFill::default();
    let _ = super::super::update(&mut state, &Message::Camera(Event::Begin));
    state
}

fn receive(
    state: &mut FastFill,
    at: Instant,
    value: u8,
) -> std::sync::Arc<image::RgbaImage> {
    let id = state.camera.draft.as_ref().unwrap().id;
    let had_job = state.camera.analysis.is_some();
    let pixels = std::sync::Arc::new(image::RgbaImage::from_pixel(
        16,
        8,
        image::Rgba([value, value, value, 255]),
    ));
    super::super::tests::present(state, id, pixels.clone());
    state
        .camera
        .draft
        .as_mut()
        .unwrap()
        .last
        .as_mut()
        .unwrap()
        .arrived = at;
    if !had_job {
        state.camera.analysis.as_mut().unwrap().frame.arrived = at;
    }
    pixels
}

fn finish(state: &mut FastFill, at: Instant) {
    let id = state.camera.analysis.as_ref().unwrap().id;
    let _ = ready(state, id, &Ok(detection()), at);
}

#[test]
fn auto_capture_freezes_analyzed_pixels_and_crop_not_a_newer_preview() {
    let mut state = camera();
    let identity = state.pages.identity();
    let start = Instant::now();
    for ms in [0, 100, 500, 900] {
        let at = start + Duration::from_millis(ms);
        receive(&mut state, at, 100);
        finish(&mut state, at);
        assert!(!state.camera.draft.as_ref().unwrap().captured);
    }
    let at = start + Duration::from_millis(1100);
    let analyzed = receive(&mut state, at, 150);
    let id = state.camera.analysis.as_ref().unwrap().id;
    let newer = receive(&mut state, at + Duration::from_millis(20), 200);
    assert_eq!(state.camera.analysis.as_ref().unwrap().id, id);
    assert!(std::sync::Arc::ptr_eq(
        &state
            .camera
            .draft
            .as_ref()
            .unwrap()
            .last
            .as_ref()
            .unwrap()
            .pixels,
        &newer,
    ));
    finish(&mut state, at + Duration::from_millis(25));
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.captured);
    assert_eq!(draft.crop, detection().corners);
    assert!(std::sync::Arc::ptr_eq(
        &draft.last.as_ref().unwrap().pixels,
        &analyzed,
    ));
    assert_eq!(state.pages.identity(), identity);
    assert!(state.camera.analysis.is_none());
    let _ = ready(&mut state, id, &Ok(detection()), at);
    assert_eq!(state.pages.identity(), identity);

    let image = super::super::worker::prepare(&analyzed).unwrap();
    let session = state.camera.draft.as_ref().unwrap().id;
    state.camera.draft.as_mut().unwrap().saving = true;
    let _ = super::super::update(
        &mut state,
        &Message::Camera(Event::Prepared(session, Ok(image))),
    );
    assert!(state.page_editor.is_open());
    // The suggestion is pending, never written into the accepted page.
    assert_eq!(state.pages.current().metadata.crop, CropQuad::default());
}

#[test]
fn auto_capture_uses_current_corners_despite_smoothing_lag() {
    let mut state = camera();
    let start = Instant::now();
    for (ms, dx) in [
        (0, 0.0),
        (100, 0.004),
        (500, 0.008),
        (900, 0.012),
        (1100, 0.016),
    ] {
        let at = start + Duration::from_millis(ms);
        let pixels = receive(&mut state, at, 150);
        let mut raw = detection();
        let quad = raw.corners.as_mut().unwrap();
        for point in [
            &mut quad.top_left,
            &mut quad.top_right,
            &mut quad.bottom_right,
            &mut quad.bottom_left,
        ] {
            point.x += dx;
        }
        let id = state.camera.analysis.as_ref().unwrap().id;
        let _ = ready(&mut state, id, &Ok(raw.clone()), at);
        let draft = state.camera.draft.as_ref().unwrap();
        assert_eq!(draft.captured, ms == 1100);
        if draft.captured {
            assert_ne!(draft.tracking.corners, raw.corners);
            assert_eq!(draft.crop, raw.corners);
            assert!(std::sync::Arc::ptr_eq(
                &draft.last.as_ref().unwrap().pixels,
                &pixels,
            ));
        }
    }
}

#[test]
fn analysis_is_bounded_across_cancellation_and_reopening_camera() {
    let mut state = camera();
    let start = Instant::now();
    receive(&mut state, start, 100);
    let id = state.camera.analysis.as_ref().unwrap().id;
    receive(&mut state, start + Duration::from_millis(20), 150);
    assert_eq!(state.camera.analysis.as_ref().unwrap().id, id);
    let _ = super::super::update(&mut state, &Message::Camera(Event::Cancel));
    assert!(state.camera.analysis.is_some());
    let _ = super::super::update(&mut state, &Message::Camera(Event::Begin));
    let latest = receive(&mut state, start + Duration::from_millis(40), 200);
    assert_eq!(state.camera.analysis.as_ref().unwrap().id, id);
    let _ = ready(&mut state, id, &Ok(detection()), start);
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(!draft.captured);
    assert!(draft.tracking.corners.is_none());
    let next = state.camera.analysis.as_ref().unwrap();
    assert_ne!(next.id, id);
    assert_eq!(next.session, draft.id);
    assert!(std::sync::Arc::ptr_eq(&next.frame.pixels, &latest));
}

#[test]
fn analysis_failure_does_not_stop_preview_or_manual_capture() {
    let mut state = camera();
    let now = Instant::now();
    receive(&mut state, now, 100);
    let id = state.camera.analysis.as_ref().unwrap().id;
    let _ = ready(&mut state, id, &Err("fixture error".into()), now);
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.error.is_none());
    assert!(draft.can_capture());
    assert!(draft.tracking.status(now).contains("capture manually"));
    assert!(state.camera.analysis.is_none());
    let _ = super::super::update(&mut state, &Message::Camera(Event::Capture));
    assert!(state.camera.draft.as_ref().unwrap().captured);
    assert!(state.camera.draft.as_ref().unwrap().crop.is_none());
}

#[test]
fn pre_sleep_or_future_wall_clock_results_cannot_finish_capture() {
    use std::time::SystemTime;
    for future in [false, true] {
        let mut state = camera();
        let start = Instant::now();
        for ms in [0, 100, 500, 900] {
            let at = start + Duration::from_millis(ms);
            receive(&mut state, at, 100);
            finish(&mut state, at);
        }
        let at = start + Duration::from_millis(1100);
        receive(&mut state, at, 100);
        let job = state.camera.analysis.as_mut().unwrap();
        job.frame.wall_time = if future {
            SystemTime::now() + Duration::from_secs(60)
        } else {
            SystemTime::now() - Duration::from_secs(60)
        };
        finish(&mut state, at);
        let draft = state.camera.draft.as_ref().unwrap();
        assert!(!draft.captured);
        assert!(draft.crop.is_none());
        assert_eq!(draft.tracking.progress(at), 0.0);
        assert!(draft.tracking.status(at).contains("Old frame discarded"));
    }
}
