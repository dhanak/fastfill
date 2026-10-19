use super::*;
use crate::app::controller::update;
use crate::app::tests::paste_text;
use crate::picker::ImportedImage;
use fastfill_rust::annotations::{AnnotationColor, Pointer};
use iced::widget::image::Handle;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

mod text_reuse;

fn image(name: &str, shift: u32) -> ImportedImage {
    let mut pixels = RgbaImage::from_pixel(800, 600, Rgba([255; 4]));
    for y in 100..144 {
        for x in 120 + shift..164 + shift {
            if !(103..141).contains(&y)
                || !(123 + shift..161 + shift).contains(&x)
            {
                pixels.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    for y in 359..362 {
        for x in 240 + shift..641 + shift {
            pixels.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let mut encoded = Cursor::new(Vec::new());
    pixels.write_to(&mut encoded, ImageFormat::Png).unwrap();
    ImportedImage::from_bytes(name.into(), encoded.into_inner(), None).unwrap()
}

fn fixture(tool: Tool) -> FastFill {
    let mut state = FastFill::default();
    state.pages.add_image(image("form.png", 0));
    let _ = update(&mut state, Message::SetTool(tool));
    state
}

fn send(state: &mut FastFill, event: Event) {
    let _ = update(state, Message::SmartSnap(event));
}

fn settle(state: &mut FastFill) {
    let id = state.smart_snap.running.as_ref().unwrap().id;
    let result = worker::analyze(state.image().unwrap().display_handle());
    send(state, Event::Ready(id, result));
}

fn click(state: &mut FastFill, pointer: Pointer, x: f32, y: f32, scale: f32) {
    let size = super::super::page_size(state);
    let point = Point::new(x / 800.0 * size.width, y / 600.0 * size.height);
    let _ = update(state, Message::DrawPressed(pointer, point, scale));
    let _ = update(state, Message::DrawReleased(pointer, point, false));
}

fn same_bounds(a: Rect, b: Rect) {
    for delta in [a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height]
    {
        assert!(delta.abs() < 0.001, "{a:?} != {b:?}");
    }
}

#[test]
fn snap_state_is_global_and_never_part_of_document_history() {
    let mut state = fixture(Tool::Checkmark);
    let identity = state.pages.identity();
    assert!(!state.smart_snap.enabled);
    send(&mut state, Event::Toggle);
    assert!(state.smart_snap.enabled);
    let job = state.smart_snap.running.as_ref().unwrap().id;
    for tool in [Tool::Text, Tool::Select, Tool::Cross] {
        let _ = update(&mut state, Message::SetTool(tool));
        assert!(state.smart_snap.enabled);
        assert_eq!(state.smart_snap.running.as_ref().unwrap().id, job);
    }
    assert_eq!(state.pages.identity(), identity);
    assert!(!state.annotations().can_undo());
    settle(&mut state);
    let _ = update(&mut state, Message::NewProject);
    let _ = update(&mut state, Message::DiscardProject);
    assert!(state.smart_snap.enabled);
    assert!(state.smart_snap.cache.is_empty());
    assert!(state.smart_snap.running.is_none());
}

#[test]
fn clicked_marks_snap_to_inset_boxes_independently_of_zoom() {
    for tool in [Tool::Checkmark, Tool::Cross] {
        for scale in [0.25, 1.0, 4.0] {
            for pointer in [Pointer::Mouse, Pointer::Touch(9)] {
                let mut state = fixture(tool);
                state.annotations_mut().set_color(AnnotationColor::Green);
                state.annotations_mut().set_stroke_width(2.0);
                send(&mut state, Event::Toggle);
                settle(&mut state);
                click(&mut state, pointer, 145.0, 120.0, scale);
                let mark =
                    state.annotations().shapes().cloned().collect::<Vec<_>>()
                        [0]
                    .clone();
                let bounds = mark.bounds();
                assert!((25.0..28.0).contains(&bounds.width));
                assert_eq!(bounds.width, bounds.height);
                assert!((bounds.center().x - 108.63).abs() < 1.0);
                assert!((bounds.center().y - 93.33).abs() < 1.0);
                assert_eq!(mark.color, AnnotationColor::Green);
                assert_eq!(mark.stroke_width, 2.0);
                let _ = update(&mut state, Message::Undo);
                assert!(state.annotations().is_empty());
                assert!(!state.annotations().can_undo());
                let _ = update(&mut state, Message::Redo);
                assert_eq!(
                    state.annotations().shapes().cloned().collect::<Vec<_>>(),
                    [mark]
                );
            }
        }
    }
}

#[test]
fn disabled_pending_and_blank_area_placements_stay_ordinary() {
    let mut state = fixture(Tool::Cross);
    click(&mut state, Pointer::Mouse, 145.0, 120.0, 1.0);
    assert_eq!(
        state.annotations().shapes().next().unwrap().bounds().width,
        612.0 / 32.0
    );
    send(&mut state, Event::Toggle);
    click(&mut state, Pointer::Mouse, 250.0, 200.0, 1.0);
    let before = state.annotations().shapes().cloned().collect::<Vec<_>>();
    settle(&mut state);
    assert_eq!(
        state.annotations().shapes().cloned().collect::<Vec<_>>(),
        before
    );
    click(&mut state, Pointer::Mouse, 700.0, 500.0, 1.0);
    assert_eq!(
        state.annotations().shapes().nth(2).unwrap().bounds().width,
        612.0 / 32.0
    );
}

#[test]
fn text_snaps_above_the_rule_but_editing_it_does_not_resnap() {
    let mut state = fixture(Tool::Text);
    send(&mut state, Event::Toggle);
    settle(&mut state);
    click(&mut state, Pointer::Mouse, 450.0, 350.0, 1.0);
    paste_text(&mut state, "Dátum");
    let first = state.annotations().text_preview().unwrap();
    assert!((183.0..186.0).contains(&first.bounds.x));
    assert!((274.0..277.0).contains(&(first.bounds.y + first.bounds.height)));
    let _ = update(&mut state, Message::ToggleTextBold);
    let _ = update(&mut state, Message::SetTextSize(32.0));
    let next = state.annotations().text_preview().unwrap();
    assert!(
        (first.bounds.y + first.bounds.height
            - next.bounds.y
            - next.bounds.height)
            .abs()
            < 0.001
    );
    let _ = update(&mut state, Message::AcceptText);
    let text = state.annotations().texts().next().unwrap().clone();
    let point = text.bounds.center();
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    assert_eq!(
        state.annotations().text_preview().unwrap().bounds,
        text.bounds
    );
    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(
        state.annotations().texts().cloned().collect::<Vec<_>>(),
        [text]
    );
}

#[test]
fn stale_results_never_supply_features_for_a_replaced_image_or_project() {
    let mut state = fixture(Tool::Checkmark);
    send(&mut state, Event::Toggle);
    let first = state.smart_snap.running.as_ref().unwrap().id;
    let old_result = worker::analyze(state.image().unwrap().display_handle());
    state.pages.replace_image(image("new.png", 40));
    send(&mut state, Event::Ready(first, old_result.clone()));
    assert!(state.smart_snap.cache.is_empty());
    let second = state.smart_snap.running.as_ref().unwrap().id;
    assert_ne!(first, second);
    send(&mut state, Event::Ready(first, old_result));
    assert_eq!(state.smart_snap.running.as_ref().unwrap().id, second);
    state.project.header.id = uuid::Uuid::new_v4();
    settle(&mut state);
    assert!(state.smart_snap.cache.is_empty());
    settle(&mut state);
    click(&mut state, Pointer::Mouse, 182.0, 122.0, 1.0);
    assert!(
        (state
            .annotations()
            .shapes()
            .next()
            .unwrap()
            .bounds()
            .center()
            .x
            - 182.0 / 800.0 * 612.0)
            .abs()
            < 1.0
    );
}

#[test]
fn worker_rejects_unloaded_or_invalid_pixels() {
    for input in [
        Handle::from_path("not-loaded.png"),
        Handle::from_bytes(Vec::new()),
        Handle::from_rgba(800, 600, Vec::new()),
        Handle::from_rgba(50_000_001, 1, Vec::new()),
    ] {
        assert!(worker::analyze(&input).is_err());
    }
}

#[test]
fn rejected_presses_cannot_change_an_active_marks_frozen_target() {
    for duplicate in [Pointer::Mouse, Pointer::Touch(2)] {
        let mut state = fixture(Tool::Cross);
        send(&mut state, Event::Toggle);
        settle(&mut state);
        let start = Point::new(250.0 * 0.765, 200.0 * 0.765);
        let other = Point::new(145.0 * 0.765, 120.0 * 0.765);
        let _ = update(
            &mut state,
            Message::DrawPressed(Pointer::Mouse, start, 1.0),
        );
        let original = state.annotations().preview().unwrap();
        let _ = update(&mut state, Message::DrawPressed(duplicate, other, 1.0));
        assert_eq!(state.annotations().preview().unwrap(), original);
        let _ = update(
            &mut state,
            Message::DrawReleased(Pointer::Mouse, start, false),
        );
        assert_eq!(
            state.annotations().shapes().cloned().collect::<Vec<_>>(),
            [original]
        );
    }
}

#[test]
fn page_changes_coalesce_and_reuse_only_live_image_results() {
    let mut state = fixture(Tool::Checkmark);
    send(&mut state, Event::Toggle);
    let first = state.smart_snap.running.as_ref().unwrap().id;
    let first_key = key(&state).unwrap();
    let result = worker::analyze(state.image().unwrap().display_handle());
    for shift in [40, 80] {
        state.pages.add_image(image("next.png", shift));
        let _ = update(&mut state, Message::SetTool(Tool::Checkmark));
        assert_eq!(state.smart_snap.running.as_ref().unwrap().id, first);
    }
    let latest_key = key(&state).unwrap();
    send(&mut state, Event::Ready(first, result));
    assert!(state.smart_snap.cache.contains_key(&first_key));
    assert_eq!(state.smart_snap.running.as_ref().unwrap().key, latest_key);
    settle(&mut state);
    assert_eq!(state.smart_snap.cache.len(), 2);
    let _ = update(&mut state, Message::SelectPage(0));
    assert_eq!(status(&state), "Ready");
    assert!(state.smart_snap.running.is_none());
    assert!(state.smart_snap.enabled);
    let _ = update(&mut state, Message::SelectPage(1));
    assert_eq!(status(&state), "Analyzing…");
    settle(&mut state);
    state.pages.delete_active();
    let _ = update(&mut state, Message::SetTool(Tool::Cross));
    assert_eq!(state.smart_snap.cache.len(), 2);
}

#[test]
fn correction_and_undo_use_display_pixels_instead_of_the_original_asset() {
    use fastfill_rust::correction::PageCorrection;

    let mut state = fixture(Tool::Cross);
    let source = state.image().unwrap().handle.id();
    send(&mut state, Event::Toggle);
    settle(&mut state);
    let original_key = key(&state).unwrap();
    // Different display pixels expose accidental original-asset cache reuse.
    let display = image("corrected.png", 40).display_handle().clone();
    state
        .pages
        .current_mut()
        .accept_correction(
            PageCorrection {
                brightness: 1,
                ..PageCorrection::default()
            },
            display,
        )
        .unwrap();
    let _ = update(&mut state, Message::SetTool(Tool::Cross));
    assert_eq!(state.image().unwrap().handle.id(), source);
    assert_ne!(key(&state).unwrap(), original_key);
    assert!(state.smart_snap.cache.is_empty());
    settle(&mut state);
    click(&mut state, Pointer::Mouse, 182.0, 122.0, 1.0);
    let mark = state.annotations().shapes().next().unwrap().clone();
    assert!((mark.bounds().center().x - 139.23).abs() < 1.0);
    let _ = update(&mut state, Message::Undo);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(key(&state).unwrap(), original_key);
    assert_eq!(status(&state), "Analyzing…");
    settle(&mut state);
    let _ = update(&mut state, Message::Redo);
    settle(&mut state);
    let _ = update(&mut state, Message::Redo);
    assert_eq!(
        state.annotations().shapes().cloned().collect::<Vec<_>>(),
        [mark]
    );
}

#[test]
fn failed_analysis_does_not_spin_and_toggle_retries_without_late_snaps() {
    let mut state = fixture(Tool::Cross);
    send(&mut state, Event::Toggle);
    let id = state.smart_snap.running.as_ref().unwrap().id;
    send(&mut state, Event::Ready(id, Err("Test failure.".into())));
    assert_eq!(status(&state), "Unavailable: Test failure.");
    assert!(state.smart_snap.running.is_none());
    click(&mut state, Pointer::Mouse, 145.0, 120.0, 1.0);
    assert_eq!(
        state.annotations().shapes().next().unwrap().bounds().width,
        19.125
    );
    assert!(state.smart_snap.running.is_none());
    send(&mut state, Event::Toggle);
    assert_eq!(status(&state), "Off");
    send(&mut state, Event::Toggle);
    assert_eq!(status(&state), "Analyzing…");
    let start = Point::new(182.0 * 0.765, 122.0 * 0.765);
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, start, 1.0));
    let draft = state.annotations().preview();
    send(&mut state, Event::Toggle);
    assert!(state.smart_snap.enabled, "active gesture blocks toggling");
    settle(&mut state);
    assert_eq!(state.annotations().preview(), draft);
    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(state.annotations().len(), 1);
}

#[test]
fn results_can_finish_inside_a_modal_without_unlocking_or_mutating_it() {
    let mut state = fixture(Tool::Text);
    send(&mut state, Event::Toggle);
    let identity = state.pages.identity();
    let _ = update(
        &mut state,
        Message::PageEdit(crate::app::page_edit::Event::Begin),
    );
    assert!(state.page_editor.is_open());
    send(&mut state, Event::Toggle);
    assert!(state.smart_snap.enabled);
    settle(&mut state);
    assert!(state.smart_snap.running.is_none());
    assert!(state.page_editor.is_open());
    assert_eq!(state.pages.identity(), identity);
}

#[test]
fn text_line_placement_is_zoom_independent_and_one_persisted_creation() {
    use crate::app::project_io::{decode_project, encode_snapshot};
    use fastfill_rust::project::save_archive;

    let mut first_bounds = None;
    for scale in [0.25, 1.0, 4.0] {
        let mut state = fixture(Tool::Checkmark);
        send(&mut state, Event::Toggle);
        settle(&mut state);
        click(&mut state, Pointer::Mouse, 145.0, 120.0, scale);
        let _ = update(&mut state, Message::SetTool(Tool::Text));
        click(&mut state, Pointer::Mouse, 450.0, 350.0, scale);
        paste_text(&mut state, "Unwrapped text\nSecond line");
        let _ = update(&mut state, Message::SetTextSize(22.0));
        let _ = update(&mut state, Message::AcceptText);
        let bounds = state.annotations().texts().next().unwrap().bounds;
        if let Some(first) = first_bounds {
            same_bounds(bounds, first);
        }
        first_bounds = Some(bounds);
        let saved = encode_snapshot(state.project_snapshot().unwrap()).unwrap();
        let bytes = save_archive(&saved, Cursor::new(Vec::new()))
            .unwrap()
            .into_inner();
        let opened =
            decode_project(Cursor::new(bytes), None, "snapped.docscan".into())
                .unwrap();
        let pages = crate::app::pages::Pages::from_opened(&opened).unwrap();
        let text = &state.annotations().texts().next().unwrap().clone();
        let mut loaded =
            pages.current().annotations.texts().next().unwrap().clone();
        same_bounds(loaded.bounds, text.bounds);
        loaded.bounds = text.bounds;
        assert_eq!(&loaded, text);
        let mark = &state.annotations().shapes().next().unwrap().clone();
        let mut loaded =
            pages.current().annotations.shapes().next().unwrap().clone();
        same_bounds(loaded.bounds(), mark.bounds());
        let fastfill_rust::annotations::AnnotationGeometry::Bounded {
            bounds: loaded_bounds,
            ..
        } = &mut loaded.geometry
        else {
            panic!("expected a bounded mark")
        };
        *loaded_bounds = mark.bounds();
        assert_eq!(&loaded, mark);
        let (Handle::Bytes(_, a), Handle::Bytes(_, b)) =
            (&opened.images[0].handle, &state.image().unwrap().handle)
        else {
            panic!("source bytes must survive saving")
        };
        assert_eq!(a, b);
        let _ = update(&mut state, Message::Undo);
        assert_eq!(
            state.annotations().texts().cloned().collect::<Vec<_>>(),
            []
        );
        assert_eq!(
            state
                .annotations()
                .shapes()
                .cloned()
                .collect::<Vec<_>>()
                .len(),
            1
        );
        let _ = update(&mut state, Message::Redo);
        assert_eq!(state.annotations().texts().next().unwrap().bounds, bounds);
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn native_worker_delivers_features_without_a_gui_runtime() {
    let input = image("worker.png", 0);
    let result = iced::futures::executor::block_on(worker::run(
        input.display_handle().clone(),
    ))
    .unwrap();
    assert_ne!(result.boxes(), []);
    assert_ne!(result.horizontal_lines(), []);
}

#[test]
fn real_forms_place_marks_inside_boxes_and_text_after_labels() {
    let cases = [
        (
            include_bytes!("../../../../tests/samples/sample-form.png")
                .as_slice(),
            Tool::Checkmark,
            113.0,
            654.0,
            95.0,
            130.0,
        ),
        (
            include_bytes!("../../../../tests/samples/sample-form.png")
                .as_slice(),
            Tool::Text,
            980.0,
            710.0,
            795.0,
            870.0,
        ),
        (
            include_bytes!("../../../../tests/samples/datum-form.png")
                .as_slice(),
            Tool::Text,
            280.0,
            1238.0,
            170.0,
            200.0,
        ),
    ];
    for (bytes, tool, x, y, left, right) in cases {
        let mut state = FastFill::default();
        let image = ImportedImage::from_bytes(
            "real-form.png".into(),
            bytes.to_vec(),
            None,
        )
        .unwrap();
        let page_units = 612.0 / image.size.width;
        state.pages.add_image(image);
        let _ = update(&mut state, Message::SetTool(tool));
        send(&mut state, Event::Toggle);
        settle(&mut state);
        let point = Point::new(x * page_units, y * page_units);
        let _ = update(
            &mut state,
            Message::DrawPressed(Pointer::Mouse, point, 1.0),
        );
        let _ = update(
            &mut state,
            Message::DrawReleased(Pointer::Mouse, point, false),
        );
        let bounds = if tool == Tool::Text {
            paste_text(&mut state, "Sample value");
            state.annotations().text_preview().unwrap().bounds
        } else {
            let bounds = state.annotations().shapes().next().unwrap().bounds();
            assert!(bounds.width < 30.0 * page_units);
            bounds
        };
        assert!(
            (left..right).contains(&(bounds.x / page_units)),
            "{tool:?}: {bounds:?}"
        );
    }
}

#[test]
fn text_in_a_table_uses_the_clicked_cells_left_border() {
    let mut pixels = RgbaImage::from_pixel(800, 600, Rgba([255; 4]));
    for y in 100..303 {
        for x in 100..703 {
            if [100, 200, 300].iter().any(|v| (*v..v + 3).contains(&y))
                || [100, 300, 500, 700]
                    .iter()
                    .any(|v| (*v..v + 3).contains(&x))
            {
                pixels.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
    }
    let mut bytes = Cursor::new(Vec::new());
    pixels.write_to(&mut bytes, ImageFormat::Png).unwrap();
    let mut state = FastFill::default();
    state.pages.add_image(
        ImportedImage::from_bytes("table.png".into(), bytes.into_inner(), None)
            .unwrap(),
    );
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    send(&mut state, Event::Toggle);
    settle(&mut state);
    click(&mut state, Pointer::Mouse, 450.0, 190.0, 1.0);
    let text = state.annotations().text_preview().unwrap();
    assert!((text.bounds.x - 300.0 * 0.765).abs() < 3.0);
    assert!((text.bounds.y + text.bounds.height - 200.0 * 0.765).abs() < 3.0);
}
