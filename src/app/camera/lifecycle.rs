//! Explicit pause/resume and restart after sensor-orientation changes.

use super::{Event, FastFill, Message, reset};
use iced::Task;

pub(super) fn pause(
    state: &mut FastFill,
    reason: &'static str,
) -> Task<Message> {
    let id = state.project.request_id();
    let draft = state.camera.draft.as_mut().unwrap();
    reset(draft, id);
    draft.sources.loading = false;
    draft.paused = Some(reason);
    Task::none()
}

pub(super) fn update(
    state: &mut FastFill,
    event: &Event,
) -> Option<Task<Message>> {
    let draft = state.camera.draft.as_mut().unwrap();
    if draft.captured || draft.saving {
        return None;
    }
    match event {
        Event::Pause if draft.sources.mode.is_some() => {
            return Some(pause(state, "Camera preview paused."));
        }
        Event::Rotate | Event::Mirror
            if draft.sources.mode.is_some() && !draft.sources.loading =>
        {
            if matches!(event, Event::Rotate) {
                draft.orientation.rotate();
            } else {
                draft.orientation.mirrored = !draft.orientation.mirrored;
            }
            let id = state.project.request_id();
            reset(draft, id);
            return Some(if draft.paused.is_some() {
                Task::none()
            } else {
                Task::done(Message::Camera(Event::Start(id)))
            });
        }
        _ => {}
    }
    None
}

pub(super) const fn permission_help() -> &'static str {
    #[cfg(target_os = "windows")]
    return "Check Camera access and desktop-app camera access in Windows \
        camera privacy settings. Close other camera apps, then retry or \
        refresh cameras. No developer mode or package identity is needed.";
    #[cfg(target_os = "linux")]
    return "Check camera privacy controls and access to /dev/videoN in your \
        desktop session. Close other camera apps, then retry or refresh \
        cameras. FastFill does not change device permissions.";
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    "Native camera capture currently supports Linux and Windows only."
}

#[cfg(test)]
mod tests;
