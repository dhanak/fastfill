//! Project identity and transactional file-workflow state.

use super::pages::Pages;
use super::project_io::{OpenedProject, SaveReceipt, SaveSnapshot};
use super::{FastFill, ImagePicker, Message};
use fastfill_rust::project::{Project, encode_editor_page};
use iced::Task;
use std::path::PathBuf;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub(super) enum LeaveAction {
    New,
    Open,
    Close(iced::window::Id),
}

#[derive(Debug, Default)]
pub(super) enum Operation {
    #[default]
    Idle,
    #[cfg(not(target_arch = "wasm32"))]
    Recovering,
    #[cfg(not(target_arch = "wasm32"))]
    ClosingRecovery,
    Confirm(LeaveAction),
    Opening(u64),
    ImportingPdf {
        id: u64,
        cancel: Arc<std::sync::atomic::AtomicBool>,
        progress: f32,
    },
    ExportingPdf {
        id: u64,
        cancel: Arc<std::sync::atomic::AtomicBool>,
        progress: f32,
    },
    Saving {
        id: u64,
        identity: Vec<(Uuid, u64)>,
        leave: Option<LeaveAction>,
        updated: OffsetDateTime,
    },
}

impl Operation {
    #[cfg(test)]
    fn request_id(&self) -> Option<u64> {
        match self {
            Self::Opening(id) | Self::Saving { id, .. } => Some(*id),
            _ => None,
        }
    }
}

pub(super) struct ProjectSession {
    pub(super) header: Project,
    pub(super) name: String,
    pub(super) path: Option<PathBuf>,
    pub(super) directory: Option<PathBuf>,
    pub(super) operation: Operation,
    saved: Option<Vec<(Uuid, u64)>>,
    next_request: u64,
}

impl Default for ProjectSession {
    fn default() -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            header: Project {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Untitled document".into(),
                created_utc: now,
                updated_utc: now,
                pages: Vec::new(),
            },
            name: "Untitled.docscan".into(),
            path: None,
            directory: None,
            operation: Operation::Idle,
            saved: None,
            next_request: 0,
        }
    }
}

impl ProjectSession {
    pub(super) fn is_idle(&self) -> bool {
        matches!(self.operation, Operation::Idle)
    }

    pub(super) fn is_dirty(&self, pages: &Pages) -> bool {
        self.saved.as_ref().map_or_else(
            || {
                pages.iter().any(|page| {
                    page.image.is_some() || !page.annotations.is_empty()
                })
            },
            |saved| *saved != pages.identity(),
        )
    }

    pub(super) fn request_id(&mut self) -> u64 {
        self.next_request += 1;
        self.next_request
    }
}

impl FastFill {
    pub(super) fn project_snapshot(&self) -> Result<SaveSnapshot, String> {
        let mut project = self.project.header.clone();
        project.updated_utc = OffsetDateTime::now_utc();
        project.pages = self
            .pages
            .iter()
            .map(|page| {
                encode_editor_page(&page.metadata, &page.annotations)
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(SaveSnapshot {
            project,
            images: self
                .pages
                .iter()
                .map(|page| {
                    page.image.as_ref().map(|image| image.handle.clone())
                })
                .collect(),
        })
    }

    fn request_leave(&mut self, action: LeaveAction) -> Task<Message> {
        self.accept_text();
        self.cancel_interaction();
        if self.project.is_dirty(&self.pages) {
            self.project.operation = Operation::Confirm(action);
            Task::none()
        } else {
            self.leave_project(action)
        }
    }

    fn leave_project(&mut self, action: LeaveAction) -> Task<Message> {
        match action {
            LeaveAction::New => {
                let directory = self.project.directory.clone();
                // Preserve request numbering across document replacements.
                let next_request = self.project.next_request;
                self.project = ProjectSession {
                    directory,
                    next_request,
                    ..ProjectSession::default()
                };
                self.pages = Pages::default();
                self.navigation.fit();
                self.page_scroll = 0.0;
                self.cancel_interaction();
                self.status = "New project.".into();
                self.status_is_error = false;
                Task::none()
            }
            LeaveAction::Open => {
                let id = self.project.request_id();
                self.project.operation = Operation::Opening(id);
                self.status = "Opening project…".into();
                self.status_is_error = false;
                Task::perform(
                    super::project_io::open_project(
                        self.project.directory.clone(),
                    ),
                    move |result| Message::ProjectOpened(id, result),
                )
            }
            LeaveAction::Close(id) => {
                #[cfg(not(target_arch = "wasm32"))]
                return super::recovery::close(self, id);
                #[cfg(target_arch = "wasm32")]
                iced::window::close(id)
            }
        }
    }

    fn save_project(
        &mut self,
        save_as: bool,
        leave: Option<LeaveAction>,
    ) -> Task<Message> {
        self.accept_text();
        self.cancel_interaction();
        self.project.operation = Operation::Idle;
        let snapshot = match self.project_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = error;
                self.status_is_error = true;
                return Task::none();
            }
        };
        let id = self.project.request_id();
        self.project.operation = Operation::Saving {
            id,
            identity: self.pages.identity(),
            leave,
            updated: snapshot.project.updated_utc,
        };
        self.status = "Saving project…".into();
        self.status_is_error = false;
        let path = if save_as {
            None
        } else {
            self.project.path.clone()
        };
        Task::perform(
            super::project_io::save_project(
                snapshot,
                path,
                self.project.directory.clone(),
                self.project.name.clone(),
            ),
            move |result| Message::ProjectSaved(id, result),
        )
    }

    fn project_opened(
        &mut self,
        id: u64,
        result: &Result<Option<Arc<OpenedProject>>, String>,
    ) {
        let Operation::Opening(active) = self.project.operation else {
            return;
        };
        if active != id {
            return;
        }
        self.project.operation = Operation::Idle;
        match result {
            Ok(Some(opened)) => match Pages::from_opened(opened) {
                Ok(pages) => {
                    let mut header = opened.project.clone();
                    header.pages.clear();
                    self.project.header = header;
                    self.project.path.clone_from(&opened.path);
                    self.project.name.clone_from(&opened.name);
                    if let Some(directory) =
                        opened.path.as_ref().and_then(|path| path.parent())
                    {
                        self.project.directory = Some(directory.to_path_buf());
                        self.image_directory = Some(directory.to_path_buf());
                    }
                    self.project.saved = Some(pages.identity());
                    self.pages = pages;
                    self.navigation.fit();
                    self.page_scroll = 0.0;
                    self.cancel_interaction();
                    self.status = format!(
                        "Opened {} ({} pages).",
                        self.project.name,
                        self.pages.len()
                    );
                    self.status_is_error = false;
                }
                Err(error) => {
                    self.status = error;
                    self.status_is_error = true;
                }
            },
            Ok(None) => {
                self.status = "Open canceled; current project kept.".into();
                self.status_is_error = false;
            }
            Err(error) => {
                self.status.clone_from(error);
                self.status_is_error = true;
            }
        }
    }

    fn project_saved(
        &mut self,
        id: u64,
        result: &Result<Option<SaveReceipt>, String>,
    ) -> Task<Message> {
        let Operation::Saving { id: active, .. } = self.project.operation
        else {
            return Task::none();
        };
        if active != id {
            return Task::none();
        }
        let Operation::Saving {
            identity,
            leave,
            updated,
            ..
        } = std::mem::take(&mut self.project.operation)
        else {
            unreachable!()
        };
        match result {
            Ok(Some(receipt)) => {
                self.project.saved = Some(identity);
                self.project.path.clone_from(&receipt.path);
                self.project.name.clone_from(&receipt.name);
                self.project.header.updated_utc = updated;
                if let Some(directory) =
                    receipt.path.as_ref().and_then(|path| path.parent())
                {
                    self.project.directory = Some(directory.to_path_buf());
                }
                self.status = format!("Saved {}.", receipt.name);
                self.status_is_error = false;
                if let Some(action) = leave {
                    return self.leave_project(action);
                }
            }
            Ok(None) => {
                self.status = "Save canceled; current project kept.".into();
                self.status_is_error = false;
            }
            Err(error) => {
                self.status.clone_from(error);
                self.status_is_error = true;
            }
        }
        Task::none()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn restore_project(
        &mut self,
        opened: &OpenedProject,
    ) -> Result<(), String> {
        let pages = Pages::from_opened(opened)?;
        let mut header = opened.project.clone();
        header.pages.clear();
        self.project.header = header;
        self.project.path = None;
        self.project.name.clone_from(&opened.name);
        self.project.saved = None;
        self.project.operation = Operation::Idle;
        self.pages = pages;
        self.navigation.fit();
        self.page_scroll = 0.0;
        self.cancel_interaction();
        self.status =
            "Recovered unsaved work. Save to keep a project copy.".into();
        self.status_is_error = false;
        Ok(())
    }
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    match message {
        Message::ProjectOpened(id, result) => {
            state.project_opened(*id, result);
            return Some(Task::none());
        }
        Message::ProjectSaved(id, result) => {
            return Some(state.project_saved(*id, result));
        }
        _ => {}
    }
    if !state.project.is_idle() {
        if let Operation::Confirm(action) = state.project.operation {
            match message {
                Message::DiscardProject => {
                    state.project.operation = Operation::Idle;
                    return Some(state.leave_project(action));
                }
                Message::SaveBeforeLeave => {
                    return Some(state.save_project(false, Some(action)));
                }
                Message::KeepProject | Message::CancelInteraction => {
                    state.project.operation = Operation::Idle;
                }
                _ => {}
            }
        }
        if matches!(message, Message::FocusLost) {
            state.cancel_interaction();
            state.modifiers = super::DragModifiers::default();
        }
        if let Message::ModifiersChanged(modifiers) = message {
            // A command-key release during I/O must not leave Ctrl stuck.
            state.modifiers = *modifiers;
        }
        return Some(Task::none());
    }
    let action = match message {
        Message::NewProject => Some(LeaveAction::New),
        Message::OpenProject => Some(LeaveAction::Open),
        Message::CloseRequested(id) => Some(LeaveAction::Close(*id)),
        Message::SaveProject | Message::SaveProjectAs => None,
        Message::KeepProject
        | Message::DiscardProject
        | Message::SaveBeforeLeave => return Some(Task::none()),
        _ => return None,
    };
    if state.image_picker != ImagePicker::Idle
        || state.confirm_delete_all
        || state.confirm_delete_page
        || state.dragged_page.is_some()
    {
        return Some(Task::none());
    }
    Some(if let Some(action) = action {
        state.request_leave(action)
    } else {
        state.save_project(matches!(message, Message::SaveProjectAs), None)
    })
}

#[cfg(test)]
mod tests;
