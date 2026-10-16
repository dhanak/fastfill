use super::*;
use crate::app::controller::update;
use crate::app::tests::{annotated_page, paste_text};
use crate::app::{PagePoint, Pointer, Tool, page_size};
use fastfill_rust::project::load_archive;
use iced::widget::image::Handle;
use std::io::Cursor;

#[test]
fn dirty_state_ignores_defaults_selection_and_canceled_drafts() {
    let mut state = FastFill::default();
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::SetColor(super::super::AnnotationColor::Blue),
    );
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
    );
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.project.is_dirty(&state.pages));
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    assert!(state.project.is_dirty(&state.pages));
}

#[test]
fn display_pixels_do_not_replace_the_original_encoded_save_data() {
    let image = test_image("source", 60, 90);
    assert!(matches!(image.handle, Handle::Bytes(_, _)));
    assert!(matches!(image.display_handle(), Handle::Rgba { .. }));
    let mut state = FastFill::default();
    let expected = image.handle.clone();
    assert!(state.pages.add_image(image));
    let snapshot = state.project_snapshot().unwrap();
    assert_eq!(snapshot.images[0], Some(expected));
}

#[test]
fn new_project_accepts_pending_text_and_cancel_keeps_it() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
    );
    paste_text(&mut state, "Keep this");
    let _ = update(&mut state, Message::NewProject);
    assert!(matches!(
        state.project.operation,
        Operation::Confirm(LeaveAction::New)
    ));
    assert_eq!(state.annotations().texts()[0].text, "Keep this");
    let _ = update(&mut state, Message::KeepProject);
    assert!(state.project.is_idle());
    assert_eq!(state.annotations().texts()[0].text, "Keep this");
    let _ = update(&mut state, Message::NewProject);
    let _ = update(&mut state, Message::DiscardProject);
    assert!(!state.project.is_dirty(&state.pages));
    assert!(state.annotations().is_empty());
}

#[test]
fn failed_or_canceled_open_keeps_every_page_and_history() {
    let mut state = annotated_page();
    let original = state.annotations().shapes()[0].clone();
    let _ = update(&mut state, Message::OpenProject);
    let _ = update(&mut state, Message::DiscardProject);
    let Operation::Opening(id) = state.project.operation else {
        panic!("open must be pending");
    };
    let _ = update(
        &mut state,
        Message::ProjectOpened(id, Err("Broken ZIP".into())),
    );
    assert!(state.status_is_error);
    assert_eq!(state.annotations().shapes()[0], original);
    assert!(state.annotations().can_undo());
    assert!(state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::OpenProject);
    let _ = update(&mut state, Message::DiscardProject);
    let Operation::Opening(id) = state.project.operation else {
        panic!("open must be pending");
    };
    let _ = update(&mut state, Message::ProjectOpened(id, Ok(None)));
    assert_eq!(state.annotations().shapes()[0], original);
    assert!(state.project.is_dirty(&state.pages));
}

#[test]
fn saves_accept_text_and_snapshot_blank_pages_into_valid_archives() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 300.0), 1.0),
    );
    paste_text(&mut state, "Saved text");
    let _ = update(&mut state, Message::SaveProject);
    let snapshot = state.project_snapshot().unwrap();
    assert!(!state.annotations().has_text_edit());
    let archive = super::super::project_io::encode_snapshot(snapshot).unwrap();
    let mut zip = Cursor::new(Vec::new());
    fastfill_rust::project::save_archive(&archive, &mut zip).unwrap();
    zip.set_position(0);
    let loaded = load_archive(zip).unwrap();
    assert_eq!(loaded.project.pages[0].annotations.len(), 2);
    assert_eq!(loaded.project.pages[0].source_pixel_width, 794);
    assert_eq!(loaded.assets.len(), 1);
}

#[test]
fn successful_save_cleans_state_and_undo_redo_restore_dirty_identity() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SaveProject);
    let id = state.project.operation.request_id().unwrap();
    let receipt = SaveReceipt {
        path: Some(PathBuf::from("forms/saved.docscan")),
        name: "saved.docscan".into(),
    };
    let _ = update(&mut state, Message::ProjectSaved(id, Ok(Some(receipt))));
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::DeleteSelected);
    assert!(state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::Undo);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::Redo);
    assert!(state.project.is_dirty(&state.pages));
}

#[test]
fn canceled_or_failed_save_cannot_continue_a_destructive_action() {
    let mut state = annotated_page();
    let original = state.annotations().shapes()[0].clone();
    for result in [Ok(None), Err("Disk full".into())] {
        let _ = update(&mut state, Message::NewProject);
        let _ = update(&mut state, Message::SaveBeforeLeave);
        let id = state.project.operation.request_id().unwrap();
        let _ = update(&mut state, Message::ProjectSaved(id, result));
        assert!(state.project.is_idle());
        assert!(state.project.is_dirty(&state.pages));
        assert_eq!(state.annotations().shapes()[0], original);
    }
}

#[test]
fn busy_operations_ignore_editing_and_stale_completion_messages() {
    let mut state = annotated_page();
    let original = state.annotations().shapes()[0].clone();
    let _ = update(&mut state, Message::SaveProject);
    let id = state.project.operation.request_id().unwrap();
    for message in [
        Message::Undo,
        Message::DeleteSelected,
        Message::NewProject,
        Message::AddPage,
        Message::SetTool(Tool::Text),
        Message::FocusLost,
    ] {
        let _ = update(&mut state, message);
    }
    assert_eq!(state.annotations().shapes()[0], original);
    let _ = update(&mut state, Message::ProjectSaved(id + 1, Ok(None)));
    assert_eq!(state.project.operation.request_id(), Some(id));
    let _ = update(&mut state, Message::ProjectSaved(id, Ok(None)));
    assert!(state.project.is_idle());
}

#[test]
fn page_order_and_image_replacement_participate_in_dirty_state() {
    let mut state = FastFill::default();
    add_image(&mut state, "one", 600, 900);
    add_image(&mut state, "two", 600, 900);
    let _ = update(&mut state, Message::SaveProject);
    finish_save(&mut state);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::SelectPage(0));
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::MovePageRight);
    assert!(state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::MovePageLeft);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::AskReplacePage);
    let _ = update(
        &mut state,
        Message::ImagePicked(
            super::super::PickPurpose::Replace,
            Ok(Some(test_image("replacement", 600, 900))),
        ),
    );
    assert!(state.project.is_dirty(&state.pages));
}

#[test]
fn a_successful_save_continues_new_but_focus_loss_does_not_discard() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::NewProject);
    let _ = update(&mut state, Message::FocusLost);
    assert!(matches!(state.project.operation, Operation::Confirm(_)));
    let _ = update(&mut state, Message::SaveBeforeLeave);
    finish_save(&mut state);
    assert!(state.annotations().is_empty());
    assert!(state.project.is_idle());
    assert!(state.project.path.is_none());
    assert!(!state.project.is_dirty(&state.pages));
}

#[test]
fn window_close_is_guarded_and_escape_keeps_the_current_project() {
    let mut state = annotated_page();
    let window = iced::window::Id::unique();
    let _ = update(&mut state, Message::CloseRequested(window));
    assert!(matches!(state.project.operation,
        Operation::Confirm(LeaveAction::Close(id)) if id == window));
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(state.project.is_idle());
    assert!(state.project.is_dirty(&state.pages));
    assert_eq!(state.annotations().shapes().len(), 1);
}

#[test]
fn command_key_release_during_save_does_not_leave_constraints_stuck() {
    let mut state = annotated_page();
    state.modifiers.square = true;
    let _ = update(&mut state, Message::SaveProject);
    let _ = update(
        &mut state,
        Message::ModifiersChanged(super::super::DragModifiers::default()),
    );
    finish_save(&mut state);
    assert!(!state.modifiers.square);
}

#[test]
fn project_shortcuts_use_the_platform_command_modifier() {
    let shortcut = super::super::controller::shortcut_message;
    let command = iced::keyboard::Modifiers::COMMAND;
    let key = |value: &str| iced::keyboard::Key::Character(value.into());
    assert!(matches!(
        shortcut(key("n"), command),
        Some(Message::NewProject)
    ));
    assert!(matches!(
        shortcut(key("O"), command),
        Some(Message::OpenProject)
    ));
    assert!(matches!(
        shortcut(key("s"), command),
        Some(Message::SaveProject)
    ));
    assert!(matches!(
        shortcut(key("s"), command | iced::keyboard::Modifiers::SHIFT),
        Some(Message::SaveProjectAs)
    ));
    assert!(shortcut(key("s"), iced::keyboard::Modifiers::empty()).is_none());
}

#[test]
fn imported_pages_use_points_but_one_to_one_still_uses_image_pixels() {
    let mut state = FastFill::default();
    add_image(&mut state, "portrait", 600, 900);
    assert_eq!(page_size(&state), super::super::PageSize::new(612.0, 918.0));
    assert_eq!(
        state.image().unwrap().size,
        super::super::PageSize::new(600.0, 900.0)
    );
    let viewport = super::super::PageRect::new(0.0, 0.0, 800.0, 600.0);
    let _ = update(&mut state, Message::OneToOne(viewport));
    let view = state.navigation.view(page_size(&state), viewport).unwrap();
    assert!((view.page_rect().width - 600.0).abs() < 0.001);
    assert!((view.page_rect().height - 900.0).abs() < 0.001);
}

#[test]
fn save_reload_keeps_pages_original_image_bytes_ids_and_empty_history() {
    let mut state = FastFill::default();
    add_image(&mut state, "portrait", 600, 900);
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
    let id = state.annotations().shapes()[0].id;
    add_image(&mut state, "landscape", 900, 600);
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
    );
    paste_text(&mut state, "Round trip");
    let _ = update(&mut state, Message::SaveProject);
    let snapshot = state.project_snapshot().unwrap();
    let archive = super::super::project_io::encode_snapshot(snapshot).unwrap();
    let assets = archive.assets.clone();
    let mut zip = Cursor::new(Vec::new());
    fastfill_rust::project::save_archive(&archive, &mut zip).unwrap();
    zip.set_position(0);
    let loaded = load_archive(zip).unwrap();
    for (name, asset) in &assets {
        assert_eq!(loaded.assets[name].bytes, asset.bytes);
    }
    let opened = super::super::project_io::prepare_opened(
        loaded,
        Some(PathBuf::from("round-trip.docscan")),
        "round-trip.docscan".into(),
    )
    .unwrap();
    finish_save(&mut state);
    let _ = update(&mut state, Message::OpenProject);
    let id_request = state.project.operation.request_id().unwrap();
    let _ = update(
        &mut state,
        Message::ProjectOpened(id_request, Ok(Some(Arc::new(opened)))),
    );
    assert!(!state.status_is_error, "{}", state.status);
    assert_eq!(state.pages.len(), 2);
    assert_eq!(state.annotations().shapes()[0].id, id);
    assert!(!state.annotations().can_undo());
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::SelectPage(1));
    assert_eq!(state.annotations().texts()[0].text, "Round trip");
    assert!(!state.annotations().can_undo());
}

#[test]
fn unsupported_page_load_fails_before_replacing_any_page() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SaveProject);
    let snapshot = state.project_snapshot().unwrap();
    let mut archive =
        super::super::project_io::encode_snapshot(snapshot).unwrap();
    let fastfill_rust::project::ProjectAnnotation::Shape { transform, .. } =
        &mut archive.project.pages[0].annotations[0]
    else {
        panic!("fixture must contain a rectangle")
    };
    transform.m21 = 0.2;
    assert!(
        super::super::project_io::prepare_opened(
            archive,
            None,
            "unsupported.docscan".into()
        )
        .unwrap_err()
        .contains("Page 1")
    );
    assert_eq!(state.annotations().shapes().len(), 1);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_save_to_existing_path_uses_atomic_archive_writer() {
    let state = annotated_page();
    let path = std::env::temp_dir()
        .join(format!("fastfill-{}.docscan", Uuid::new_v4()));
    let snapshot = state.project_snapshot().unwrap();
    let receipt = iced::futures::executor::block_on(
        super::super::project_io::save_project(
            snapshot,
            Some(path.clone()),
            None,
            "test.docscan".into(),
        ),
    )
    .unwrap()
    .unwrap();
    let loaded = fastfill_rust::project::load_archive_file(&path).unwrap();
    assert_eq!(loaded.project.pages[0].annotations.len(), 1);
    assert_eq!(receipt.path.as_ref(), Some(&path));
    std::fs::remove_file(path).unwrap();
}

fn finish_save(state: &mut FastFill) {
    let id = state.project.operation.request_id().unwrap();
    let _ = update(
        state,
        Message::ProjectSaved(
            id,
            Ok(Some(SaveReceipt {
                path: Some(PathBuf::from("saved.docscan")),
                name: "saved.docscan".into(),
            })),
        ),
    );
}

fn add_image(state: &mut FastFill, name: &str, width: u32, height: u32) {
    let _ = update(state, Message::AddPage);
    let _ = update(
        state,
        Message::ImagePicked(
            super::super::PickPurpose::Add,
            Ok(Some(test_image(name, width, height))),
        ),
    );
}

fn test_image(
    name: &str,
    width: u32,
    height: u32,
) -> crate::picker::ImportedImage {
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        width,
        height,
        image::Rgb([255, 255, 255]),
    ))
    .write_to(&mut png, image::ImageFormat::Png)
    .unwrap();
    crate::picker::ImportedImage::from_bytes(
        format!("{name}.png"),
        png.into_inner(),
        None,
    )
    .unwrap()
}
