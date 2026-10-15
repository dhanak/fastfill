use super::*;
use crate::app::controller::update;
use crate::app::tests::{annotated_page, paste_text};
use crate::app::{PagePoint, Pointer, Tool, page_size};
use fastfill_rust::geometry::Size as PageSize;
use iced::widget::image::Handle;

#[test]
fn pages_keep_independent_annotations_and_history_when_switched() {
    let mut state = annotated_image_page();
    let first = state.annotations().shapes()[0].clone();

    let _ = update(&mut state, Message::AddPage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Add, Ok(Some(test_image("two")))),
    );
    assert_eq!(state.pages.len(), 2);
    assert_eq!(state.pages.active_index(), 1);
    assert_eq!(state.annotations().shapes(), []);
    assert_eq!(state.annotations().tool(), Tool::Rectangle);

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
    let second = state.annotations().shapes()[0].clone();

    let _ = update(&mut state, Message::SelectPage(0));
    assert_eq!(state.annotations().shapes(), std::slice::from_ref(&first));
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().shapes(), []);

    let _ = update(&mut state, Message::SelectPage(1));
    assert_eq!(state.annotations().shapes(), [second]);
    assert!(state.annotations().can_undo());
}

#[test]
fn page_buttons_and_drag_reorder_the_active_page() {
    let mut state = FastFill::default();
    for name in ["one", "two", "three"] {
        let _ = update(&mut state, Message::AddPage);
        let _ = update(
            &mut state,
            Message::ImagePicked(PickPurpose::Add, Ok(Some(test_image(name)))),
        );
    }
    assert_eq!(state.pages.names(), ["one.png", "two.png", "three.png"]);

    let _ = update(&mut state, Message::SelectPage(0));
    let _ = update(&mut state, Message::MovePageRight);
    assert_eq!(state.pages.names(), ["two.png", "one.png", "three.png"]);
    assert_eq!(state.pages.active_index(), 1);

    let _ = update(&mut state, Message::PageDragStarted(1));
    let _ = update(&mut state, Message::PageDragEntered(2));
    let _ = update(&mut state, Message::PageDragFinished(2));
    assert_eq!(state.pages.names(), ["two.png", "three.png", "one.png"]);
    assert_eq!(state.pages.active_index(), 2);
}

#[test]
fn deleting_pages_requires_confirmation_and_keeps_one_blank_page() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::AddPage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Add, Ok(Some(test_image("only")))),
    );

    let _ = update(&mut state, Message::AskDeletePage);
    assert!(state.confirm_delete_page);
    assert_eq!(state.pages.names(), ["only.png"]);
    let _ = update(&mut state, Message::KeepPage);
    assert!(!state.confirm_delete_page);

    let _ = update(&mut state, Message::AskDeletePage);
    let _ = update(&mut state, Message::ConfirmDeletePage);
    assert!(!state.confirm_delete_page);
    assert_eq!(state.pages.len(), 1);
    assert!(state.image().is_none());
    assert!(state.annotations().is_empty());
}

#[test]
fn first_image_requires_confirmation_before_replacing_an_annotated_blank() {
    let mut state = annotated_page();
    let original = state.annotations().shapes()[0].clone();
    let _ = update(&mut state, Message::AddPage);
    assert_eq!(state.image_picker, ImagePicker::ConfirmReplace);
    assert_eq!(state.pages.len(), 1);
    assert_eq!(
        state.annotations().shapes(),
        std::slice::from_ref(&original)
    );

    let _ = update(&mut state, Message::KeepPage);
    assert_eq!(
        state.annotations().shapes(),
        std::slice::from_ref(&original)
    );
    let _ = update(&mut state, Message::AddPage);
    let _ = update(&mut state, Message::ConfirmReplacePage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Replace, Ok(None)),
    );
    assert_eq!(state.annotations().shapes(), [original]);

    let _ = update(&mut state, Message::AddPage);
    let _ = update(&mut state, Message::ConfirmReplacePage);
    let _ = update(
        &mut state,
        Message::ImagePicked(
            PickPurpose::Replace,
            Ok(Some(test_image("first"))),
        ),
    );
    assert_eq!(state.pages.names(), ["first.png"]);
    assert!(state.annotations().is_empty());
}

#[test]
fn successful_import_remembers_the_folder_and_cancel_keeps_it() {
    let mut state = FastFill::default();
    let directory = std::path::PathBuf::from("forms");
    let mut image = test_image("first");
    image.directory = Some(directory.clone());
    let _ = update(&mut state, Message::AddPage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Add, Ok(Some(image))),
    );
    assert_eq!(state.image_directory.as_ref(), Some(&directory));
    let _ = update(&mut state, Message::AddPage);
    let _ =
        update(&mut state, Message::ImagePicked(PickPurpose::Add, Ok(None)));
    assert_eq!(state.image_directory.as_ref(), Some(&directory));
}

fn test_image(name: &str) -> ImportedImage {
    ImportedImage {
        handle: Handle::from_rgba(1, 1, vec![255; 4]),
        display: None,
        name: format!("{name}.png"),
        size: PageSize::new(600.0, 600.0),
        info: fastfill_rust::image_input::ImageInfo {
            width: 600,
            height: 600,
            format: fastfill_rust::image_input::ImageFormatKind::Png,
        },
        directory: None,
    }
}

fn annotated_image_page() -> FastFill {
    let mut state = annotated_page();
    state.pages.current_mut().image = Some(test_image("one"));
    state
}

#[test]
fn page_limit_and_canceled_adds_keep_the_current_document() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::AddPage);
    let _ =
        update(&mut state, Message::ImagePicked(PickPurpose::Add, Ok(None)));
    assert_eq!(state.pages.len(), 1);
    assert!(state.image().is_none());
    for index in 0..MAXIMUM_PAGES {
        let _ = update(&mut state, Message::AddPage);
        let _ = update(
            &mut state,
            Message::ImagePicked(
                PickPurpose::Add,
                Ok(Some(test_image(&index.to_string()))),
            ),
        );
    }
    assert_eq!(state.pages.len(), 20);
    let _ = update(&mut state, Message::AddPage);
    assert_eq!(state.image_picker, ImagePicker::Idle);
    assert!(state.status_is_error);
    assert_eq!(state.pages.len(), 20);
    assert_eq!(state.image().unwrap().name, "19.png");
}

#[test]
fn page_switch_accepts_pending_text_and_cancels_a_shape_draft() {
    let mut state = annotated_image_page();
    let _ = update(&mut state, Message::AddPage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Add, Ok(Some(test_image("two")))),
    );
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
    );
    paste_text(&mut state, "Keep on page two");
    let _ = update(&mut state, Message::SelectPage(0));
    let _ = update(&mut state, Message::SelectPage(1));
    assert_eq!(state.annotations().texts()[0].text, "Keep on page two");
    assert!(!state.annotations().has_text_edit());

    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 300.0), 1.0),
    );
    let _ = update(&mut state, Message::SelectPage(0));
    let _ = update(&mut state, Message::SelectPage(1));
    assert!(state.annotations().active_pointer().is_none());
    assert_eq!(state.annotations().shapes(), []);
}

#[test]
fn invalid_or_modal_page_messages_cannot_change_the_document() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SelectPage(usize::MAX));
    let _ = update(&mut state, Message::MovePageLeft);
    let _ = update(&mut state, Message::MovePageRight);
    assert_eq!(state.pages.active_index(), 0);
    assert_eq!(state.annotations().shapes().len(), 1);

    let _ = update(&mut state, Message::AskDeletePage);
    for message in [Message::Undo, Message::AddPage, Message::SelectPage(1)] {
        let _ = update(&mut state, message);
    }
    assert_eq!(state.pages.len(), 1);
    assert_eq!(state.annotations().shapes().len(), 1);
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.confirm_delete_page);

    // A late picker response must not replace the current page.
    let _ = update(
        &mut state,
        Message::ImagePicked(
            PickPurpose::Replace,
            Ok(Some(test_image("late"))),
        ),
    );
    assert!(state.image().is_none());
    assert_eq!(state.annotations().shapes().len(), 1);
}

#[test]
fn tool_defaults_follow_page_switches_without_mutating_old_objects() {
    let mut state = annotated_image_page();
    let original = state.annotations().shapes()[0].clone();
    let _ = update(&mut state, Message::SetTool(Tool::Line));
    let _ = update(
        &mut state,
        Message::SetColor(crate::app::AnnotationColor::Red),
    );
    let _ = update(&mut state, Message::AdjustThickness(3.0));
    let _ = update(&mut state, Message::AddPage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Add, Ok(Some(test_image("two")))),
    );
    let style = state.annotations().current_style().unwrap();
    assert_eq!(style.color, crate::app::AnnotationColor::Red);
    assert_eq!(style.stroke_width, 6.0);

    let _ = update(
        &mut state,
        Message::SetColor(crate::app::AnnotationColor::Blue),
    );
    let _ = update(&mut state, Message::SelectPage(0));
    let style = state.annotations().current_style().unwrap();
    assert_eq!(style.color, crate::app::AnnotationColor::Blue);
    assert_eq!(state.annotations().shapes(), [original]);
}
