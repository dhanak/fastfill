use super::super::canvas::{CropCanvas, CropState};
use super::*;
use fastfill_rust::form_snap::CornerFeatures;
use iced::widget::canvas::{Event as InputEvent, Program};
use iced::{Point, Rectangle, keyboard, mouse, touch};

fn pixels() -> image::RgbaImage {
    image::RgbaImage::from_fn(320, 240, |x, y| {
        let value = if (60..260).contains(&x) && (50..190).contains(&y) {
            240
        } else {
            40
        };
        image::Rgba([value, value, value, 255])
    })
}

fn source<'a>(
    image: &'a Handle,
    corners: &'a CornerFeatures,
) -> CropCanvas<'a> {
    CropCanvas {
        image: Some(image),
        source_size: Size::new(320.0, 240.0),
        crop: CropQuad::default(),
        corners: Some(corners),
        snap: false,
        epoch: 0,
        background: false,
        mode: viewer::Mode::Frame,
        view: viewer::ViewState::default(),
        rotation: 0,
    }
}

fn position(bounds: Rectangle, point: NormalizedPoint) -> Point {
    let rect = fastfill_rust::geometry::fit_contain(
        Size::new(320.0, 240.0),
        crate::page_canvas::page_viewport(bounds.size()),
    )
    .unwrap()
    .page_rect();
    Point::new(
        bounds.x + rect.x + point.x * rect.width,
        bounds.y + rect.y + point.y * rect.height,
    )
}

fn emit(
    source: &CropCanvas<'_>,
    input: &mut CropState,
    bounds: Rectangle,
    event: &InputEvent,
    cursor: mouse::Cursor,
) -> Option<NormalizedPoint> {
    let (message, ..) =
        source.update(input, event, bounds, cursor)?.into_inner();
    let Some(Message::PageEdit(Event::Corner(index, point))) = message else {
        return None;
    };
    assert_eq!(index, 0);
    Some(point)
}

#[test]
fn corner_drag_survives_view_rebuild_after_each_move() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let mut page = state();
    send(&mut page, Event::Begin);
    let draft = page.page_editor.draft.as_ref().unwrap();
    let canvas = CropCanvas {
        epoch: page.input_epoch.wrapping_add(draft.viewer.epoch),
        ..source(&handle, &features)
    };
    let bounds =
        Rectangle::new(Point::new(50.0, 70.0), iced::Size::new(640.0, 400.0));
    let start = position(bounds, canvas.crop.top_left);
    let first = position(bounds, NormalizedPoint { x: 0.1, y: 0.1 });
    let second = position(bounds, NormalizedPoint { x: 0.2, y: 0.2 });
    let mut input = CropState::default();
    emit(
        &canvas,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        mouse::Cursor::Available(start),
    );
    let point = emit(
        &canvas,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::CursorMoved { position: first }),
        mouse::Cursor::Available(first),
    )
    .unwrap();
    send(&mut page, Event::Corner(0, point));
    let draft = page.page_editor.draft.as_ref().unwrap();
    let rebuilt = CropCanvas {
        crop: draft.settings.crop,
        epoch: page.input_epoch.wrapping_add(draft.viewer.epoch),
        ..canvas
    };
    let point = emit(
        &rebuilt,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::CursorMoved { position: second }),
        mouse::Cursor::Available(second),
    )
    .unwrap();
    assert!((point.x - 0.2).abs() < 0.001);
    assert!((point.y - 0.2).abs() < 0.001);
}

#[test]
fn rotated_frame_drag_updates_the_original_corner() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let source = CropCanvas {
        rotation: 1,
        ..source(&handle, &features)
    };
    let bounds =
        Rectangle::new(Point::new(40.0, 60.0), iced::Size::new(600.0, 600.0));
    let rect = fastfill_rust::geometry::fit_contain(
        Size::new(240.0, 320.0),
        crate::page_canvas::page_viewport(bounds.size()),
    )
    .unwrap()
    .page_rect();
    let start = Point::new(bounds.x + rect.x + rect.width, bounds.y + rect.y);
    let moved = Point::new(
        bounds.x + rect.x + rect.width * 0.7,
        bounds.y + rect.y + rect.height * 0.2,
    );
    let mut input = CropState::default();
    emit(
        &source,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        mouse::Cursor::Available(start),
    );
    let point = emit(
        &source,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::CursorMoved { position: moved }),
        mouse::Cursor::Available(moved),
    )
    .unwrap();
    assert!((point.x - 0.2).abs() < 0.001);
    assert!((point.y - 0.3).abs() < 0.001);
}

#[test]
fn wheel_and_touch_pan_navigate_without_changing_crop() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let source = source(&handle, &features);
    let bounds =
        Rectangle::new(Point::new(40.0, 60.0), iced::Size::new(600.0, 500.0));
    let center = bounds.center();
    let mut input = CropState::default();
    let wheel = InputEvent::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
    });
    let (message, ..) = source
        .update(&mut input, &wheel, bounds, mouse::Cursor::Available(center))
        .unwrap()
        .into_inner();
    let Some(Message::PageEdit(Event::View(viewer::Event::Navigate {
        delta,
        ..
    }))) = message
    else {
        panic!("wheel should zoom")
    };
    assert!(delta.zoom_factor > 1.0);

    let press = InputEvent::Touch(touch::Event::FingerPressed {
        id: touch::Finger(5),
        position: center,
    });
    let move_to = InputEvent::Touch(touch::Event::FingerMoved {
        id: touch::Finger(5),
        position: Point::new(center.x + 20.0, center.y),
    });
    source
        .update(&mut input, &press, bounds, mouse::Cursor::Unavailable)
        .unwrap();
    let (message, ..) = source
        .update(&mut input, &move_to, bounds, mouse::Cursor::Unavailable)
        .unwrap()
        .into_inner();
    let Some(Message::PageEdit(Event::View(viewer::Event::Navigate {
        delta,
        ..
    }))) = message
    else {
        panic!("touch should pan")
    };
    assert!((delta.pan.x - 20.0).abs() < 0.01);
    assert_eq!(source.crop, CropQuad::default());
}

#[test]
fn ctrl_changes_snap_and_unsnap_without_moving_the_pointer() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let source = source(&handle, &features);
    let bounds =
        Rectangle::new(Point::new(50.0, 100.0), iced::Size::new(640.0, 400.0));
    let mut input = CropState::default();
    let start = position(bounds, source.crop.top_left);
    let raw = NormalizedPoint { x: 0.2, y: 0.23 };
    let moved = position(bounds, raw);
    emit(
        &source,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        mouse::Cursor::Available(start),
    );
    let manual = emit(
        &source,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::CursorMoved { position: moved }),
        mouse::Cursor::Available(moved),
    )
    .unwrap();
    assert!((manual.x - raw.x).abs() < 0.0001);
    let target = features.find_corner(raw, 0.09).unwrap();
    let ctrl = InputEvent::Keyboard(keyboard::Event::ModifiersChanged(
        keyboard::Modifiers::CTRL,
    ));
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &ctrl,
            mouse::Cursor::Unavailable
        ),
        Some(target)
    );
    // Input may arrive again before the root modifier state rebuilds the view.
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &InputEvent::Mouse(mouse::Event::CursorMoved { position: moved }),
            mouse::Cursor::Available(moved)
        ),
        Some(target)
    );
    let release = InputEvent::Keyboard(keyboard::Event::ModifiersChanged(
        keyboard::Modifiers::empty(),
    ));
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &release,
            mouse::Cursor::Unavailable
        ),
        Some(manual)
    );
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &ctrl,
            mouse::Cursor::Unavailable
        ),
        Some(target)
    );
    let up =
        InputEvent::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &up,
            mouse::Cursor::Available(moved)
        ),
        Some(target)
    );
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &release,
            mouse::Cursor::Unavailable
        ),
        None
    );
}

#[test]
fn touch_lost_restores_unsnapped_original_and_other_fingers_are_ignored() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let mut source = source(&handle, &features);
    source.snap = true;
    source.crop.top_left = NormalizedPoint { x: 0.17, y: 0.19 };
    let bounds =
        Rectangle::new(Point::new(20.0, 40.0), iced::Size::new(400.0, 640.0));
    let mut input = CropState::default();
    let cursor = mouse::Cursor::Unavailable;
    let start = position(bounds, source.crop.top_left);
    emit(
        &source,
        &mut input,
        bounds,
        &InputEvent::Touch(touch::Event::FingerPressed {
            id: touch::Finger(1),
            position: start,
        }),
        cursor,
    );
    let raw = NormalizedPoint { x: 0.2, y: 0.23 };
    let moved = position(bounds, raw);
    let target = features.find_corner(raw, 0.09).unwrap();
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &InputEvent::Touch(touch::Event::FingerMoved {
                id: touch::Finger(2),
                position: moved
            }),
            cursor
        ),
        None
    );
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &InputEvent::Touch(touch::Event::FingerMoved {
                id: touch::Finger(1),
                position: moved
            }),
            cursor
        ),
        Some(target)
    );
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &InputEvent::Touch(touch::Event::FingerLost {
                id: touch::Finger(1),
                position: moved
            }),
            cursor
        ),
        Some(source.crop.top_left)
    );
}

#[test]
fn missing_hints_and_invalid_snapped_quads_keep_manual_dragging_available() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let mut source = source(&handle, &features);
    source.snap = true;
    let raw = NormalizedPoint { x: 0.17, y: 0.20 };
    source.crop.top_left = raw;
    source.crop.top_right = features.find_corner(raw, 0.09).unwrap();
    let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(640.0, 400.0));
    let mut input = CropState::default();
    let start = position(bounds, raw);
    emit(
        &source,
        &mut input,
        bounds,
        &InputEvent::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        mouse::Cursor::Available(start),
    );
    let move_event =
        InputEvent::Mouse(mouse::Event::CursorMoved { position: start });
    let manual = emit(
        &source,
        &mut input,
        bounds,
        &move_event,
        mouse::Cursor::Available(start),
    )
    .unwrap();
    assert!((manual.x - raw.x).abs() < 0.0001);
    assert!((manual.y - raw.y).abs() < 0.0001);
    let source = CropCanvas {
        corners: None,
        ..source
    };
    assert_eq!(
        emit(
            &source,
            &mut input,
            bounds,
            &move_event,
            mouse::Cursor::Available(start)
        ),
        Some(manual)
    );
    let up =
        InputEvent::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
    assert_eq!(
        emit(&source, &mut input, bounds, &up, mouse::Cursor::Unavailable),
        Some(raw)
    );
}

#[test]
fn original_hints_are_reused_across_corrections_not_derived_from_preview() {
    let original = preview::build(
        Input::Image(Handle::from_rgba(320, 240, pixels().into_raw())),
        PageCorrection::default(),
    )
    .unwrap()
    .original
    .clone();
    assert_eq!(original.corners.as_ref().unwrap().corners().len(), 4);
    let correction = PageCorrection {
        rotation_quarter_turns: 1,
        brightness: 100,
        ..PageCorrection::default()
    };
    let changed =
        preview::build(Input::Decoded(original.clone()), correction).unwrap();
    assert!(Arc::ptr_eq(&changed.original, &original));
    assert!(matches!(
        changed.handle,
        Handle::Rgba {
            width: 240,
            height: 320,
            ..
        }
    ));
    assert_eq!(
        changed.original.corners.as_ref().unwrap().analysis_size(),
        (320, 240)
    );
    let failed = Arc::new(Original {
        pixels: pixels(),
        handle: original.handle.clone(),
        corners: Err("Test failure".into()),
        framing: std::sync::OnceLock::new(),
    });
    assert!(preview::build(Input::Decoded(failed), correction).is_ok());
}

#[test]
fn legacy_radius_tracks_the_displayed_image_not_the_window_or_preview() {
    let features = CornerFeatures::analyze(&pixels()).unwrap();
    let handle = Handle::from_rgba(320, 240, pixels().into_raw());
    let source = CropCanvas {
        snap: true,
        ..source(&handle, &features)
    };
    let raw = NormalizedPoint { x: 0.23, y: 0.22 };
    for size in [400.0, 4000.0] {
        let bounds =
            Rectangle::new(Point::new(50.0, 70.0), iced::Size::new(size, size));
        let mut input = CropState::default();
        let start = position(bounds, source.crop.top_left);
        emit(
            &source,
            &mut input,
            bounds,
            &InputEvent::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            mouse::Cursor::Available(start),
        );
        let moved = position(bounds, raw);
        let point = emit(
            &source,
            &mut input,
            bounds,
            &InputEvent::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            mouse::Cursor::Available(moved),
        )
        .unwrap();
        if size < 1000.0 {
            assert_eq!(Some(point), features.find_corner(raw, 0.09));
        } else {
            assert!((point.x - raw.x).abs() < 0.0001);
            assert!((point.y - raw.y).abs() < 0.0001);
        }
    }
}

#[test]
fn snapped_crops_commit_as_one_history_step_and_cancel_keeps_the_page() {
    let mut bytes = Cursor::new(Vec::new());
    pixels()
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let mut state = FastFill::default();
    state.pages.add_image(
        ImportedImage::from_bytes(
            "rectangle.png".into(),
            bytes.into_inner(),
            None,
        )
        .unwrap(),
    );
    mark_saved(&mut state);
    let before = state.pages.current().metadata.clone();
    send(&mut state, Event::Begin);
    settle(&mut state);
    let target = state
        .page_editor
        .draft
        .as_ref()
        .unwrap()
        .original
        .as_ref()
        .unwrap()
        .corners
        .as_ref()
        .unwrap()
        .find_corner(NormalizedPoint { x: 0.2, y: 0.23 }, 0.09)
        .unwrap();
    send(&mut state, Event::Corner(0, target));
    assert_eq!(state.pages.current().metadata, before);
    assert!(!state.project.is_dirty(&state.pages));
    settle(&mut state);
    send(&mut state, Event::Accept);
    assert_eq!(state.pages.current().metadata.crop.top_left, target);
    let accepted = state.pages.current().metadata.clone();
    let identity = state.pages.identity();
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.pages.current().metadata, before);
    assert!(!state.project.is_dirty(&state.pages));
    let _ = update(&mut state, Message::Redo);
    assert_eq!(state.pages.current().metadata, accepted);
    send(&mut state, Event::Begin);
    send(&mut state, Event::FullFrame);
    send(&mut state, Event::Cancel);
    assert_eq!(state.pages.current().metadata, accepted);
    assert_eq!(state.pages.identity(), identity);
}
