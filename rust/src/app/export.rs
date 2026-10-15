//! PDF export changes files, never the project's saved identity.

use super::export_io::{ExportReceipt, pick_and_export};
use super::project::Operation;
use super::{FastFill, ImagePicker, Message};
use crate::page_canvas::export::ExportPage;
use iced::Task;
#[cfg(not(target_arch = "wasm32"))]
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    match message {
        Message::ExportPdf => Some(begin(state)),
        Message::CancelPdfExport => {
            if let Operation::ExportingPdf { cancel, .. } =
                &state.project.operation
            {
                // Wait for acknowledgment: a file commit cannot be revoked.
                cancel.store(true, Ordering::Relaxed);
                state.status = "Canceling PDF export…".into();
            }
            Some(Task::none())
        }
        Message::PdfExportProgress(id, done, total) => {
            if let Operation::ExportingPdf {
                id: active,
                progress,
                cancel,
            } = &mut state.project.operation
                && *active == *id
                && !cancel.load(Ordering::Relaxed)
            {
                #[allow(clippy::cast_precision_loss)]
                {
                    *progress =
                        (*done as f32 / (*total).max(1) as f32).clamp(0.0, 1.0);
                }
                state.status = if done == total {
                    "Finishing PDF and writing file…".into()
                } else {
                    format!("Exporting PDF: {done} of {total} pages ready…")
                };
            }
            Some(Task::none())
        }
        Message::PdfExported(id, result) => {
            if matches!(state.project.operation,
                Operation::ExportingPdf { id: active, .. } if active == *id)
            {
                finish(state, result);
            }
            Some(Task::none())
        }
        _ => None,
    }
}

fn begin(state: &mut FastFill) -> Task<Message> {
    if !state.project.is_idle()
        || state.image_picker != ImagePicker::Idle
        || state.confirm_delete_all
        || state.confirm_delete_page
        || state.dragged_page.is_some()
    {
        return Task::none();
    }
    state.accept_text();
    state.cancel_interaction();
    let pages = state
        .pages
        .iter()
        .map(|page| {
            ExportPage::new(
                page.size,
                page.image
                    .as_ref()
                    .map(|image| image.display_handle().clone()),
                &page.annotations,
            )
        })
        .collect();
    let id = state.project.request_id();
    let cancel = Arc::new(AtomicBool::new(false));
    state.project.operation = Operation::ExportingPdf {
        id,
        cancel: Arc::clone(&cancel),
        progress: 0.0,
    };
    state.status = "Choose a PDF destination…".into();
    state.status_is_error = false;
    let directory = state
        .export_directory
        .clone()
        .or_else(|| state.project.directory.clone())
        .or_else(|| state.image_directory.clone());
    export_task(id, pages, cancel, directory, state.project.name.clone())
}

fn finish(
    state: &mut FastFill,
    result: &Result<Option<ExportReceipt>, String>,
) {
    state.project.operation = Operation::Idle;
    state.status_is_error = false;
    match result {
        Ok(Some(receipt)) => {
            if let Some(directory) = &receipt.directory {
                state.export_directory = Some(directory.clone());
            }
            state.status = format!("Exported {}.", receipt.name);
        }
        Ok(None) => state.status = "PDF export canceled.".into(),
        Err(error) => {
            state.status.clone_from(error);
            state.status_is_error = true;
        }
    }
}

fn export_task(
    id: u64,
    pages: Vec<ExportPage>,
    cancel: Arc<AtomicBool>,
    directory: Option<PathBuf>,
    name: String,
) -> Task<Message> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let stream = iced::futures::stream::once(async move {
            let (sender, receiver) = mpsc::unbounded();
            let completion = sender.clone();
            let worker = std::thread::Builder::new()
                .name("fastfill-pdf-export".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(
                        std::panic::AssertUnwindSafe(|| {
                            iced::futures::executor::block_on(pick_and_export(
                                pages,
                                directory,
                                name,
                                cancel,
                                id,
                                completion.clone(),
                            ))
                        }),
                    )
                    .unwrap_or_else(|_| {
                        Err("PDF renderer failed; export canceled.".into())
                    });
                    let _ = completion
                        .unbounded_send(Message::PdfExported(id, result));
                });
            if let Err(error) = worker {
                let _ = sender.unbounded_send(Message::PdfExported(
                    id,
                    Err(format!("Could not start PDF export: {error}")),
                ));
            }
            receiver
        })
        .flatten();
        Task::run(stream, std::convert::identity)
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Browser harness has no worker runtime yet, as with PDF import.
        let (sender, receiver) = mpsc::unbounded();
        Task::batch([
            Task::run(receiver, std::convert::identity),
            Task::perform(
                pick_and_export(pages, directory, name, cancel, id, sender),
                move |result| Message::PdfExported(id, result),
            ),
        ])
    }
}

#[cfg(test)]
mod tests;
