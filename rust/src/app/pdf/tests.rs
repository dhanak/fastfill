use super::*;
use crate::app::controller::update;
use crate::app::pages::MAXIMUM_PAGES;
use crate::app::project::Operation;
use crate::app::tests::annotated_page;
use crate::app::{FastFill, Message, PagePoint, PickPurpose, Pointer, Tool};
use crate::picker::ImportedImage;
use fastfill_rust::geometry::Size;
use fastfill_rust::image_input::{ImageFormatKind, ImageInfo};
use fastfill_rust::pdf_import::import_pdf;
use iced::widget::image::Handle;

#[test]
fn replaces_only_an_empty_starter_and_preserves_import_order() {
    let mut state = FastFill::default();
    assert_eq!(state.pages.pdf_capacity(), 20);
    let id = begin(&mut state);
    complete(&mut state, id, &["one", "two"]);
    assert_eq!(state.pages.names(), ["one.png", "two.png"]);
    assert_eq!(state.pages.active_index(), 0);
    assert!(state.project.is_dirty(&state.pages));
    let id = begin(&mut state);
    complete(&mut state, id, &["three", "four"]);
    assert_eq!(
        state.pages.names(),
        ["one.png", "three.png", "four.png", "two.png"]
    );
    assert_eq!(state.pages.active_index(), 1);
}

#[test]
fn keeps_an_annotated_starter_and_its_undo_history() {
    let mut state = annotated_page();
    let original = state.annotations().shapes()[0].clone();
    assert_eq!(state.pages.pdf_capacity(), 19);
    let id = begin(&mut state);
    complete(&mut state, id, &["form"]);
    assert_eq!(state.pages.len(), 2);
    assert_eq!(state.pages.active_index(), 1);
    let _ = update(&mut state, Message::SelectPage(0));
    assert_eq!(state.annotations().shapes(), [original]);
    assert!(state.annotations().can_undo());
}

#[test]
fn failure_and_cancel_leave_the_page_order_selection_and_history_unchanged() {
    let mut state = annotated_page();
    let identity = state.pages.identity();
    let id = begin(&mut state);
    let _ = update(&mut state, Message::PdfProgress(id, 1, 3));
    assert!(state.status.contains("1 of 3"));
    let _ =
        update(&mut state, Message::PdfImported(id, Err("Bad page".into())));
    assert!(state.status_is_error);
    assert_eq!(state.pages.identity(), identity);
    let id = begin(&mut state);
    let _ = update(&mut state, Message::CancelPdfImport);
    complete(&mut state, id, &["late"]);
    assert!(state.project.is_idle());
    assert_eq!(state.pages.identity(), identity);
    assert!(state.annotations().has_selection());
    assert!(state.annotations().can_undo());
    let current = begin(&mut state);
    complete(&mut state, id, &["old worker"]);
    assert!(active(&state, current));
    assert_eq!(state.pages.identity(), identity);
    complete(&mut state, current, &["new worker"]);
    assert_eq!(state.pages.names(), ["new worker.png"]);
}

#[test]
fn capacity_failure_is_atomic_and_ignores_empty_or_stale_results() {
    let mut state = FastFill::default();
    let id = begin(&mut state);
    let names = vec!["page"; MAXIMUM_PAGES];
    complete(&mut state, id, &names);
    assert_eq!(state.pages.len(), 20);
    assert_eq!(state.pages.pdf_capacity(), 0);
    let identity = state.pages.identity();
    let _ = update(&mut state, Message::ImportPdf);
    assert!(state.project.is_idle());
    assert!(state.status_is_error);
    complete(&mut state, id, &["stale"]);
    assert_eq!(state.pages.identity(), identity);

    let mut state = annotated_page();
    let identity = state.pages.identity();
    let id = begin(&mut state);
    complete(&mut state, id, &names);
    assert!(state.status_is_error);
    assert_eq!(state.pages.identity(), identity);
    let id = begin(&mut state);
    complete(&mut state, id, &[]);
    assert!(state.status_is_error);
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn importing_blocks_other_edits_and_releases_modifiers_correctly() {
    let mut state = annotated_page();
    let identity = state.pages.identity();
    let id = begin(&mut state);
    for message in [
        Message::Undo,
        Message::DeleteSelected,
        Message::NewProject,
        Message::AddPage,
        Message::SelectPage(0),
    ] {
        let _ = update(&mut state, message);
    }
    assert_eq!(state.pages.identity(), identity);
    assert!(matches!(
        state.project.operation,
        Operation::ImportingPdf { .. }
    ));
    let _ = update(&mut state, Message::PdfProgress(id + 1, 10, 20));
    assert!(!state.status.contains("10 of 20"));
    state.modifiers.square = true;
    let _ = update(
        &mut state,
        Message::ModifiersChanged(crate::app::DragModifiers::default()),
    );
    assert!(!state.modifiers.square);
    let _ = update(&mut state, Message::PdfImported(id, Ok(None)));
    assert!(state.project.is_idle());
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn pending_text_is_accepted_and_shapes_are_canceled_before_import() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
    );
    crate::app::tests::paste_text(&mut state, "Keep me");
    let id = begin(&mut state);
    assert!(!state.annotations().has_text_edit());
    complete(&mut state, id, &["form"]);
    assert_eq!(state.pages.len(), 2);
    let _ = update(&mut state, Message::SelectPage(0));
    assert_eq!(state.annotations().texts()[0].text, "Keep me");
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 300.0), 1.0),
    );
    let id = begin(&mut state);
    assert!(state.annotations().active_pointer().is_none());
    let _ = update(&mut state, Message::PdfImported(id, Ok(None)));
}

#[test]
fn import_does_not_override_another_modal_operation() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::AskDeletePage);
    let _ = update(&mut state, Message::ImportPdf);
    assert!(state.confirm_delete_page);
    assert!(state.project.is_idle());
    let _ = update(&mut state, Message::KeepPage);
    let _ = update(&mut state, Message::AddPage);
    assert!(matches!(
        state.image_picker,
        crate::app::ImagePicker::ConfirmReplace
    ));
    let _ = update(&mut state, Message::ImportPdf);
    assert!(state.project.is_idle());
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Replace, Ok(None)),
    );
}

#[test]
fn stage_progress_updates_within_one_page_and_ignores_stale_requests() {
    let mut state = FastFill::default();
    let id = begin(&mut state);
    let _ = update(
        &mut state,
        Message::PdfStage(id, 0, 1, PdfStage::Rasterizing),
    );
    assert!(state.status.contains("page 1 of 1: Rendering"));
    let Operation::ImportingPdf { progress, .. } = state.project.operation
    else {
        panic!("import must be pending")
    };
    assert!((progress - 1.0 / 3.0).abs() < f32::EPSILON);
    let _ = update(
        &mut state,
        Message::PdfStage(id + 1, 0, 1, PdfStage::Encoding),
    );
    assert!(state.status.contains("Rendering"));
}

#[test]
fn rasterized_pdf_pages_and_annotations_survive_a_project_round_trip() {
    use crate::app::pages::Pages;
    use crate::app::project_io::{encode_snapshot, prepare_opened};
    use fastfill_rust::project::{load_archive, save_archive};
    use std::io::Cursor;

    let source = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/samples/fastfill-camera-sample-form.pdf"
    ))
    .unwrap();
    let mut state = annotated_page();
    let id = begin(&mut state);
    let images = import_pdf(source, 19, |_, _| true)
        .unwrap()
        .into_iter()
        .map(|page| {
            ImportedImage::from_validated(
                "form.png".into(),
                page.bytes,
                page.info,
                None,
            )
        })
        .collect::<Vec<_>>();
    let original_info = images[0].info;
    let batch = PdfBatch {
        images,
        directory: Some(PathBuf::from("forms")),
    };
    let _ = update(
        &mut state,
        Message::PdfImported(id, Ok(Some(Arc::new(batch)))),
    );
    assert_eq!(state.image_directory, Some(PathBuf::from("forms")));
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
    );
    let _ = update(
        &mut state,
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(200.0, 200.0),
            false,
        ),
    );
    let identity = state.pages.identity();
    let archive = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    let mut zip = Cursor::new(Vec::new());
    save_archive(&archive, &mut zip).unwrap();
    zip.set_position(0);
    let loaded = load_archive(zip).unwrap();
    let opened = prepare_opened(loaded, None, "form.docscan".into()).unwrap();
    state.pages = Pages::from_opened(&opened).unwrap();
    assert_eq!(state.pages.len(), 2);
    for (page, (id, _)) in state.pages.iter().zip(identity) {
        assert_eq!(page.metadata.id, id);
        assert_eq!(page.annotations.len(), 1);
        assert!(!page.annotations.can_undo());
    }
    assert!(state.pages.select(1));
    assert_eq!(
        state.pages.current().image.as_ref().unwrap().info,
        original_info
    );
    assert_eq!(state.pages.current().size.width, 612.0);
}

fn begin(state: &mut FastFill) -> u64 {
    let _ = update(state, Message::ImportPdf);
    let Operation::ImportingPdf { id, .. } = state.project.operation else {
        panic!("PDF import must be pending");
    };
    id
}

fn complete(state: &mut FastFill, id: u64, names: &[&str]) {
    let batch = PdfBatch {
        images: names.iter().map(|name| image(name)).collect(),
        directory: None,
    };
    let _ = update(state, Message::PdfImported(id, Ok(Some(Arc::new(batch)))));
}

fn image(name: &str) -> ImportedImage {
    ImportedImage {
        handle: Handle::from_rgba(1, 1, vec![255; 4]),
        display: None,
        name: format!("{name}.png"),
        size: Size::new(600.0, 900.0),
        info: ImageInfo {
            width: 600,
            height: 900,
            format: ImageFormatKind::Png,
        },
        directory: None,
    }
}
