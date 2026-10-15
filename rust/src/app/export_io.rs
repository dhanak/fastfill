//! Save-dialog adapter and atomic destination replacement for PDF export.

use super::Message;
use crate::page_canvas::export::{ExportPage, export_pdf};
use iced::futures::channel::mpsc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug)]
pub(crate) struct ExportReceipt {
    pub(super) name: String,
    pub(super) directory: Option<PathBuf>,
}

pub(super) async fn pick_and_export(
    pages: Vec<ExportPage>,
    directory: Option<PathBuf>,
    name: String,
    cancel: Arc<AtomicBool>,
    id: u64,
    progress: mpsc::UnboundedSender<Message>,
) -> Result<Option<ExportReceipt>, String> {
    let name = Path::new(&name)
        .with_extension("pdf")
        .to_string_lossy()
        .into_owned();
    #[cfg(not(target_arch = "wasm32"))]
    let file = {
        let mut dialog = rfd::AsyncFileDialog::new()
            .set_title("Export PDF")
            .set_file_name(&name)
            .add_filter("PDF", &["pdf"]);
        if let Some(directory) = directory.filter(|path| path.is_dir()) {
            dialog = dialog.set_directory(directory);
        }
        let Some(file) = dialog.save_file().await else {
            return Ok(None);
        };
        file
    };
    #[cfg(target_arch = "wasm32")]
    let _ = directory;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    #[cfg(not(target_arch = "wasm32"))]
    validate_destination(file.path())?;
    let result = export_pdf(pages, |done, total| {
        !cancel.load(Ordering::Relaxed)
            && progress
                .unbounded_send(Message::PdfExportProgress(id, done, total))
                .is_ok()
    })
    .await;
    if cancel.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let bytes = result?;
    // Match the import limit so exports remain importable by this app.
    if bytes.len() > fastfill_rust::pdf_import::MAX_PDF_BYTES {
        return Err("Exported PDF exceeds the 100 MiB limit.".into());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match write_pdf(file.path(), &bytes, &cancel) {
            Err(_) if cancel.load(Ordering::Relaxed) => Ok(None),
            Err(error) => Err(error),
            Ok(()) => Ok(Some(ExportReceipt {
                name: file.file_name(),
                directory: file.path().parent().map(Path::to_path_buf),
            })),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        crate::browser_download::download(&name, &bytes, "application/pdf")?;
        Ok(Some(ExportReceipt {
            name,
            directory: None,
        }))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_destination(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        Ok(())
    } else {
        Err("Choose a .pdf destination, not a project or image file.".into())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_pdf(
    path: &Path,
    bytes: &[u8],
    cancel: &AtomicBool,
) -> Result<(), String> {
    use atomicwrites::{AllowOverwrite, AtomicFile};
    use std::io::{Read, Seek, SeekFrom, Write};

    validate_destination(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(true);
    AtomicFile::new(path, AllowOverwrite)
        .write_with_options(
            |file| {
                file.write_all(bytes)?;
                file.flush()?;
                file.seek(SeekFrom::Start(0))?;
                let mut buffer = [0_u8; 8_192];
                for expected in bytes.chunks(buffer.len()) {
                    let actual = &mut buffer[..expected.len()];
                    file.read_exact(actual)?;
                    if actual != expected {
                        return Err(std::io::Error::other(
                            "PDF read-back verification failed.",
                        ));
                    }
                }
                if file.read(&mut buffer[..1])? != 0 {
                    return Err(std::io::Error::other(
                        "PDF read-back length verification failed.",
                    ));
                }
                if cancel.load(Ordering::Relaxed) {
                    return Err(std::io::Error::other("PDF export canceled."));
                }
                Ok(())
            },
            options,
        )
        .map_err(|error| destination_error(path, error))
}

#[cfg(not(target_arch = "wasm32"))]
fn destination_error(
    path: &Path,
    error: atomicwrites::Error<std::io::Error>,
) -> String {
    use atomicwrites::Error;
    use std::io::ErrorKind;

    let reason = match error {
        // Internal errors may include random temporary filenames. Report
        // their kind, with the user's chosen destination, not that path.
        Error::Internal(error) => match error.kind() {
            ErrorKind::PermissionDenied => {
                "Permission denied. Choose a writable folder.".into()
            }
            ErrorKind::ReadOnlyFilesystem => {
                "The destination drive is read-only.".into()
            }
            ErrorKind::StorageFull => "The destination drive is full.".into(),
            ErrorKind::NotFound => {
                "The destination folder was not found.".into()
            }
            kind => std::io::Error::from(kind).to_string(),
        },
        // Our write/verification errors have useful context and no temp path.
        Error::User(error) => error.to_string(),
    };
    format!("Could not export PDF to \"{}\": {reason}", path.display())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn export_error_names_the_destination_not_atomic_write_internals() {
        let path = std::env::temp_dir()
            .join(format!("fastfill-absent-{}", uuid::Uuid::new_v4()))
            .join("form.pdf");
        let message =
            write_pdf(&path, b"PDF", &AtomicBool::new(false)).unwrap_err();
        assert!(message.contains("form.pdf"), "{message}");
        assert!(!message.contains(".atomicwrite"), "{message}");
        assert!(!message.contains("tmpfile"), "{message}");
    }

    #[test]
    fn permission_errors_are_actionable_and_verification_errors_are_kept() {
        let path = Path::new("form.pdf");
        let message = destination_error(
            path,
            atomicwrites::Error::Internal(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "access denied at .atomicwrite123/tmpfile.tmp",
            )),
        );
        assert!(message.contains("Choose a writable folder"));
        assert!(message.contains("form.pdf"));
        assert!(!message.contains(".atomicwrite"));
        let message = destination_error(
            path,
            atomicwrites::Error::User(std::io::Error::other(
                "PDF read-back verification failed.",
            )),
        );
        assert!(message.contains("PDF read-back verification failed"));
    }

    #[test]
    fn atomic_export_preserves_destination_on_cancel_or_invalid_extension() {
        let directory = std::env::temp_dir()
            .join(format!("fastfill-pdf-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("existing.pdf");
        std::fs::write(&path, b"original").unwrap();
        let cancel = AtomicBool::new(true);
        assert!(write_pdf(&path, b"replacement", &cancel).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        let project = directory.join("existing.docscan");
        std::fs::write(&project, b"project").unwrap();
        cancel.store(false, Ordering::Relaxed);
        assert!(write_pdf(&project, b"replacement", &cancel).is_err());
        assert_eq!(std::fs::read(&project).unwrap(), b"project");
        write_pdf(&path, b"replacement", &cancel).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
        assert!(
            write_pdf(&directory.join("absent/file.pdf"), b"", &cancel)
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
