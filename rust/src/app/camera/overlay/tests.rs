use super::*;
use crate::app::tests::render_element;
use ::image::{Rgba, RgbaImage};
use std::sync::Arc;
use std::time::Instant;

#[test]
fn translucent_outline_stays_on_the_letterboxed_image_at_both_dpi_scales() {
    let mut crop = CropQuad::default();
    crop.top_left.x = 0.25;
    crop.top_left.y = 0.25;
    crop.top_right.x = 0.75;
    crop.top_right.y = 0.25;
    crop.bottom_left.x = 0.25;
    crop.bottom_left.y = 0.75;
    crop.bottom_right.x = 0.75;
    crop.bottom_right.y = 0.75;
    for (width, height) in [(400, 200), (200, 400)] {
        let pixels = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
        let frame = crate::app::camera::tests::display_frame(
            Arc::new(pixels),
            Instant::now(),
        );
        for scale in [1_u32, 2] {
            #[allow(clippy::cast_precision_loss)]
            let dpi = scale as f32;
            let rendered = render_element(
                preview(&frame, Some(crop), false),
                iced::Size::new(400, 400),
                scale,
                dpi,
            );
            let pixel = |x: usize, y: usize| {
                let scale = usize::try_from(scale).unwrap();
                let offset = ((y * scale) * (400 * scale) + x * scale) * 4;
                &rendered[offset..offset + 4]
            };
            let sample = |x, y| {
                if width > height {
                    pixel(x, y)
                } else {
                    pixel(y, x)
                }
            };
            assert_eq!(sample(200, 50), [255; 4]);
            assert_eq!(sample(200, 120), [0, 0, 0, 255]);
            let inside = pixel(200, 200);
            assert!(inside[1] > 20 && inside[1] < 80, "{inside:?}");
            assert!(sample(100, 200)[1] > 120);
        }
    }
}

#[test]
fn circular_countdown_draws_more_accent_as_it_fills() {
    let count = |progress| {
        render_element(countdown(progress), iced::Size::new(96, 96), 1, 1.0)
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[2] > 150 && p[0] < 70 && p[1] > 60)
            .count()
    };
    assert_eq!(count(0.0), 0);
    let quarter = count(0.25);
    assert!(quarter > 100);
    assert!(count(1.0) > quarter * 2);
}

#[test]
fn mirrored_preview_and_outline_flip_together_without_mirroring_photo() {
    use crate::app::camera::{
        Event,
        tests::{present, ready},
        update,
    };
    let mut state = ready();
    let draft = state.camera.draft.as_mut().unwrap();
    draft.orientation.mirrored = true;
    let id = draft.id;
    // Asymmetric source and crop make a missing/double flip observable.
    let pixels = Arc::new(RgbaImage::from_fn(400, 200, |x, _| {
        if x < 200 {
            Rgba([220, 0, 0, 255])
        } else {
            Rgba([0, 0, 220, 255])
        }
    }));
    present(&mut state, id, pixels.clone());
    let mut crop = CropQuad::default();
    crop.top_left.x = 0.1;
    crop.bottom_left.x = 0.1;
    crop.top_right.x = 0.4;
    crop.bottom_right.x = 0.4;
    crop.top_left.y = 0.2;
    crop.top_right.y = 0.2;
    crop.bottom_left.y = 0.8;
    crop.bottom_right.y = 0.8;
    let display = |frame: &Frame, mirrored| {
        render_element(
            preview(frame, Some(crop), mirrored),
            iced::Size::new(400, 400),
            1,
            1.0,
        )
    };
    let live = display(
        state.camera.draft.as_ref().unwrap().last.as_ref().unwrap(),
        true,
    );
    let original = crate::app::camera::tests::display_frame(
        pixels.clone(),
        Instant::now(),
    );
    let unmirrored = display(&original, false);
    for y in [50, 120, 180, 200, 250, 350] {
        for x in [20, 80, 160, 240, 320, 380] {
            let at = (y * 400 + x) * 4;
            let flipped = (y * 400 + 399 - x) * 4;
            assert_eq!(&unmirrored[at..at + 4], &live[flipped..flipped + 4]);
        }
    }
    let at = (200 * 400 + 80) * 4;
    assert!(unmirrored[at + 1] > 20); // Crop belongs to the red/left field.
    assert_eq!(&live[at..at + 4], [0, 0, 220, 255]);
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.captured);
    assert!(draft.orientation.mirrored); // Retake keeps the preview aid.
    assert_eq!(display(draft.last.as_ref().unwrap(), true), live);
    assert!(Arc::ptr_eq(&draft.last.as_ref().unwrap().pixels, &pixels));
    let photo = super::super::worker::prepare(&pixels).unwrap();
    let iced::widget::image::Handle::Bytes(_, bytes) = photo.handle else {
        panic!("Expected PNG bytes");
    };
    assert_eq!(
        ::image::load_from_memory(&bytes).unwrap().to_rgba8(),
        *pixels
    );
}
