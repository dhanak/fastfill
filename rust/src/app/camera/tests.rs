use super::worker::prepare;
use super::*;
use fastfill_rust::annotations::{Pointer, Tool};
use fastfill_rust::geometry::{Point, Size};

mod appearance;

pub(super) fn ready() -> FastFill {
    let mut state = FastFill::default();
    state.project.operation = super::super::project::Operation::Idle;
    let _ = update(&mut state, &Message::Camera(Event::Begin));
    state
}

pub(super) fn frame() -> Arc<RgbaImage> {
    Arc::new(RgbaImage::from_pixel(
        16,
        8,
        image::Rgba([40, 100, 220, 255]),
    ))
}

fn renderer() -> iced::Renderer {
    use iced::advanced::renderer::Headless;
    iced::futures::executor::block_on(<iced::Renderer as Headless>::new(
        iced::Font::with_name("Fira Sans"),
        iced::Pixels(16.0),
        Some("tiny-skia"),
    ))
    .expect("software renderer needs no camera/display/GPU")
}

pub(super) fn display_frame(pixels: Arc<RgbaImage>, arrived: Instant) -> Frame {
    use iced::advanced::image::Renderer as _;
    let handle = Handle::from_rgba(
        pixels.width(),
        pixels.height(),
        pixels.as_raw().clone(),
    );
    Frame {
        allocation: renderer().load_image(&handle).unwrap(),
        pixels,
        arrived,
        wall_time: SystemTime::now(),
    }
}

pub(super) fn present(state: &mut FastFill, id: u64, pixels: Arc<RgbaImage>) {
    let task =
        update(state, &Message::Camera(Event::Frame(id, Ok(pixels)))).unwrap();
    finish_upload(state, task);
}

fn finish_upload(state: &mut FastFill, task: Task<Message>) {
    use iced::advanced::image::Renderer as _;
    use iced::futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let renderer = renderer();
    // Drive the actual image-allocation task, without a window or GPU.
    while let Some(action) = iced::futures::executor::block_on(stream.next()) {
        match action {
            iced_runtime::Action::Image(
                iced_runtime::image::Action::Allocate(handle, sender),
            ) => {
                let _ = sender.send(renderer.load_image(&handle));
            }
            iced_runtime::Action::Output(message) => {
                assert!(matches!(
                    message,
                    Message::Camera(Event::Uploaded(..))
                ));
                let _ = update(state, &message);
            }
            other => panic!("Unexpected upload action: {other:?}"),
        }
    }
}

#[test]
fn full_hd_upload_keeps_the_previous_visible_frame_and_capture_pixels() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    let visible = Arc::new(RgbaImage::from_pixel(
        1920,
        1080,
        image::Rgba([40, 100, 220, 255]),
    ));
    present(&mut state, id, visible.clone());
    let hd = Arc::new(RgbaImage::from_pixel(
        1920,
        1080,
        image::Rgba([200, 100, 40, 255]),
    ));
    state
        .camera
        .draft
        .as_ref()
        .unwrap()
        .pending
        .store(true, Ordering::Relaxed);
    let task =
        update(&mut state, &Message::Camera(Event::Frame(id, Ok(hd)))).unwrap();
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &visible));
    assert!(draft.pending.load(Ordering::Relaxed));
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.captured);
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &visible));
    finish_upload(&mut state, task);
    assert!(Arc::ptr_eq(
        &state
            .camera
            .draft
            .as_ref()
            .unwrap()
            .last
            .as_ref()
            .unwrap()
            .pixels,
        &visible,
    ));
}

#[test]
fn first_full_hd_frame_becomes_capturable_only_after_allocation() {
    let mut state = ready();
    state.camera.draft.as_mut().unwrap().orientation.mirrored = true;
    let id = state.camera.draft.as_ref().unwrap().id;
    let pixels = Arc::new(RgbaImage::from_pixel(
        1920,
        1080,
        image::Rgba([200, 100, 40, 255]),
    ));
    state
        .camera
        .draft
        .as_ref()
        .unwrap()
        .pending
        .store(true, Ordering::Relaxed);
    let task = update(
        &mut state,
        &Message::Camera(Event::Frame(id, Ok(pixels.clone()))),
    )
    .unwrap();
    assert!(!state.camera.draft.as_ref().unwrap().can_capture());
    assert!(state.camera.draft.as_ref().unwrap().last.is_none());
    finish_upload(&mut state, task);
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.can_capture());
    assert!(!draft.pending.load(Ordering::Relaxed));
    let frame = draft.last.as_ref().unwrap();
    assert!(Arc::ptr_eq(&frame.pixels, &pixels));
    assert_eq!(frame.allocation.size(), iced::Size::new(1920, 1080));
    assert!(frame.allocation.downgrade().upgrade().is_some());
    let saved = prepare(&frame.pixels).unwrap();
    assert_eq!((saved.info.width, saved.info.height), (1920, 1080));
}

#[test]
fn failed_upload_keeps_visible_pixels_and_allows_the_next_frame() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    let visible = frame();
    present(&mut state, id, visible.clone());
    state
        .camera
        .draft
        .as_ref()
        .unwrap()
        .pending
        .store(true, Ordering::Relaxed);
    let _ = update(
        &mut state,
        &Message::Camera(Event::Uploaded(
            id,
            Err("Synthetic upload error".into()),
        )),
    );
    let draft = state.camera.draft.as_ref().unwrap();
    assert_eq!(draft.error.as_deref(), Some("Synthetic upload error"));
    assert!(!draft.pending.load(Ordering::Relaxed));
    assert!(draft.can_capture());
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &visible));
    let next = frame();
    present(&mut state, id, next.clone());
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.error.is_none());
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &next));
}

#[test]
fn first_preview_frame_enables_immediate_capture() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    assert!(!state.camera.draft.as_ref().unwrap().can_capture());
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    assert!(!state.camera.draft.as_ref().unwrap().captured);
    let pixels = frame();
    present(&mut state, id, pixels.clone());
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.last.is_some());
    assert!(draft.can_capture());
    let _ = update(&mut state, &Message::FocusLost);
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.can_capture());
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    assert!(state.camera.draft.as_ref().unwrap().captured);
}

#[test]
fn canceled_preview_keeps_document_and_rejects_late_frames() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    let before = state.pages.identity();
    state
        .camera
        .draft
        .as_ref()
        .unwrap()
        .pending
        .store(true, Ordering::Relaxed);
    present(&mut state, id, frame());
    assert!(state.camera.draft.as_ref().unwrap().last.is_some());
    assert!(
        !state
            .camera
            .draft
            .as_ref()
            .unwrap()
            .pending
            .load(Ordering::Relaxed)
    );
    let _ = update(&mut state, &Message::SetTool(Tool::Rectangle));
    assert_eq!(state.annotations().tool(), Tool::Navigate);
    let _ = update(&mut state, &Message::Camera(Event::Cancel));
    let _ = update(&mut state, &Message::Camera(Event::Frame(id, Ok(frame()))));
    assert!(!state.camera.is_open());
    assert_eq!(state.pages.identity(), before);
}

#[test]
fn capture_freezes_visible_frame_and_accept_adds_without_discarding() {
    let mut state = FastFill::default();
    state.annotations_mut().set_tool(Tool::Rectangle);
    assert!(state.annotations_mut().begin(
        Pointer::Mouse,
        Point::new(20.0, 20.0),
        super::super::DEFAULT_PAGE,
    ));
    state.annotations_mut().finish(
        Pointer::Mouse,
        Point::new(80.0, 60.0),
        false,
    );
    let original = state.pages.identity();
    let _ = update(&mut state, &Message::Camera(Event::Begin));
    let id = state.camera.draft.as_ref().unwrap().id;
    let pixels = frame();
    present(&mut state, id, pixels.clone());
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    let _ = update(&mut state, &Message::Camera(Event::Frame(id, Ok(frame()))));
    assert!(state.camera.draft.as_ref().unwrap().captured);
    assert!(Arc::ptr_eq(
        &state
            .camera
            .draft
            .as_ref()
            .unwrap()
            .last
            .as_ref()
            .unwrap()
            .pixels,
        &pixels,
    ));
    let image = prepare(&pixels).unwrap();
    assert_eq!(image.size, Size::new(16.0, 8.0));
    // Only a requested encoding completion may alter the project.
    let _ = update(
        &mut state,
        &Message::Camera(Event::Prepared(id, Ok(image.clone()))),
    );
    assert_eq!(state.pages.identity(), original);
    state.camera.draft.as_mut().unwrap().saving = true;
    let _ =
        update(&mut state, &Message::Camera(Event::Prepared(id, Ok(image))));
    assert_eq!(state.pages.len(), 2);
    assert_eq!(state.pages.identity()[0], original[0]);
    assert!(state.page_editor.is_open());
    assert!(!state.camera.is_open());
    let added = state.pages.identity();
    let _ = super::super::page_edit::update(
        &mut state,
        &Message::PageEdit(super::super::page_edit::Event::Brightness(50)),
    );
    let _ = super::super::page_edit::update(
        &mut state,
        &Message::PageEdit(super::super::page_edit::Event::Cancel),
    );
    assert!(!state.page_editor.is_open());
    assert_eq!(state.pages.identity(), added);
    assert_eq!(state.pages.current().metadata.brightness, 0);
    assert!(state.image().is_some());
}

#[test]
fn preview_and_discovery_failures_are_recoverable() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    let _ = update(
        &mut state,
        &Message::Camera(Event::Frame(id, Err("Permission denied".into()))),
    );
    assert_eq!(
        state.camera.draft.as_ref().unwrap().error.as_deref(),
        Some("Permission denied")
    );
    let _ = update(&mut state, &Message::Camera(Event::Refresh));
    let id = state.camera.draft.as_ref().unwrap().id;
    let _ = update(
        &mut state,
        &Message::Camera(Event::Devices(
            id,
            Err("Camera access denied".into()),
        )),
    );
    assert!(state.camera.draft.as_ref().unwrap().error.is_some());
    assert!(!state.camera.draft.as_ref().unwrap().sources.loading);
    let _ = update(&mut state, &Message::Camera(Event::Refresh));
    assert!(state.camera.draft.as_ref().unwrap().error.is_none());
    let before = state.pages.identity();
    let _ = update(&mut state, &Message::CancelInteraction);
    assert!(!state.camera.is_open());
    assert_eq!(state.pages.identity(), before);
}

#[test]
fn focus_loss_and_window_moves_keep_preview_and_capture_state() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    let pixels = frame();
    present(&mut state, id, pixels.clone());
    let _ = update(&mut state, &Message::FocusLost);
    let _ = update(&mut state, &Message::ViewportResized);
    let draft = state.camera.draft.as_ref().unwrap();
    assert_eq!(draft.id, id);
    assert!(draft.error.is_none());
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &pixels));
    let _ = update(&mut state, &Message::Camera(Event::Frame(id, Ok(frame()))));
    assert!(state.camera.draft.as_ref().unwrap().last.is_some());
}

#[test]
fn canceled_encoding_rejects_stale_completions() {
    let mut state = ready();
    let id = state.camera.draft.as_ref().unwrap().id;
    let before = state.pages.identity();
    let _ = update(&mut state, &Message::Camera(Event::Cancel));
    let _ = update(
        &mut state,
        &Message::Camera(Event::Prepared(id, Ok(prepare(&frame()).unwrap()))),
    );
    assert_eq!(state.pages.identity(), before);
    assert!(!state.camera.is_open());
}

#[test]
fn mirrored_camera_review_stays_mirrored_until_use_photo() {
    let mut state = ready();
    let draft = state.camera.draft.as_mut().unwrap();
    draft.orientation.mirrored = true;
    draft.sources.loading = false;
    let id = draft.id;
    let pixels = Arc::new(RgbaImage::from_fn(400, 200, |x, _| {
        if x < 200 {
            image::Rgba([220, 0, 0, 255])
        } else {
            image::Rgba([0, 0, 220, 255])
        }
    }));
    present(&mut state, id, pixels.clone());
    let assert_mirrored_review = |state: &FastFill| {
        let rendered = super::super::tests::render_element(
            view(state),
            iced::Size::new(800, 1000),
            1,
            1.0,
        );
        let at = (500 * 800 + 200) * 4;
        assert_eq!(&rendered[at..at + 4], [0, 0, 220, 255]);
    };
    assert_mirrored_review(&state);
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    assert_mirrored_review(&state);
    let _ = update(&mut state, &Message::Camera(Event::UsePhoto));
    assert_mirrored_review(&state);
    let photo = prepare(&pixels).unwrap();
    let _ =
        update(&mut state, &Message::Camera(Event::Prepared(id, Ok(photo))));
    assert!(!state.camera.is_open());
    assert!(state.page_editor.is_open());
    let Handle::Bytes(_, bytes) = &state.image().unwrap().handle else {
        panic!("Page editor must receive original photo bytes");
    };
    assert_eq!(image::load_from_memory(bytes).unwrap().to_rgba8(), *pixels);
}

#[test]
fn page_limit_and_other_modal_operations_prevent_opening_camera() {
    let mut state = FastFill {
        confirm_delete_all: true,
        ..FastFill::default()
    };
    let _ = update(&mut state, &Message::Camera(Event::Begin));
    assert!(!state.camera.is_open());
    state.confirm_delete_all = false;
    let image = prepare(&frame()).unwrap();
    assert!(state.pages.import_images(&vec![image; 20]));
    let _ = update(&mut state, &Message::Camera(Event::Begin));
    assert!(!state.camera.is_open());
    assert_eq!(state.pages.len(), 20);
}
