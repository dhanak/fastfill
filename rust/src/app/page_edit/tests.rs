use super::*;
use crate::app::controller::update;
use crate::app::project::Operation;
use crate::app::project_io::{SaveReceipt, decode_project, encode_snapshot};
use crate::app::{PagePoint, Pointer, Tool};
use crate::picker::ImportedImage;
use fastfill_rust::geometry::Size;
use fastfill_rust::project::{load_archive, save_archive};
use iced::widget::image::Handle;
use std::io::Cursor;

mod auto_frame;
mod corner_snap;

fn state() -> FastFill {
    let pixels = image::RgbaImage::from_fn(120, 180, |x, y| {
        image::Rgba([
            u8::try_from(x).unwrap(),
            u8::try_from(y).unwrap(),
            90,
            255,
        ])
    });
    let mut bytes = Cursor::new(Vec::new());
    pixels
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let image = ImportedImage::from_bytes(
        "source.png".into(),
        bytes.into_inner(),
        None,
    )
    .unwrap();
    let mut state = FastFill::default();
    state.pages.add_image(image);
    let page = state.pages.current().size;
    state.annotations_mut().set_tool(Tool::Rectangle);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 500.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 600.0),
        false,
    );
    mark_saved(&mut state);
    state
}

fn mark_saved(state: &mut FastFill) {
    let _ = update(state, Message::SaveProject);
    let Operation::Saving { id, .. } = state.project.operation else {
        panic!("save must be pending")
    };
    let _ = update(
        state,
        Message::ProjectSaved(
            id,
            Ok(Some(SaveReceipt {
                path: None,
                name: "fixture.docscan".into(),
            })),
        ),
    );
    assert!(!state.project.is_dirty(&state.pages));
}

fn send(state: &mut FastFill, event: Event) {
    let _ = update(state, Message::PageEdit(event));
}

fn finish_one(state: &mut FastFill) {
    let job = state.page_editor.running.as_ref().expect("pending job");
    let (id, settings, auto_frame) = (job.id, job.settings, job.auto_frame);
    let input = state
        .page_editor
        .draft
        .as_ref()
        .and_then(|draft| draft.original.as_ref())
        .map_or_else(
            || Input::Image(state.image().unwrap().handle.clone()),
            |original| {
                if auto_frame {
                    Input::Frame(original.clone())
                } else {
                    Input::Decoded(original.clone())
                }
            },
        );
    send(state, Event::Ready(id, preview::build(input, settings)));
}

fn settle(state: &mut FastFill) {
    for _ in 0..10 {
        if state.page_editor.running.is_none() {
            return;
        }
        finish_one(state);
    }
    panic!("preview scheduling must converge")
}

#[test]
fn cancel_and_escape_preserve_page_history_pixels_and_clean_state() {
    for escape in [false, true] {
        let mut state = state();
        let metadata = state.pages.current().metadata.clone();
        let shapes = state.annotations().shapes().to_vec();
        let display = state.image().unwrap().display_handle().clone();
        let identity = state.pages.identity();
        send(&mut state, Event::Begin);
        send(&mut state, Event::Brightness(20));
        send(&mut state, Event::Rotate(1));
        settle(&mut state);
        assert_eq!(state.pages.identity(), identity);
        assert!(!state.project.is_dirty(&state.pages));
        if escape {
            let _ = update(&mut state, Message::CancelInteraction);
        } else {
            send(&mut state, Event::Cancel);
        }
        assert!(!state.page_editor.is_open());
        assert_eq!(state.pages.current().metadata, metadata);
        assert_eq!(state.annotations().shapes(), shapes);
        assert_eq!(state.image().unwrap().display_handle(), &display);
        assert_eq!(state.pages.identity(), identity);
    }
}

#[test]
fn accepts_current_preview_and_undo_redo_restore_page_and_annotations_together()
{
    let mut state = state();
    let metadata = state.pages.current().metadata.clone();
    let shape = state.annotations().shapes()[0].clone();
    let display = state.image().unwrap().display_handle().clone();
    send(&mut state, Event::Begin);
    send(&mut state, Event::Rotate(1));
    send(&mut state, Event::Brightness(15));
    send(&mut state, Event::Accept);
    assert!(
        state.page_editor.is_open(),
        "stale preview cannot be accepted"
    );
    settle(&mut state);
    send(&mut state, Event::Accept);
    assert!(!state.page_editor.is_open());
    assert!(state.project.is_dirty(&state.pages));
    assert_eq!(state.pages.current().metadata.rotation_quarter_turns, 1);
    assert_eq!(state.pages.current().metadata.brightness, 15);
    let changed = state.annotations().shapes()[0].clone();
    assert_eq!(changed.id, shape.id);
    assert_eq!(changed.rotation, shape.rotation);
    assert_eq!(changed.stroke_width, shape.stroke_width);
    assert!(
        (changed.bounds().width - shape.bounds().width * 2.0 / 3.0).abs()
            < 0.01
    );
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.pages.current().metadata, metadata);
    assert_eq!(state.annotations().shapes()[0], shape);
    assert_eq!(state.image().unwrap().display_handle(), &display);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::Redo);
    assert_eq!(state.annotations().shapes()[0], changed);
    assert_eq!(state.pages.current().metadata.rotation_quarter_turns, 1);
    assert!(state.project.is_dirty(&state.pages));
}

#[test]
fn rapid_settings_changes_coalesce_and_only_the_latest_preview_can_be_accepted()
{
    let mut state = state();
    send(&mut state, Event::Begin);
    let first = state.page_editor.running.as_ref().unwrap().id;
    for value in 1..=40 {
        send(&mut state, Event::Contrast(value));
    }
    assert_eq!(state.page_editor.running.as_ref().unwrap().id, first);
    finish_one(&mut state);
    let latest = state.page_editor.running.as_ref().unwrap();
    assert_ne!(latest.id, first);
    assert_eq!(latest.settings.filter.contrast, 40);
    send(&mut state, Event::Ready(first, Err("stale failure".into())));
    assert!(state.page_editor.draft.as_ref().unwrap().error.is_none());
    settle(&mut state);
    assert_eq!(
        state
            .page_editor
            .draft
            .as_ref()
            .unwrap()
            .ready
            .as_ref()
            .unwrap()
            .settings
            .filter
            .contrast,
        40
    );
    send(&mut state, Event::Accept);
    assert_eq!(state.pages.current().metadata.filter.contrast, 40);
}

#[test]
fn canceled_job_cannot_update_a_new_editor_session_or_commit_its_pixels() {
    let mut state = state();
    send(&mut state, Event::Begin);
    let job = state.page_editor.running.as_ref().unwrap();
    let (old_id, settings) = (job.id, job.settings);
    let old_result = preview::build(
        Input::Image(state.image().unwrap().handle.clone()),
        settings,
    )
    .unwrap();
    send(&mut state, Event::Cancel);
    send(&mut state, Event::Begin);
    send(&mut state, Event::Brightness(25));
    assert_eq!(state.page_editor.running.as_ref().unwrap().id, old_id);
    send(&mut state, Event::Ready(old_id, Ok(old_result)));
    assert!(state.page_editor.draft.as_ref().unwrap().ready.is_none());
    assert!(state.page_editor.draft.as_ref().unwrap().original.is_none());
    settle(&mut state);
    send(&mut state, Event::Accept);
    assert_eq!(state.pages.current().metadata.brightness, 25);
}

#[test]
fn unchanged_accept_and_failed_previews_leave_committed_data_unchanged() {
    let mut state = state();
    let identity = state.pages.identity();
    send(&mut state, Event::Begin);
    settle(&mut state);
    send(&mut state, Event::Accept);
    assert_eq!(state.pages.identity(), identity);
    send(&mut state, Event::Begin);
    send(&mut state, Event::Brightness(101));
    assert!(state.page_editor.draft.as_ref().unwrap().error.is_some());
    assert_eq!(
        state
            .page_editor
            .draft
            .as_ref()
            .unwrap()
            .settings
            .brightness,
        0
    );
    let id = state.page_editor.running.as_ref().unwrap().id;
    send(&mut state, Event::Ready(id, Err("Decode failed".into())));
    send(&mut state, Event::Accept);
    assert!(state.page_editor.is_open());
    assert_eq!(state.pages.identity(), identity);
    send(&mut state, Event::Cancel);
    assert!(!state.project.is_dirty(&state.pages));
}

#[test]
fn modal_editor_blocks_commands_but_resize_and_focus_loss_keep_the_draft() {
    let mut state = state();
    let identity = state.pages.identity();
    send(&mut state, Event::Begin);
    send(&mut state, Event::Brightness(10));
    for message in [
        Message::NewProject,
        Message::SaveProject,
        Message::ExportPdf,
        Message::Undo,
        Message::DeleteSelected,
        Message::SelectPage(0),
        Message::AddPage,
        Message::ViewportResized,
        Message::FocusLost,
    ] {
        let _ = update(&mut state, message);
        assert!(state.page_editor.is_open());
        assert_eq!(state.pages.identity(), identity);
        assert!(state.project.is_idle());
    }
    settle(&mut state);
    send(&mut state, Event::Accept);
    assert_eq!(state.pages.current().metadata.brightness, 10);
}

#[test]
fn accepted_changes_survive_save_reload_without_modifying_source_bytes() {
    let mut state = state();
    let original = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    send(&mut state, Event::Begin);
    send(&mut state, Event::Rotate(1));
    send(&mut state, Event::Brightness(20));
    settle(&mut state);
    send(&mut state, Event::Accept);
    let saved = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
    assert_eq!(saved.project.schema_version, 2);
    let asset = &state.pages.current().metadata.source_asset;
    assert_eq!(saved.assets[asset].bytes, original.assets[asset].bytes);
    let bytes = save_archive(&saved, Cursor::new(Vec::new()))
        .unwrap()
        .into_inner();
    let opened = decode_project(
        Cursor::new(bytes.clone()),
        None,
        "changed.docscan".into(),
    )
    .unwrap();
    let archive = load_archive(Cursor::new(bytes)).unwrap();
    assert_eq!(archive.project.pages, saved.project.pages);
    let pages = super::super::pages::Pages::from_opened(&opened).unwrap();
    let a = pages.current().annotations.shapes()[0].bounds();
    let b = state.annotations().shapes()[0].bounds();
    assert!((a.x - b.x).abs() < 0.001 && (a.y - b.y).abs() < 0.001);
    assert!(
        (a.width - b.width).abs() < 0.001
            && (a.height - b.height).abs() < 0.001
    );
}

#[test]
fn invalid_corners_are_rejected_and_full_frame_is_explicit() {
    let mut state = state();
    send(&mut state, Event::Begin);
    send(
        &mut state,
        Event::Corner(0, NormalizedPoint { x: 1.0, y: 1.0 }),
    );
    assert_eq!(
        state.page_editor.draft.as_ref().unwrap().settings.crop,
        CropQuad::default()
    );
    assert!(state.page_editor.draft.as_ref().unwrap().error.is_some());
    send(
        &mut state,
        Event::Corner(0, NormalizedPoint { x: 0.1, y: 0.1 }),
    );
    assert!(state.page_editor.draft.as_ref().unwrap().error.is_none());
    send(&mut state, Event::FullFrame);
    assert_eq!(
        state.page_editor.draft.as_ref().unwrap().settings.crop,
        CropQuad::default()
    );
}

#[test]
fn new_annotation_edits_invalidate_correction_redo_and_release_cached_versions()
{
    let mut state = state();
    send(&mut state, Event::Begin);
    send(&mut state, Event::Brightness(10));
    settle(&mut state);
    send(&mut state, Event::Accept);
    let handle = state.image().unwrap().display_handle().clone();
    let _ = update(&mut state, Message::Undo);
    let _ = update(
        &mut state,
        Message::SetColor(super::super::AnnotationColor::Red),
    );
    assert!(!state.annotations().can_redo());
    assert!(!state.annotations().uses_page_context(1));
    let before = state.image().unwrap().display_handle().clone();
    let _ = update(&mut state, Message::Redo);
    assert_eq!(state.image().unwrap().display_handle(), &before);
    assert_ne!(state.image().unwrap().display_handle(), &handle);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn recovery_skips_previews_and_snapshots_accepted_corrections() {
    use crate::app::recovery::{Command, Event as RecoveryEvent};
    let mut state = state();
    let (sender, receiver) = std::sync::mpsc::channel();
    let _ = update(
        &mut state,
        Message::Recovery(RecoveryEvent::Ready(sender, None)),
    );
    send(&mut state, Event::Begin);
    send(&mut state, Event::Brightness(20));
    let _ = update(&mut state, Message::Recovery(RecoveryEvent::Tick));
    assert!(receiver.try_recv().is_err());
    settle(&mut state);
    send(&mut state, Event::Accept);
    let _ = update(&mut state, Message::Recovery(RecoveryEvent::Tick));
    let Command::Write(_, snapshot) = receiver.try_recv().unwrap() else {
        panic!("expected recovery snapshot")
    };
    assert_eq!(snapshot.project.pages[0].brightness, 20);
    assert_eq!(snapshot.project.schema_version, 2);
}

#[test]
fn blank_page_cannot_open_an_image_editor() {
    let mut state = FastFill::default();
    send(&mut state, Event::Begin);
    assert!(!state.page_editor.is_open());
    assert!(state.page_editor.running.is_none());
}

#[test]
fn displaced_annotations_warn_and_restore_exactly_on_undo() {
    let mut state = state();
    let before = state.annotations().shapes()[0].clone();
    send(&mut state, Event::Begin);
    send(
        &mut state,
        Event::Corner(2, NormalizedPoint { x: 1.0, y: 0.3 }),
    );
    send(
        &mut state,
        Event::Corner(3, NormalizedPoint { x: 0.0, y: 0.3 }),
    );
    settle(&mut state);
    send(&mut state, Event::Accept);
    assert!(!state.page_editor.is_open());
    assert!(
        state
            .page_editor
            .warning
            .as_ref()
            .unwrap()
            .contains("1 annotation")
    );
    let shape = &state.annotations().shapes()[0];
    assert!(shape.bounds().y < state.pages.current().size.height);
    assert_eq!(shape.id, before.id);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().shapes()[0], before);
}

#[test]
fn window_close_requires_an_explicit_choice_but_does_not_disable_accept() {
    let mut state = state();
    send(&mut state, Event::Begin);
    send(&mut state, Event::Brightness(10));
    settle(&mut state);
    let _ = update(
        &mut state,
        Message::CloseRequested(iced::window::Id::unique()),
    );
    let draft = state.page_editor.draft.as_ref().unwrap();
    assert!(draft.notice.is_some());
    assert!(draft.error.is_none());
    send(&mut state, Event::Accept);
    assert!(!state.page_editor.is_open());
    assert_eq!(state.pages.current().metadata.brightness, 10);
}

#[test]
fn corner_dragging_keeps_pointer_ownership_and_lost_touch_reverts() {
    use super::canvas::{CropCanvas, CropState};
    use iced::widget::canvas::{Event as CanvasEvent, Program};
    use iced::{Point, Rectangle, mouse, touch};
    let handle = Handle::from_rgba(1, 1, vec![255; 4]);
    let bounds =
        Rectangle::new(Point::new(50.0, 100.0), iced::Size::new(400.0, 400.0));
    let source = CropCanvas {
        image: Some(&handle),
        source_size: Size::new(100.0, 100.0),
        crop: CropQuad::default(),
        corners: None,
        snap: false,
        epoch: 0,
        background: false,
    };
    let rect = fastfill_rust::geometry::fit_contain(
        source.source_size,
        crate::page_canvas::page_viewport(bounds.size()),
    )
    .unwrap()
    .page_rect();
    let start = Point::new(bounds.x + rect.x, bounds.y + rect.y);
    let moved =
        Point::new(start.x + rect.width * 0.1, start.y + rect.height * 0.2);
    let mut input = CropState::default();
    assert!(
        source
            .update(
                &mut input,
                &CanvasEvent::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Left
                )),
                bounds,
                mouse::Cursor::Available(start)
            )
            .is_some()
    );
    let action = source
        .update(
            &mut input,
            &CanvasEvent::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            bounds,
            mouse::Cursor::Available(moved),
        )
        .unwrap();
    let (message, ..) = action.into_inner();
    let Some(Message::PageEdit(Event::Corner(0, point))) = message else {
        panic!("release must publish final corner position")
    };
    assert!((point.x - 0.1).abs() < 0.001 && (point.y - 0.2).abs() < 0.001);
    let touch_event = |id, position| {
        CanvasEvent::Touch(touch::Event::FingerPressed {
            id: touch::Finger(id),
            position,
        })
    };
    assert!(
        source
            .update(
                &mut input,
                &touch_event(1, start),
                bounds,
                mouse::Cursor::Unavailable
            )
            .is_some()
    );
    assert!(
        source
            .update(
                &mut input,
                &touch_event(2, moved),
                bounds,
                mouse::Cursor::Unavailable
            )
            .is_none()
    );
    let lost = CanvasEvent::Touch(touch::Event::FingerLost {
        id: touch::Finger(1),
        position: moved,
    });
    let (message, ..) = source
        .update(&mut input, &lost, bounds, mouse::Cursor::Unavailable)
        .unwrap()
        .into_inner();
    assert!(
        matches!(message, Some(Message::PageEdit(Event::Corner(0, point)))
        if point == NormalizedPoint { x: 0.0, y: 0.0 })
    );
    assert!(
        source
            .update(
                &mut input,
                &touch_event(3, Point::new(bounds.x - 1.0, start.y)),
                bounds,
                mouse::Cursor::Unavailable
            )
            .is_none()
    );
}

#[test]
fn source_corner_overlay_renders_above_image_and_controls_remain_separate() {
    let mut state = state();
    let form = ImportedImage::from_bytes(
        "sample-form.png".into(),
        include_bytes!("../../../../tests/samples/sample-form.png").to_vec(),
        None,
    )
    .unwrap();
    state.pages.replace_image(form);
    send(&mut state, Event::Begin);
    settle(&mut state);
    let pixels = crate::app::tests::render_element(
        super::view(&state),
        iced::Size::new(1_200, 800),
        1,
        1.0,
    );
    let blue = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, p)| {
            index % 1_200 < 850
                && index / 1_200 > 100
                && p[2] > 180
                && p[0] < 30
                && p[1] > 80
                && p[1] < 200
        })
        .count();
    assert!(blue > 500, "corner overlay was hidden by the image: {blue}");
    let heading_ink = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, p)| {
            index % 1_200 < 300
                && index / 1_200 < 60
                && p[..3].iter().all(|channel| *channel < 100)
        })
        .count();
    assert!(heading_ink > 100, "page editor heading must remain visible");
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = std::env::var_os("FASTFILL_PAGE_EDIT_PREVIEW") {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        let image = image::RgbaImage::from_raw(1_200, 800, pixels).unwrap();
        image
            .write_to(
                &mut std::io::BufWriter::new(file),
                image::ImageFormat::Png,
            )
            .unwrap();
    }
}

#[test]
fn rotation_controls_normalize_turn_counts_without_overflow() {
    let mut state = state();
    state.pages.current_mut().metadata.rotation_quarter_turns = i32::MAX;
    send(&mut state, Event::Begin);
    send(&mut state, Event::Rotate(-1));
    assert_eq!(
        state
            .page_editor
            .draft
            .as_ref()
            .unwrap()
            .settings
            .rotation_quarter_turns,
        2
    );
}

#[test]
fn interleaved_annotation_and_page_edits_restore_each_cached_version() {
    let mut state = state();
    let original = state.annotations().shapes()[0].clone();
    let mut versions = vec![(
        state.pages.current().metadata.clone(),
        original.clone(),
        state.image().unwrap().display_handle().clone(),
    )];
    for event in [Event::Brightness(10), Event::Rotate(1)] {
        send(&mut state, Event::Begin);
        send(&mut state, event);
        settle(&mut state);
        send(&mut state, Event::Accept);
        versions.push((
            state.pages.current().metadata.clone(),
            state.annotations().shapes()[0].clone(),
            state.image().unwrap().display_handle().clone(),
        ));
        let _ = update(
            &mut state,
            Message::SetColor(super::super::AnnotationColor::Red),
        );
    }
    // The final color is already red; only the first color change adds undo.
    for index in [1, 1, 0] {
        let _ = update(&mut state, Message::Undo);
        let (metadata, shape, display) = &versions[index];
        assert_eq!(&state.pages.current().metadata, metadata);
        assert_eq!(state.image().unwrap().display_handle(), display);
        if index == 0 {
            assert_eq!(&state.annotations().shapes()[0], shape);
        }
    }
    assert!(!state.project.is_dirty(&state.pages));
    for _ in 0..3 {
        let _ = update(&mut state, Message::Redo);
    }
    assert_eq!(state.pages.current().metadata, versions[2].0);
    assert_eq!(state.annotations().shapes()[0], versions[2].1);
    assert_eq!(state.image().unwrap().display_handle(), &versions[2].2);
}
