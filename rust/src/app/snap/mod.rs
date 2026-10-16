//! Session-global snapping; immutable displayed pixels identify cached results.

#[cfg(test)]
mod tests;
mod worker;

use super::{FastFill, ImagePicker, Message};
use fastfill_rust::annotations::{Pointer, Tool};
use fastfill_rust::form_snap::FormFeatures;
use fastfill_rust::geometry::{Point, Rect};
use fastfill_rust::project::NormalizedPoint;
use iced::Task;
use iced::advanced::image::Id;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

type ImageKey = (Uuid, Id);
type Features = Result<Arc<FormFeatures>, String>;
const SNAP_RADIUS: f32 = 0.04;

#[derive(Default)]
pub(super) struct SmartSnap {
    pub(super) enabled: bool,
    cache: BTreeMap<ImageKey, Features>,
    running: Option<Job>,
}

struct Job {
    id: u64,
    key: ImageKey,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Toggle,
    Ready(u64, Features),
}

pub(super) fn supports(tool: Tool) -> bool {
    matches!(tool, Tool::Text | Tool::Checkmark | Tool::Cross)
}

fn key(state: &FastFill) -> Option<ImageKey> {
    state
        .image()
        .map(|image| (state.project.header.id, image.display_handle().id()))
}

fn available(state: &FastFill) -> bool {
    state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.page_editor.is_open()
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none()
}

pub(super) fn can_toggle(state: &FastFill) -> bool {
    available(state) && state.annotations().active_pointer().is_none()
}

pub(super) fn update(state: &mut FastFill, message: &Message) -> bool {
    match message {
        Message::SmartSnap(Event::Toggle) => {
            if can_toggle(state) {
                state.smart_snap.enabled = !state.smart_snap.enabled;
                if state.smart_snap.enabled
                    && let Some(key) = key(state)
                    && state
                        .smart_snap
                        .cache
                        .get(&key)
                        .is_some_and(Result::is_err)
                {
                    state.smart_snap.cache.remove(&key);
                }
            }
        }
        Message::SmartSnap(Event::Ready(id, result)) => {
            if state
                .smart_snap
                .running
                .as_ref()
                .is_some_and(|j| j.id == *id)
            {
                let job = state.smart_snap.running.take().unwrap();
                if live(state, job.key) {
                    state.smart_snap.cache.insert(job.key, result.clone());
                }
            }
        }
        _ => return false,
    }
    true
}

fn live(state: &FastFill, key: ImageKey) -> bool {
    key.0 == state.project.header.id
        && state.pages.iter().any(|page| {
            page.image
                .as_ref()
                .is_some_and(|image| image.display_handle().id() == key.1)
        })
}

pub(super) fn synchronize(state: &mut FastFill) -> Task<Message> {
    let project = state.project.header.id;
    let pages = &state.pages;
    state.smart_snap.cache.retain(|key, _| {
        key.0 == project
            && pages.iter().any(|page| {
                page.image
                    .as_ref()
                    .is_some_and(|image| image.display_handle().id() == key.1)
            })
    });
    if !state.smart_snap.enabled
        || !supports(state.annotations().tool())
        || !available(state)
        || state.smart_snap.running.is_some()
    {
        return Task::none();
    }
    let Some(key) = key(state) else {
        return Task::none();
    };
    if state.smart_snap.cache.contains_key(&key) {
        return Task::none();
    }
    let input = state.image().unwrap().display_handle().clone();
    let id = state.project.request_id();
    state.smart_snap.running = Some(Job { id, key });
    Task::perform(worker::run(input), move |result| {
        Message::SmartSnap(Event::Ready(id, result))
    })
}

pub(super) fn status(state: &FastFill) -> String {
    if !state.smart_snap.enabled {
        return "Off".into();
    }
    let Some(key) = key(state) else {
        return "No image".into();
    };
    match state.smart_snap.cache.get(&key) {
        Some(Ok(_)) => "Ready".into(),
        Some(Err(error)) => format!("Unavailable: {error}"),
        None => "Analyzing…".into(),
    }
}

fn features(state: &FastFill) -> Option<&FormFeatures> {
    if !state.smart_snap.enabled {
        return None;
    }
    state
        .smart_snap
        .cache
        .get(&key(state)?)?
        .as_ref()
        .ok()
        .map(Arc::as_ref)
}

fn normalized(state: &FastFill, point: Point) -> NormalizedPoint {
    let size = super::page_size(state);
    NormalizedPoint {
        x: point.x / size.width,
        y: point.y / size.height,
    }
}

pub(super) fn place_mark(
    state: &mut FastFill,
    pointer: Pointer,
    point: Point,
    scale: f32,
) {
    if !matches!(state.annotations().tool(), Tool::Checkmark | Tool::Cross) {
        return;
    }
    let Some(bounds) = features(state).and_then(|features| {
        features.find_box(normalized(state, point), SNAP_RADIUS)
    }) else {
        return;
    };
    let page = super::page_size(state);
    let side =
        (bounds.width * page.width).min(bounds.height * page.height) * 0.78;
    let center = Point::new(
        (bounds.x + bounds.width / 2.0) * page.width,
        (bounds.y + bounds.height / 2.0) * page.height,
    );
    state.annotations_mut().set_mark_snap(
        pointer,
        Rect::new(center.x - side / 2.0, center.y - side / 2.0, side, side),
        8.0 / scale,
    );
}

/// Start text placement; return whether a snapped field's text was reopened.
pub(super) fn begin_text(
    state: &mut FastFill,
    point: Point,
    scale: f32,
) -> bool {
    let size = super::page_size(state);
    let anchor = features(state)
        .and_then(|features| {
            features.find_horizontal_line(normalized(state, point), SNAP_RADIUS)
        })
        .map(|line| {
            Point::new(
                line.start.x * size.width,
                line.start.y.midpoint(line.end.y) * size.height,
            )
        });
    let reused = anchor.is_some_and(|anchor| {
        state
            .annotations_mut()
            .begin_text_edit_on_line(point, anchor, size, scale)
    });
    if !reused
        && state.annotations_mut().begin_text(point, size, scale)
        && let Some(anchor) = anchor
    {
        state.annotations_mut().set_text_line_anchor(anchor);
    }
    reused
}
