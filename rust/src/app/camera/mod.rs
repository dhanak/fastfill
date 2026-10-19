//! Manual capture is a modal draft; only Use photo changes the document.

mod framing;
mod lifecycle;
mod options;
mod orientation;
mod overlay;
mod view;
mod worker;

use super::{FastFill, ImagePicker, Message};
use crate::picker::ImportedImage;
use fastfill_rust::camera::{Spec, catalog::Device};
use fastfill_rust::document_detection::DocumentDetection;
use fastfill_rust::project::CropQuad;
use iced::Task;
use iced::widget::image::Handle;
use iced_runtime::image::Allocation;
use image::RgbaImage;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime};

pub(super) use view::view;

#[derive(Default)]
pub(super) struct CameraScreen {
    draft: Option<Draft>,
    analysis: Option<framing::Job>,
}

struct Draft {
    id: u64,
    sources: options::Sources,
    orientation: orientation::Orientation,
    paused: Option<&'static str>,
    running: Option<worker::Running>,
    last: Option<Frame>,
    crop: Option<CropQuad>,
    tracking: framing::Tracking,
    captured: bool,
    saving: bool,
    error: Option<String>,
    pending: Arc<AtomicBool>,
}

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pixels: Arc<RgbaImage>,
    allocation: Allocation,
    arrived: Instant,
    wall_time: SystemTime,
}

impl Frame {
    fn is_recent(&self) -> bool {
        self.wall_time
            .elapsed()
            .is_ok_and(|age| age <= framing::MAX_AGE)
    }
}

impl Draft {
    fn can_capture(&self) -> bool {
        self.last.is_some() && !self.captured && !self.saving
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Begin,
    Start(u64),
    Refresh,
    Discover(u64, Option<u32>),
    Devices(u64, Result<Vec<Device>, String>),
    Device(Device),
    Modes(u64, Result<Vec<Spec>, String>),
    Mode(Spec),
    Rotate,
    Mirror,
    Pause,
    Frame(u64, Result<Arc<RgbaImage>, String>),
    Uploaded(u64, Result<Frame, String>),
    Framed(u64, Result<DocumentDetection, String>),
    Capture,
    UsePhoto,
    Prepared(u64, Result<ImportedImage, String>),
    Cancel,
}

impl CameraScreen {
    pub(super) const fn is_open(&self) -> bool {
        self.draft.is_some()
    }
}

pub(super) fn can_open(state: &FastFill) -> bool {
    !state.camera.is_open()
        && state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.page_editor.is_open()
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none()
        && state.pages.can_add()
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if let Message::Camera(Event::Framed(id, result)) = message {
        return Some(framing::ready(state, *id, result, Instant::now()));
    }
    if matches!(message, Message::Camera(Event::Begin)) {
        return Some(begin(state));
    }
    if !state.camera.is_open() {
        return matches!(message, Message::Camera(_)).then(Task::none);
    }
    if matches!(message, Message::CloseRequested(_)) {
        state.camera.draft = None;
        // Continue through normal unsaved-project/close handling.
        return None;
    }
    // Recovery completion still owns its unchanged document snapshot.
    if matches!(message, Message::Recovery(_)) {
        return None;
    }
    if let Message::Camera(event) = message
        && let Some(task) = options::update(state, event)
            .or_else(|| lifecycle::update(state, event))
    {
        return Some(task);
    }
    let draft = state.camera.draft.as_mut().unwrap();
    match message {
        Message::Camera(Event::Cancel) | Message::CancelInteraction => {
            state.camera.draft = None;
            state.status = "Capture canceled; project unchanged.".into();
            state.status_is_error = false;
        }
        Message::FocusLost => {
            // Moving a window can lose focus; it must not stop the camera.
            state.modifiers = super::DragModifiers::default();
        }
        Message::Camera(Event::Start(id))
            if *id == draft.id && !draft.saving && !draft.sources.loading =>
        {
            return Some(start(state));
        }
        Message::Camera(Event::Frame(id, result))
            if *id == draft.id && !draft.captured && !draft.saving =>
        {
            if draft.last.as_ref().is_some_and(|frame| !frame.is_recent()) {
                // A blocked window must reset tracking, not stop preview.
                draft.tracking = framing::Tracking::default();
            }
            match result {
                Ok(pixels) => {
                    return Some(upload(
                        *id,
                        pixels,
                        draft.orientation.mirrored,
                    ));
                }
                Err(error) => {
                    if error == fastfill_rust::camera::stream::INTERRUPTED {
                        return Some(lifecycle::pause(
                            state,
                            "Camera paused after sleep or an interruption.",
                        ));
                    }
                    let id = state.project.request_id();
                    reset(draft, id);
                    draft.error = Some(error.clone());
                }
            }
        }
        Message::Camera(Event::Uploaded(id, result))
            if *id == draft.id && !draft.captured && !draft.saving =>
        {
            // Keep one frame in flight through GPU upload, not just delivery.
            draft.pending.store(false, Ordering::Relaxed);
            match result {
                Ok(frame) => {
                    draft.last = Some(frame.clone());
                    draft.error = None;
                }
                Err(error) => draft.error = Some(error.clone()),
            }
        }
        Message::Camera(Event::Capture) if draft.can_capture() => {
            draft.running = None;
            draft.captured = true;
            // Manual capture freezes the displayed image, never an old crop.
            draft.crop = None;
            draft.tracking = framing::Tracking::default();
        }
        Message::Camera(Event::UsePhoto) if draft.captured && !draft.saving => {
            draft.saving = true;
            draft.error = None;
            let id = draft.id;
            let pixels = draft.last.as_ref().unwrap().pixels.clone();
            return Some(worker::prepare_task(id, pixels));
        }
        Message::Camera(Event::Prepared(id, result))
            if *id == draft.id && draft.saving =>
        {
            return Some(prepared(state, result));
        }
        _ => {}
    }
    // No editing/project command can mutate the document behind the camera.
    Some(framing::schedule(state))
}

fn upload(id: u64, pixels: &Arc<RgbaImage>, mirrored: bool) -> Task<Message> {
    // Camera review keeps its display orientation; source pixels stay normal.
    let rgba = if mirrored {
        image::imageops::flip_horizontal(pixels.as_ref()).into_raw()
    } else {
        pixels.as_raw().clone()
    };
    let handle = Handle::from_rgba(pixels.width(), pixels.height(), rgba);
    let pixels = pixels.clone();
    let arrived = Instant::now();
    let wall_time = SystemTime::now();
    // Large handles upload asynchronously; don't show a not-yet-ready image.
    iced_runtime::image::allocate(handle).map(move |result| {
        Message::Camera(Event::Uploaded(
            id,
            result
                .map(|allocation| Frame {
                    pixels: pixels.clone(),
                    allocation,
                    arrived,
                    wall_time,
                })
                .map_err(|error| {
                    format!("Could not display camera frame: {error}")
                }),
        ))
    })
}

fn begin(state: &mut FastFill) -> Task<Message> {
    if !can_open(state) {
        return Task::none();
    }
    state.accept_text();
    state.cancel_interaction();
    state.camera.draft = Some(Draft {
        id: state.project.request_id(),
        sources: options::Sources::default(),
        orientation: orientation::Orientation::default(),
        paused: None,
        running: None,
        last: None,
        crop: None,
        tracking: framing::Tracking::default(),
        captured: false,
        saving: false,
        error: None,
        pending: Arc::default(),
    });
    options::refresh(state)
}

fn prepared(
    state: &mut FastFill,
    result: &Result<ImportedImage, String>,
) -> Task<Message> {
    match result {
        Ok(image) if state.pages.can_add() => {
            let crop = state.camera.draft.as_ref().unwrap().crop;
            state.pages.add_image(image.clone());
            state.camera.draft = None;
            state.navigation.fit();
            state.page_scroll =
                super::rail::slot_top(state.pages.active_index());
            state.input_epoch = state.input_epoch.wrapping_add(1);
            state.status = "Photo added. Adjust framing if needed.".into();
            state.status_is_error = false;
            super::page_edit::begin(state, crop)
        }
        result => {
            let draft = state.camera.draft.as_mut().unwrap();
            draft.saving = false;
            draft.error = Some(match result {
                Err(error) => error.clone(),
                Ok(_) => "A project may contain at most 20 pages.".into(),
            });
            Task::none()
        }
    }
}

fn start(state: &mut FastFill) -> Task<Message> {
    let id = state.project.request_id();
    let draft = state.camera.draft.as_mut().unwrap();
    let (Some(device), Some(mode)) =
        (draft.sources.device.as_ref(), draft.sources.mode)
    else {
        draft.error = Some("Choose a camera and supported mode first.".into());
        return Task::none();
    };
    let index = device.index;
    reset(draft, id);
    draft.paused = None;
    match worker::start(
        index,
        mode,
        draft.orientation,
        id,
        draft.pending.clone(),
    ) {
        Ok((running, task)) => {
            draft.running = Some(running);
            task
        }
        Err(error) => {
            draft.error = Some(error);
            Task::none()
        }
    }
}

fn reset(draft: &mut Draft, id: u64) {
    draft.running = None;
    draft.sources.query = None;
    draft.id = id;
    draft.last = None;
    draft.crop = None;
    draft.tracking = framing::Tracking::default();
    draft.captured = false;
    draft.error = None;
    draft.pending = Arc::default();
    // Keep orientation and explicit pause intent across configuration changes.
}

#[cfg(test)]
mod tests;
