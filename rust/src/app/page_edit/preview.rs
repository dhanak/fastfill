//! One correction job at a time; native CPU work never runs in UI updates.

use super::{Event, Message};
use fastfill_rust::correction::{
    PageCorrection, correct_image, corrected_dimensions,
};
use fastfill_rust::document_detection::{DocumentDetection, detect_document};
use fastfill_rust::form_snap::CornerFeatures;
use fastfill_rust::image_input::decode_image;
use iced::Task;
use iced::widget::image::Handle;
use image::RgbaImage;
use std::sync::{Arc, OnceLock};

#[derive(Debug)]
pub(super) struct Original {
    pub(super) pixels: RgbaImage,
    pub(super) handle: Handle,
    pub(super) corners: Result<CornerFeatures, String>,
    pub(super) framing: OnceLock<Result<DocumentDetection, String>>,
}

#[derive(Clone, Debug)]
pub(super) enum Input {
    Image(Handle),
    Decoded(Arc<Original>),
    Frame(Arc<Original>),
}

#[derive(Debug)]
pub(crate) struct Preview {
    pub(super) settings: PageCorrection,
    pub(super) original: Arc<Original>,
    pub(super) handle: Handle,
    pub(super) notice: Option<String>,
}

pub(super) fn build(
    input: Input,
    mut settings: PageCorrection,
) -> Result<Arc<Preview>, String> {
    let framing = matches!(&input, Input::Frame(_));
    let original = match input {
        Input::Decoded(original) | Input::Frame(original) => original,
        Input::Image(handle) => {
            let pixels = match &handle {
                Handle::Bytes(_, bytes) => decode_image(bytes.as_ref())
                    .map_err(|error| error.to_string())?
                    .1
                    .into_rgba8(),
                Handle::Rgba {
                    width,
                    height,
                    pixels,
                    ..
                } => {
                    corrected_dimensions(*width, *height, settings)
                        .map_err(|error| error.to_string())?;
                    RgbaImage::from_raw(
                        *width,
                        *height,
                        pixels.as_ref().to_vec(),
                    )
                    .ok_or("Original image pixels are invalid.")?
                }
                Handle::Path(_, _) => {
                    return Err("Original image is not loaded.".into());
                }
            };
            let handle = match handle {
                Handle::Rgba { .. } => handle,
                _ => Handle::from_rgba(
                    pixels.width(),
                    pixels.height(),
                    pixels.as_raw().clone(),
                ),
            };
            let corners = CornerFeatures::analyze(&pixels)
                .map_err(|error| error.to_string());
            Arc::new(Original {
                pixels,
                handle,
                corners,
                framing: OnceLock::new(),
            })
        }
    };
    let notice = framing.then(|| reframe(&original, &mut settings));
    let handle = if settings == PageCorrection::default() {
        original.handle.clone()
    } else {
        let pixels = correct_image(&original.pixels, settings)
            .map_err(|error| error.to_string())?;
        Handle::from_rgba(pixels.width(), pixels.height(), pixels.into_raw())
    };
    Ok(Arc::new(Preview {
        settings,
        original,
        handle,
        notice,
    }))
}

fn reframe(original: &Original, settings: &mut PageCorrection) -> String {
    let detection = original.framing.get_or_init(|| {
        detect_document(&original.pixels).map_err(|error| error.to_string())
    });
    let detection = match detection {
        Ok(detection) => detection,
        Err(error) => {
            return format!(
                "Auto frame unavailable ({error}). \
                 Crop unchanged; adjust corners manually."
            );
        }
    };
    let crop = detection
        .corners
        .or(detection.candidate_corners.filter(|_| {
            // Retain the C# manual-review cutoff, not live acceptance.
            detection.confidence >= 0.45
        }));
    let Some(crop) = crop else {
        return "No likely document frame found. Crop unchanged; \
                adjust corners manually."
            .into();
    };
    let proposed = PageCorrection { crop, ..*settings };
    if corrected_dimensions(
        original.pixels.width(),
        original.pixels.height(),
        proposed,
    )
    .is_err()
    {
        return "Detected crop is unusable. Crop unchanged; \
                adjust corners manually."
            .into();
    }
    *settings = proposed;
    if detection.corners.is_some() {
        "Frame suggested. Check all four corners before accepting.".into()
    } else {
        "Low-confidence frame suggested. Check or adjust every corner.".into()
    }
}

pub(super) fn task(
    id: u64,
    input: Input,
    settings: PageCorrection,
) -> Task<Message> {
    Task::perform(
        async move {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let (sender, receiver) =
                    iced::futures::channel::oneshot::channel();
                std::thread::Builder::new()
                    .name("fastfill-correction".into())
                    .spawn(move || {
                        let result = std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| {
                                build(input, settings)
                            }),
                        )
                        .unwrap_or_else(|_| {
                            Err("Correction worker failed.".into())
                        });
                        let _ = sender.send(result);
                    })
                    .map_err(|error| {
                        format!("Could not start correction: {error}")
                    })?;
                receiver
                    .await
                    .map_err(|_| "Correction worker stopped.".to_string())?
            }
            #[cfg(target_arch = "wasm32")]
            {
                // No browser worker runtime yet, as with PDF import.
                build(input, settings)
            }
        },
        move |result| Message::PageEdit(Event::Ready(id, result)),
    )
}
