use super::*;
use crate::app::camera::tests::{frame, ready};

fn send(state: &mut FastFill, event: Event) {
    if let Event::Frame(id, Ok(pixels)) = event {
        crate::app::camera::tests::present(state, id, pixels);
    } else {
        let _ = super::super::update(state, &Message::Camera(event));
    }
}

fn device(index: u32) -> Device {
    Device {
        index,
        name: format!("Camera {index}"),
    }
}

fn discovered() -> FastFill {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    send(
        &mut state,
        Event::Devices(id, Ok(vec![device(0), device(2)])),
    );
    state
}

#[test]
fn discovery_prefers_full_hd_and_switches_invalidate_preview_and_jobs() {
    let mut state = discovered();
    let hd = Spec {
        width: 1920,
        height: 1080,
        ..Spec::default()
    };
    let low = Spec::default();
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Modes(id, Ok(vec![hd, low])));
    assert_eq!(state.camera.draft.as_ref().unwrap().sources.mode, Some(hd));
    send(&mut state, Event::Frame(id, Ok(frame())));
    assert!(state.camera.draft.as_ref().unwrap().can_capture());
    send(&mut state, Event::Mode(low));
    let draft = state.camera.draft.as_ref().unwrap();
    let mode_id = draft.id;
    assert_ne!(mode_id, id);
    assert_eq!(draft.sources.mode, Some(low));
    assert!(!draft.can_capture());
    assert!(draft.tracking.corners.is_none());
    send(&mut state, Event::Frame(id, Ok(frame())));
    assert!(state.camera.draft.as_ref().unwrap().last.is_none());
    send(&mut state, Event::Device(device(2)));
    let draft = state.camera.draft.as_ref().unwrap();
    assert_ne!(draft.id, mode_id);
    assert_eq!(draft.sources.device, Some(device(2)));
    assert!(draft.sources.mode.is_none());
    assert_eq!(draft.sources.modes, [] as [Spec; 0]);
    assert!(draft.sources.loading);
    send(&mut state, Event::Modes(mode_id, Ok(vec![hd])));
    assert!(state.camera.draft.as_ref().unwrap().sources.mode.is_none());
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Modes(id, Ok(vec![hd])));
    assert_eq!(state.camera.draft.as_ref().unwrap().sources.mode, Some(hd));
}

#[test]
fn empty_catalogs_errors_and_unadvertised_choices_never_start_drivers() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Devices(id, Ok(vec![])));
    assert!(state.camera.draft.as_ref().unwrap().error.is_some());
    send(&mut state, Event::Device(device(99)));
    assert_eq!(state.camera.draft.as_ref().unwrap().id, id);
    send(&mut state, Event::Mode(Spec::default()));
    assert_eq!(state.camera.draft.as_ref().unwrap().id, id);
    let mut state = discovered();
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Modes(id, Ok(vec![])));
    assert!(state.camera.draft.as_ref().unwrap().error.is_some());
    assert!(state.camera.draft.as_ref().unwrap().sources.mode.is_none());
    send(&mut state, Event::Device(device(2)));
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Modes(id, Err("Busy camera".into())));
    assert!(!state.camera.draft.as_ref().unwrap().sources.loading);
    assert_eq!(
        state.camera.draft.as_ref().unwrap().error.as_deref(),
        Some("Busy camera")
    );
}

#[test]
fn late_discovery_cannot_reopen_canceled_or_replaced_sessions() {
    let mut state = ready();
    let old = state.camera.draft.as_ref().unwrap().id;
    let before = state.pages.identity();
    send(&mut state, Event::Cancel);
    send(&mut state, Event::Devices(old, Ok(vec![device(0)])));
    assert!(!state.camera.is_open());
    send(&mut state, Event::Begin);
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Discover(old, None));
    send(&mut state, Event::Devices(old, Ok(vec![device(2)])));
    let draft = state.camera.draft.as_ref().unwrap();
    assert_eq!(draft.id, id);
    assert!(draft.sources.device.is_none());
    assert_eq!(state.pages.identity(), before);
}

#[test]
fn captured_photo_is_not_discarded_by_device_or_mode_events() {
    let mut state = discovered();
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Modes(id, Ok(vec![Spec::default()])));
    send(&mut state, Event::Frame(id, Ok(frame())));
    send(&mut state, Event::Capture);
    send(&mut state, Event::Device(device(2)));
    send(&mut state, Event::Mode(Spec::default()));
    send(&mut state, Event::Refresh);
    let draft = state.camera.draft.as_ref().unwrap();
    assert_eq!(draft.id, id);
    assert!(draft.captured);
    assert!(draft.last.is_some());
}

#[test]
fn front_camera_defaults_mirror_but_refresh_keeps_user_override() {
    let mut state = ready();
    let front = Device {
        index: 0,
        name: "Camera Front".into(),
    };
    let rear = Device {
        index: 1,
        name: "Camera Rear".into(),
    };
    let devices = vec![front.clone(), rear.clone()];
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Devices(id, Ok(devices.clone())));
    assert!(state.camera.draft.as_ref().unwrap().orientation.mirrored);
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Modes(id, Ok(vec![Spec::default()])));
    send(&mut state, Event::Mirror);
    assert!(!state.camera.draft.as_ref().unwrap().orientation.mirrored);
    send(&mut state, Event::Refresh);
    let id = state.camera.draft.as_ref().unwrap().id;
    send(&mut state, Event::Devices(id, Ok(devices)));
    assert!(!state.camera.draft.as_ref().unwrap().orientation.mirrored);
    send(&mut state, Event::Device(rear));
    assert!(!state.camera.draft.as_ref().unwrap().orientation.mirrored);
    send(&mut state, Event::Device(front));
    assert!(state.camera.draft.as_ref().unwrap().orientation.mirrored);
}
