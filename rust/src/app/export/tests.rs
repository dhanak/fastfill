use super::*;
use crate::app::controller::update;
use crate::app::project_io::SaveReceipt;
use crate::app::tests::{annotated_page, paste_text};
use fastfill_rust::annotations::{DragModifiers, Pointer, Tool};
use fastfill_rust::geometry::Point;

fn begin(state: &mut FastFill) -> u64 {
    let _ = update(state, Message::ExportPdf);
    let Operation::ExportingPdf { id, .. } = state.project.operation else {
        panic!("export did not start");
    };
    id
}

fn saved(state: &mut FastFill) {
    let _ = update(state, Message::SaveProject);
    let Operation::Saving { id, .. } = state.project.operation else {
        panic!("save did not start");
    };
    let _ = update(
        state,
        Message::ProjectSaved(
            id,
            Ok(Some(SaveReceipt {
                path: Some(PathBuf::from("/forms/original.docscan")),
                name: "original.docscan".into(),
            })),
        ),
    );
    assert!(!state.project.is_dirty(&state.pages));
}

#[test]
fn successful_export_preserves_project_identity_dirty_state_and_history() {
    for clean in [false, true] {
        let mut state = annotated_page();
        if clean {
            saved(&mut state);
        }
        let identity = state.pages.identity();
        let path = state.project.path.clone();
        let name = state.project.name.clone();
        let viewport =
            fastfill_rust::geometry::Rect::new(0.0, 0.0, 800.0, 600.0);
        state.navigation.zoom_at(
            2.0,
            viewport.center(),
            state.pages.current().size,
            viewport,
        );
        state.navigation.pan_by(Point::new(25.0, -30.0));
        let view = state.navigation.view(state.pages.current().size, viewport);
        let id = begin(&mut state);
        let _ = update(
            &mut state,
            Message::PdfExported(
                id,
                Ok(Some(ExportReceipt {
                    name: "export.pdf".into(),
                    directory: Some(PathBuf::from("/exports")),
                })),
            ),
        );
        assert!(state.project.is_idle());
        assert_eq!(state.pages.identity(), identity);
        assert_eq!(state.project.path, path);
        assert_eq!(state.project.name, name);
        assert_eq!(state.project.is_dirty(&state.pages), !clean);
        assert!(state.annotations().has_selection());
        assert!(state.annotations().can_undo());
        assert_eq!(state.pages.active_index(), 0);
        assert_eq!(
            state.navigation.view(state.pages.current().size, viewport),
            view
        );
        assert_eq!(state.export_directory, Some(PathBuf::from("/exports")));
        assert!(!state.status_is_error);
        assert!(state.status.contains("export.pdf"));
    }
}

#[test]
fn cancel_waits_for_worker_acknowledgment_and_ignores_stale_completions() {
    let mut state = annotated_page();
    let identity = state.pages.identity();
    let id = begin(&mut state);
    let _ = update(&mut state, Message::CancelPdfExport);
    assert!(!state.project.is_idle());
    let Operation::ExportingPdf { ref cancel, .. } = state.project.operation
    else {
        panic!("cancellation must keep edits blocked until acknowledged")
    };
    assert!(cancel.load(Ordering::Relaxed));
    let _ = update(&mut state, Message::PdfExportProgress(id, 1, 2));
    assert!(state.status.contains("Canceling"));
    let _ = update(&mut state, Message::PdfExported(id + 1, Ok(None)));
    assert!(!state.project.is_idle());
    let _ = update(&mut state, Message::PdfExported(id, Ok(None)));
    assert!(state.project.is_idle());
    assert!(!state.status_is_error);
    let current = begin(&mut state);
    let _ = update(&mut state, Message::PdfExported(id, Err("stale".into())));
    assert!(!state.status_is_error);
    assert_eq!(state.pages.identity(), identity);
    let _ = update(
        &mut state,
        Message::PdfExported(current, Err("disk full".into())),
    );
    assert!(state.status_is_error);
    assert_eq!(state.status, "disk full");
    assert!(state.project.is_idle());
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn export_blocks_edits_and_other_io_but_releases_modifiers() {
    let mut state = annotated_page();
    let identity = state.pages.identity();
    let id = begin(&mut state);
    for message in [
        Message::Undo,
        Message::DeleteSelected,
        Message::NewProject,
        Message::OpenProject,
        Message::SaveProject,
        Message::AddPage,
        Message::ImportPdf,
        Message::SelectPage(0),
    ] {
        let _ = update(&mut state, message);
    }
    assert_eq!(state.pages.identity(), identity);
    let _ = update(&mut state, Message::PdfExportProgress(id, 1, 2));
    assert!(state.status.contains("1 of 2"));
    let _ = update(&mut state, Message::PdfExportProgress(id + 1, 0, 2));
    assert!(state.status.contains("1 of 2"));
    state.modifiers.square = true;
    let _ = update(
        &mut state,
        Message::ModifiersChanged(DragModifiers::default()),
    );
    assert!(!state.modifiers.square);
    let _ = update(&mut state, Message::PdfExported(id, Ok(None)));
    assert!(state.project.is_idle());
}

#[test]
fn export_accepts_pending_text_and_cancels_unfinished_shapes() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, Point::new(100.0, 100.0), 1.0),
    );
    paste_text(&mut state, "Pending text");
    let id = begin(&mut state);
    assert!(!state.annotations().has_text_edit());
    assert_eq!(
        state.annotations().texts().next().unwrap().text,
        "Pending text"
    );
    assert!(state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::PdfExported(id, Ok(None)));
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, Point::new(300.0, 300.0), 1.0),
    );
    let _ = begin(&mut state);
    assert!(state.annotations().active_pointer().is_none());
    assert_eq!(state.annotations().len(), 1);
}

#[test]
fn export_cannot_replace_another_modal_operation() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::AskDeletePage);
    let _ = update(&mut state, Message::ExportPdf);
    assert!(state.confirm_delete_page);
    assert!(state.project.is_idle());
    let _ = update(&mut state, Message::KeepPage);
    let _ = update(&mut state, Message::ImportPdf);
    let _ = update(&mut state, Message::ExportPdf);
    assert!(matches!(
        state.project.operation,
        Operation::ImportingPdf { .. }
    ));
}

#[test]
fn late_cancel_reports_a_file_that_already_committed_as_successful() {
    let mut state = annotated_page();
    let id = begin(&mut state);
    let _ = update(&mut state, Message::CancelPdfExport);
    let _ = update(
        &mut state,
        Message::PdfExported(
            id,
            Ok(Some(ExportReceipt {
                name: "committed.pdf".into(),
                directory: None,
            })),
        ),
    );
    assert!(state.project.is_idle());
    assert!(!state.status_is_error);
    assert_eq!(state.status, "Exported committed.pdf.");
}
