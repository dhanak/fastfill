//! One correction job at a time; native CPU work never runs in UI updates.

use super::{Event, Message};
use fastfill_rust::correction::{
    PageCorrection, correct_image, corrected_dimensions,
};
use fastfill_rust::image_input::decode_image;
use iced::Task;
use iced::widget::image::Handle;
use image::RgbaImage;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct Original {
    pub(super) pixels: RgbaImage,
    pub(super) handle: Handle,
}

#[derive(Clone, Debug)]
pub(super) enum Input {
    Image(Handle),
    Decoded(Arc<Original>),
}

#[derive(Debug)]
pub(crate) struct Preview {
    pub(super) settings: PageCorrection,
    pub(super) original: Arc<Original>,
    pub(super) handle: Handle,
}

pub(super) fn build(
    input: Input,
    settings: PageCorrection,
) -> Result<Arc<Preview>, String> {
    let original = match input {
        Input::Decoded(original) => original,
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
            Arc::new(Original { pixels, handle })
        }
    };
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
    }))
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
