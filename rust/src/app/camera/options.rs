//! Device/mode choices own discovery state, never document state.

use super::{Draft, Event, FastFill, Message, orientation::Orientation, reset};
use fastfill_rust::camera::{
    Spec,
    catalog::{self, Device},
};
use iced::Task;

#[derive(Default)]
pub(super) struct Sources {
    pub(super) devices: Vec<Device>,
    pub(super) device: Option<Device>,
    pub(super) modes: Vec<Spec>,
    pub(super) mode: Option<Spec>,
    pub(super) loading: bool,
    pub(super) query: Option<fastfill_rust::camera::stream::Stream>,
}

pub(super) fn refresh(state: &mut FastFill) -> Task<Message> {
    let id = state.project.request_id();
    let draft = state.camera.draft.as_mut().unwrap();
    reset(draft, id);
    let device = draft.sources.device.take();
    draft.sources = Sources {
        device,
        loading: true,
        ..Sources::default()
    };
    Task::done(Message::Camera(Event::Discover(id, None)))
}

pub(super) fn update(
    state: &mut FastFill,
    event: &Event,
) -> Option<Task<Message>> {
    let draft = state.camera.draft.as_mut().unwrap();
    if let Event::Settings(open) = event {
        draft.settings_open = *open && !draft.captured && !draft.saving;
        return Some(Task::none());
    }
    if draft.saving || draft.captured {
        return None;
    }
    match event {
        Event::Refresh => return Some(refresh(state)),
        Event::Discover(id, index)
            if *id == draft.id
                && draft.sources.loading
                && draft.sources.query.is_none()
                && index.is_none_or(|index| {
                    draft
                        .sources
                        .device
                        .as_ref()
                        .is_some_and(|d| d.index == index)
                }) =>
        {
            return Some(discover(draft, *index));
        }
        Event::Devices(id, result)
            if *id == draft.id && draft.sources.loading =>
        {
            draft.sources.loading = false;
            draft.sources.query = None;
            match result {
                Ok(devices) => {
                    draft.sources.devices.clone_from(devices);
                    let selected = draft
                        .sources
                        .device
                        .as_ref()
                        .and_then(|old| devices.iter().find(|d| *d == old))
                        .or_else(|| devices.first())
                        .cloned();
                    select_device(draft, selected);
                    if draft.sources.device.is_some() {
                        return Some(load_modes(state));
                    }
                    draft.error = Some(
                        "No camera found. Connect one, check \
                        permissions and refresh cameras."
                            .into(),
                    );
                }
                Err(error) => return Some(discovery_error(state, error)),
            }
        }
        Event::Device(device) if draft.sources.devices.contains(device) => {
            select_device(draft, Some(device.clone()));
            return Some(load_modes(state));
        }
        Event::Modes(id, result)
            if *id == draft.id && draft.sources.loading =>
        {
            draft.sources.loading = false;
            draft.sources.query = None;
            match result {
                Ok(modes) => {
                    draft.sources.modes = catalog::resolutions(modes.clone());
                    draft.sources.mode =
                        catalog::preferred(&draft.sources.modes);
                    if draft.sources.mode.is_some() && draft.paused.is_none() {
                        return Some(Task::done(Message::Camera(
                            Event::Start(draft.id),
                        )));
                    }
                    if draft.sources.modes.is_empty() {
                        draft.error = Some(
                            "This device has no supported capture \
                            modes. Choose another camera."
                                .into(),
                        );
                    }
                }
                Err(error) => return Some(discovery_error(state, error)),
            }
        }
        Event::Mode(mode)
            if !draft.sources.loading && draft.sources.modes.contains(mode) =>
        {
            draft.sources.mode = Some(*mode);
            let id = state.project.request_id();
            reset(draft, id);
            return Some(if draft.paused.is_some() {
                Task::none()
            } else {
                Task::done(Message::Camera(Event::Start(id)))
            });
        }
        _ => return None,
    }
    Some(Task::none())
}

fn select_device(draft: &mut Draft, device: Option<Device>) {
    if draft.sources.device != device {
        draft.orientation = device
            .as_ref()
            .map_or_else(Orientation::default, Orientation::for_device);
    }
    draft.sources.device = device;
}

fn discovery_error(state: &mut FastFill, error: &str) -> Task<Message> {
    if error == fastfill_rust::camera::stream::INTERRUPTED {
        super::lifecycle::pause(
            state,
            "Camera paused after sleep; refresh cameras, then Resume preview.",
        )
    } else {
        state.camera.draft.as_mut().unwrap().error = Some(error.into());
        Task::none()
    }
}

fn load_modes(state: &mut FastFill) -> Task<Message> {
    let id = state.project.request_id();
    let draft: &mut Draft = state.camera.draft.as_mut().unwrap();
    reset(draft, id);
    draft.sources.modes.clear();
    draft.sources.mode = None;
    draft.sources.loading = true;
    Task::done(Message::Camera(Event::Discover(
        id,
        Some(draft.sources.device.as_ref().unwrap().index),
    )))
}

fn discover(draft: &mut Draft, index: Option<u32>) -> Task<Message> {
    // The modal owns discovery just like preview; dropping it kills the child.
    match super::worker::discover(draft.id, index) {
        Ok((query, task)) => {
            draft.sources.query = Some(query);
            task
        }
        Err(error) => {
            draft.sources.loading = false;
            draft.error = Some(error);
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests;
