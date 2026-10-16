//! Pending page corrections stay separate from committed page/history state.

mod canvas;
mod preview;
mod view;

pub(super) use view::{view, warning};

use super::{FastFill, ImagePicker, Message};
use fastfill_rust::correction::{PageCorrection, corrected_dimensions};
use fastfill_rust::project::{CropQuad, NormalizedPoint, PageFilterMode};
use iced::Task;
use preview::{Input, Original, Preview};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct PageEditor {
    draft: Option<Draft>,
    running: Option<Job>,
    warning: Option<String>,
}

struct Draft {
    id: u64,
    page: uuid::Uuid,
    settings: PageCorrection,
    original: Option<Arc<Original>>,
    ready: Option<Arc<Preview>>,
    error: Option<String>,
    notice: Option<String>,
}

struct Job {
    id: u64,
    session: u64,
    settings: PageCorrection,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Begin,
    Cancel,
    Accept,
    FullFrame,
    Rotate(i32),
    Mode(PageFilterMode),
    Brightness(i32),
    Contrast(i32),
    Corner(usize, NormalizedPoint),
    Ready(u64, Result<Arc<Preview>, String>),
    DismissWarning,
}

impl PageEditor {
    pub(super) const fn is_open(&self) -> bool {
        self.draft.is_some()
    }
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if let Message::PageEdit(event) = message {
        return Some(match event {
            Event::Begin => begin(state),
            Event::Ready(id, result) => ready(state, *id, result),
            Event::DismissWarning => {
                state.page_editor.warning = None;
                Task::none()
            }
            _ if state.page_editor.is_open() => edit(state, event),
            _ => Task::none(),
        });
    }
    if !state.page_editor.is_open() {
        if matches!(
            message,
            Message::Undo
                | Message::Redo
                | Message::NewProject
                | Message::OpenProject
        ) {
            state.page_editor.warning = None;
        }
        return None;
    }
    match message {
        Message::CancelInteraction => cancel(state),
        Message::FocusLost | Message::ViewportResized => {
            state.input_epoch = state.input_epoch.wrapping_add(1);
            state.modifiers = super::DragModifiers::default();
        }
        Message::ModifiersChanged(modifiers) => state.modifiers = *modifiers,
        Message::CloseRequested(_) => {
            if let Some(draft) = &mut state.page_editor.draft {
                draft.notice =
                    Some("Accept or cancel page edits before closing.".into());
            }
        }
        _ => {}
    }
    // Main document controls cannot mutate the page behind the editor.
    Some(Task::none())
}

fn begin(state: &mut FastFill) -> Task<Message> {
    if state.page_editor.is_open()
        || !state.project.is_idle()
        || state.image_picker != ImagePicker::Idle
        || state.image().is_none()
        || state.confirm_delete_all
        || state.confirm_delete_page
        || state.dragged_page.is_some()
    {
        return Task::none();
    }
    state.accept_text();
    state.cancel_interaction();
    state.page_editor.warning = None;
    state.page_editor.draft = Some(Draft {
        id: state.project.request_id(),
        page: state.pages.current().metadata.id,
        settings: PageCorrection::from(&state.pages.current().metadata),
        original: None,
        ready: None,
        error: None,
        notice: None,
    });
    schedule(state)
}

fn cancel(state: &mut FastFill) {
    state.page_editor.draft = None;
    state.input_epoch = state.input_epoch.wrapping_add(1);
    state.status = "Page edits canceled; page unchanged.".into();
    state.status_is_error = false;
}

fn edit(state: &mut FastFill, event: &Event) -> Task<Message> {
    if matches!(event, Event::Cancel) {
        cancel(state);
        return Task::none();
    }
    if matches!(event, Event::Accept) {
        accept(state);
        return Task::none();
    }
    let draft = state.page_editor.draft.as_mut().expect("active page edit");
    let mut settings = draft.settings;
    match event {
        Event::FullFrame => settings.crop = CropQuad::default(),
        Event::Rotate(delta) => {
            settings.rotation_quarter_turns =
                (settings.rotation_quarter_turns.rem_euclid(4)
                    + delta.rem_euclid(4))
                .rem_euclid(4);
        }
        Event::Mode(mode) => settings.filter.mode = *mode,
        Event::Brightness(value) => settings.brightness = *value,
        Event::Contrast(value) => settings.filter.contrast = *value,
        Event::Corner(index, point) => {
            let corners = [
                &mut settings.crop.top_left,
                &mut settings.crop.top_right,
                &mut settings.crop.bottom_right,
                &mut settings.crop.bottom_left,
            ];
            let Some(corner) = corners.into_iter().nth(*index) else {
                return Task::none();
            };
            *corner = *point;
        }
        _ => return Task::none(),
    }
    let metadata = &state.pages.current().metadata;
    let valid = corrected_dimensions(
        u32::try_from(metadata.source_pixel_width).unwrap_or(0),
        u32::try_from(metadata.source_pixel_height).unwrap_or(0),
        settings,
    );
    if let Err(error) = valid {
        draft.error = Some(error.to_string());
        return Task::none();
    }
    draft.error = None;
    draft.notice = None;
    draft.settings = settings;
    schedule(state)
}

fn schedule(state: &mut FastFill) -> Task<Message> {
    let Some(draft) = &state.page_editor.draft else {
        return Task::none();
    };
    if state.page_editor.running.is_some()
        || draft.error.is_some()
        || draft
            .ready
            .as_ref()
            .is_some_and(|p| p.settings == draft.settings)
    {
        return Task::none();
    }
    let input = draft.original.as_ref().map_or_else(
        || {
            Input::Image(
                state.image().expect("page has an image").handle.clone(),
            )
        },
        |original| Input::Decoded(original.clone()),
    );
    let job = Job {
        id: state.project.request_id(),
        session: draft.id,
        settings: draft.settings,
    };
    let task = preview::task(job.id, input, job.settings);
    state.page_editor.running = Some(job);
    task
}

fn ready(
    state: &mut FastFill,
    id: u64,
    result: &Result<Arc<Preview>, String>,
) -> Task<Message> {
    if state
        .page_editor
        .running
        .as_ref()
        .is_none_or(|job| job.id != id)
    {
        return Task::none();
    }
    let job = state
        .page_editor
        .running
        .take()
        .expect("matching preview job");
    if let Some(draft) = &mut state.page_editor.draft
        && draft.id == job.session
    {
        match result {
            Ok(preview) => {
                draft.original = Some(preview.original.clone());
                if draft.settings == job.settings {
                    draft.ready = Some(preview.clone());
                    draft.error = None;
                }
            }
            Err(error) if draft.settings == job.settings => {
                draft.error = Some(error.clone());
            }
            Err(_) => {}
        }
    }
    schedule(state)
}

fn accept(state: &mut FastFill) {
    let draft = state.page_editor.draft.as_ref().expect("active edit");
    let Some(preview) = draft.ready.as_ref().filter(|preview| {
        preview.settings == draft.settings && draft.error.is_none()
    }) else {
        return;
    };
    if state.pages.current().metadata.id != draft.page {
        return;
    }
    let old_size = state.pages.current().size;
    match state
        .pages
        .current_mut()
        .accept_correction(draft.settings, preview.handle.clone())
    {
        Ok(moved) => {
            state.page_editor.draft = None;
            if moved > 0 {
                state.page_editor.warning = Some(format!(
                    "{moved} annotation(s) moved back within reach. \
                     Undo restores their positions."
                ));
            }
            if state.pages.current().size != old_size {
                state.navigation.fit();
            }
            state.input_epoch = state.input_epoch.wrapping_add(1);
            state.status = "Page corrections accepted.".into();
            state.status_is_error = false;
        }
        Err(error) => {
            state.page_editor.draft.as_mut().expect("active edit").error =
                Some(error);
        }
    }
}

#[cfg(test)]
mod tests;
