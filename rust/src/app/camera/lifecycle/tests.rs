use super::*;
use crate::app::camera::tests::{display_frame, frame, present, ready};
use fastfill_rust::camera::{Spec, catalog::Device, stream::INTERRUPTED};
use std::sync::Arc;
use std::time::Instant;

fn configured() -> FastFill {
    let mut state = ready();
    let sources = &mut state.camera.draft.as_mut().unwrap().sources;
    sources.loading = false;
    sources.devices = vec![Device {
        index: 0,
        name: "Test camera".into(),
    }];
    sources.device = sources.devices.first().cloned();
    sources.modes = vec![Spec::default()];
    sources.mode = Some(Spec::default());
    state
}

fn send(state: &mut FastFill, event: Event) -> Task<Message> {
    super::super::update(state, &Message::Camera(event)).unwrap()
}

#[test]
fn pause_invalidates_frames_jobs_and_queued_starts_without_editing() {
    let mut state = configured();
    let identity = state.pages.identity();
    let id = state.camera.draft.as_ref().unwrap().id;
    present(&mut state, id, frame());
    let job = state.camera.analysis.as_ref().unwrap().id;
    assert_eq!(send(&mut state, Event::Pause).units(), 0);
    let paused_id = state.camera.draft.as_ref().unwrap().id;
    assert_ne!(paused_id, id);
    let _ = send(&mut state, Event::Start(id));
    let _ = send(&mut state, Event::Frame(id, Ok(frame())));
    let _ = send(
        &mut state,
        Event::Uploaded(id, Ok(display_frame(frame(), Instant::now()))),
    );
    let _ = send(&mut state, Event::Framed(job, Err("Old job".into())));
    let draft = state.camera.draft.as_ref().unwrap();
    assert_eq!(draft.id, paused_id);
    assert!(draft.paused.is_some());
    assert!(draft.last.is_none());
    assert!(draft.running.is_none());
    assert!(draft.error.is_none());
    assert!(!draft.can_capture());
    assert!(state.camera.analysis.is_none());
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn paused_configuration_never_starts_preview_until_explicit_resume() {
    let mut state = configured();
    let _ = send(&mut state, Event::Pause);
    for event in [Event::Rotate, Event::Mirror, Event::Mode(Spec::default())] {
        assert_eq!(send(&mut state, event).units(), 0);
    }
    let orientation = state.camera.draft.as_ref().unwrap().orientation;
    assert_eq!(orientation.degrees(), 90);
    assert!(orientation.mirrored);
    let _ = send(&mut state, Event::Refresh);
    let id = state.camera.draft.as_ref().unwrap().id;
    let device = Device {
        index: 0,
        name: "Test camera".into(),
    };
    let _ = send(&mut state, Event::Devices(id, Ok(vec![device.clone()])));
    let id = state.camera.draft.as_ref().unwrap().id;
    assert_eq!(
        send(&mut state, Event::Modes(id, Ok(vec![Spec::default()]))).units(),
        0
    );
    let _ = send(&mut state, Event::Device(device));
    let id = state.camera.draft.as_ref().unwrap().id;
    assert_eq!(
        send(&mut state, Event::Modes(id, Ok(vec![Spec::default()]))).units(),
        0
    );
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.paused.is_some());
    assert!(draft.running.is_none());
    assert_eq!(draft.orientation, orientation);
    // Start is the explicit Resume command; do not open hardware in tests.
    let _ = super::super::view(&state);
}

#[test]
fn interruption_pauses_but_denied_access_remains_a_recoverable_error() {
    for error in [INTERRUPTED, "Permission denied", "Camera busy"] {
        let mut state = configured();
        let identity = state.pages.identity();
        let id = state.camera.draft.as_ref().unwrap().id;
        present(&mut state, id, frame());
        let _ = send(&mut state, Event::Frame(id, Err(error.into())));
        let draft = state.camera.draft.as_ref().unwrap();
        assert_ne!(draft.id, id);
        assert_eq!(draft.paused.is_some(), error == INTERRUPTED);
        assert_eq!(
            draft.error.as_deref(),
            (error != INTERRUPTED).then_some(error)
        );
        assert!(draft.last.is_none());
        assert!(draft.running.is_none());
        assert_eq!(state.pages.identity(), identity);
        let _ = super::super::view(&state);
        let _ = send(&mut state, Event::Refresh);
        assert!(state.camera.draft.as_ref().unwrap().sources.loading);
        assert!(state.camera.draft.as_ref().unwrap().error.is_none());
    }
    assert!(permission_help().contains("camera"));
}

#[test]
fn frozen_photo_ignores_pause_orientation_and_late_errors() {
    let mut state = configured();
    let _ = send(&mut state, Event::Rotate);
    let id = state.camera.draft.as_ref().unwrap().id;
    let pixels = frame();
    present(&mut state, id, pixels.clone());
    let _ = send(&mut state, Event::Capture);
    let orientation = state.camera.draft.as_ref().unwrap().orientation;
    for event in [
        Event::Rotate,
        Event::Mirror,
        Event::Pause,
        Event::Frame(id, Err(INTERRUPTED.into())),
    ] {
        let _ = send(&mut state, event);
    }
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.captured);
    assert!(draft.error.is_none());
    assert_eq!(draft.orientation, orientation);
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &pixels));
}

#[test]
fn transformed_pixels_are_the_full_resolution_encoded_photo() {
    let mut orientation = super::super::orientation::Orientation::default();
    orientation.rotate();
    orientation.mirrored = true;
    let pixels = orientation.apply(image::RgbaImage::from_fn(16, 8, |x, y| {
        image::Rgba([
            u8::try_from(x).unwrap(),
            u8::try_from(y).unwrap(),
            30,
            255,
        ])
    }));
    let saved = super::super::worker::prepare(&pixels).unwrap();
    assert_eq!((saved.info.width, saved.info.height), (8, 16));
    let iced::widget::image::Handle::Bytes(_, bytes) = &saved.handle else {
        panic!("Prepared photo must retain PNG bytes");
    };
    let encoded = image::load_from_memory(bytes).unwrap().to_rgba8();
    assert_eq!(encoded, pixels);
    // A clockwise turn, never a horizontal mirror of the printed document.
    assert_eq!(encoded[(0, 0)], image::Rgba([0, 7, 30, 255]));
}

#[test]
fn changing_cameras_resets_orientation_but_keeps_explicit_pause() {
    let mut state = configured();
    let _ = send(&mut state, Event::Pause);
    let _ = send(&mut state, Event::Rotate);
    let _ = send(&mut state, Event::Mirror);
    let other = Device {
        index: 1,
        name: "Other camera".into(),
    };
    state
        .camera
        .draft
        .as_mut()
        .unwrap()
        .sources
        .devices
        .push(other.clone());
    let _ = send(&mut state, Event::Device(other));
    let draft = state.camera.draft.as_ref().unwrap();
    assert_eq!(
        draft.orientation,
        super::super::orientation::Orientation::default()
    );
    assert!(draft.paused.is_some());
    assert!(draft.sources.loading);
    assert!(draft.last.is_none());
}

#[test]
fn interrupted_discovery_requires_refresh_and_explicit_resume() {
    for devices in [true, false] {
        let mut state = configured();
        let draft = state.camera.draft.as_mut().unwrap();
        let old = draft.id;
        draft.sources.loading = true;
        let event = if devices {
            Event::Devices(old, Err(INTERRUPTED.into()))
        } else {
            Event::Modes(old, Err(INTERRUPTED.into()))
        };
        let _ = send(&mut state, event);
        let _ = send(&mut state, Event::Modes(old, Ok(vec![Spec::default()])));
        let draft = state.camera.draft.as_ref().unwrap();
        assert_ne!(draft.id, old);
        assert!(draft.paused.is_some());
        assert!(draft.error.is_none());
        assert!(!draft.sources.loading);
        assert!(draft.running.is_none());
        let _ = send(&mut state, Event::Refresh);
        assert!(state.camera.draft.as_ref().unwrap().paused.is_some());
    }
}
