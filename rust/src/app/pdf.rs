//! Transactional PDF import: prepare in a worker, install the entire batch.

use super::project::Operation;
use super::{FastFill, ImagePicker, Message};
use crate::picker::ImportedImage;
use fastfill_rust::pdf_import::{
    MAX_PDF_BYTES, PdfStage, import_pdf_with_progress, validate_pdf_length,
};
use iced::Task;
#[cfg(not(target_arch = "wasm32"))]
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub(crate) struct PdfBatch {
    images: Vec<ImportedImage>,
    directory: Option<PathBuf>,
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    match message {
        Message::ImportPdf => Some(begin(state)),
        Message::CancelPdfImport => {
            if let Operation::ImportingPdf { cancel, .. } =
                &state.project.operation
            {
                cancel.store(true, Ordering::Relaxed);
                state.project.operation = Operation::Idle;
                state.status =
                    "PDF import canceled; current pages kept.".into();
                state.status_is_error = false;
            }
            Some(Task::none())
        }
        Message::PdfProgress(id, done, total) => {
            if active(state, *id) {
                set_progress(state, *done, *total, 0.0);
                state.status =
                    format!("Importing PDF: {done} of {total} pages ready…");
            }
            Some(Task::none())
        }
        Message::PdfStage(id, done, total, phase) => {
            if active(state, *id) {
                let (label, step) = match phase {
                    PdfStage::Parsing => ("Reading PDF structure", 0.0),
                    PdfStage::Interpreting => ("Interpreting", 0.0),
                    PdfStage::Rasterizing => ("Rendering", 1.0),
                    PdfStage::Encoding => ("Compressing image", 2.0),
                    PdfStage::Ready => ("Ready", 0.0),
                };
                set_progress(state, *done, *total, step);
                state.status = if *total == 0 {
                    format!("{label}…")
                } else {
                    format!("PDF page {} of {total}: {label}…", done + 1)
                };
            }
            Some(Task::none())
        }
        Message::PdfImported(id, result) => {
            if active(state, *id) {
                finish(state, result);
            }
            Some(Task::none())
        }
        _ => None,
    }
}

#[allow(clippy::cast_precision_loss)]
fn set_progress(state: &mut FastFill, done: usize, total: usize, step: f32) {
    if let Operation::ImportingPdf { progress, .. } =
        &mut state.project.operation
    {
        // Three work stages per page; this is not an elapsed-time estimate.
        *progress = if total == 0 {
            0.0
        } else {
            ((done as f32 * 3.0 + step) / (total as f32 * 3.0)).clamp(0.0, 1.0)
        };
    }
}

fn active(state: &FastFill, id: u64) -> bool {
    matches!(state.project.operation,
        Operation::ImportingPdf { id: current, .. } if id == current)
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
    let available = state.pages.pdf_capacity();
    if available == 0 {
        state.status = "Project already has the maximum of 20 pages.".into();
        state.status_is_error = true;
        return Task::none();
    }
    let id = state.project.request_id();
    let cancel = Arc::new(AtomicBool::new(false));
    state.project.operation = Operation::ImportingPdf {
        id,
        cancel: Arc::clone(&cancel),
        progress: 0.0,
    };
    state.status = "Selecting a PDF…".into();
    state.status_is_error = false;
    import_task(id, available, cancel, state.image_directory.clone())
}

fn finish(
    state: &mut FastFill,
    result: &Result<Option<Arc<PdfBatch>>, String>,
) {
    state.project.operation = Operation::Idle;
    match result {
        Ok(Some(batch)) if state.pages.import_images(&batch.images) => {
            if let Some(directory) = &batch.directory {
                state.image_directory = Some(directory.clone());
            }
            state.navigation.fit();
            state.page_scroll =
                super::rail::slot_top(state.pages.active_index());
            state.cancel_interaction();
            state.status =
                format!("Imported {} PDF pages.", batch.images.len());
            state.status_is_error = false;
        }
        Ok(Some(_)) => {
            state.status =
                "PDF batch is empty or exceeds the 20-page project limit."
                    .into();
            state.status_is_error = true;
        }
        Ok(None) => {
            state.status = "PDF import canceled; current pages kept.".into();
            state.status_is_error = false;
        }
        Err(error) => {
            state.status.clone_from(error);
            state.status_is_error = true;
        }
    }
}

fn import_task(
    id: u64,
    available: usize,
    cancel: Arc<AtomicBool>,
    directory: Option<PathBuf>,
) -> Task<Message> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Start only when polled, not when the controller constructs the task.
        let stream = iced::futures::stream::once(async move {
            let (sender, receiver) = mpsc::unbounded();
            let completion = sender.clone();
            let worker = std::thread::Builder::new()
                .name("fastfill-pdf-import".into())
                .spawn(move || {
                    let result = iced::futures::executor::block_on(pick_pdf(
                        directory,
                        available,
                        cancel,
                        id,
                        completion.clone(),
                    ));
                    let _ = completion
                        .unbounded_send(Message::PdfImported(id, result));
                });
            if let Err(error) = worker {
                let _ = sender.unbounded_send(Message::PdfImported(
                    id,
                    Err(format!("Could not start PDF import: {error}")),
                ));
            }
            receiver
        })
        .flatten();
        Task::run(stream, std::convert::identity)
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Browser harness has no worker runtime yet; rasterization is blocking.
        let (sender, receiver) = mpsc::unbounded();
        Task::batch([
            Task::run(receiver, std::convert::identity),
            Task::perform(
                pick_pdf(directory, available, cancel, id, sender),
                move |result| Message::PdfImported(id, result),
            ),
        ])
    }
}

async fn pick_pdf(
    directory: Option<PathBuf>,
    available: usize,
    cancel: Arc<AtomicBool>,
    id: u64,
    progress: mpsc::UnboundedSender<Message>,
) -> Result<Option<Arc<PdfBatch>>, String> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Import PDF")
        .add_filter("PDF", &["pdf"]);
    if let Some(directory) = directory.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(directory);
    }
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let (bytes, directory) = {
        use std::io::Read;
        let file_path = file.path();
        let mut reader = std::fs::File::open(file_path)
            .map_err(|error| format!("Could not read PDF: {error}"))?
            .take(MAX_PDF_BYTES as u64 + 1);
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read PDF: {error}"))?;
        (bytes, file_path.parent().map(std::path::Path::to_path_buf))
    };
    #[cfg(target_arch = "wasm32")]
    let (bytes, directory) = {
        let limit = u32::try_from(MAX_PDF_BYTES).expect("100 MiB fits u32");
        if file.inner().size() > f64::from(limit) {
            return Err("PDF exceeds the 100 MiB limit.".into());
        }
        (file.read().await, None)
    };
    validate_pdf_length(bytes.len()).map_err(|error| error.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let pages =
        import_pdf_with_progress(bytes, available, |done, total, stage| {
            let message = if stage == PdfStage::Ready {
                Message::PdfProgress(id, done, total)
            } else {
                Message::PdfStage(id, done, total, stage)
            };
            !cancel.load(Ordering::Relaxed)
                && progress.unbounded_send(message).is_ok()
        })
        .map_err(|error| error.to_string())?;
    let images = pages
        .into_iter()
        .enumerate()
        .map(|(index, page)| {
            let mut image = ImportedImage::from_validated(
                format!("{} — page {}.png", file.file_name(), index + 1),
                page.bytes,
                page.info,
                directory.clone(),
            );
            image.set_pixels(page.pixels);
            image
        })
        .collect();
    Ok(Some(Arc::new(PdfBatch { images, directory })))
}

#[cfg(test)]
mod tests;
