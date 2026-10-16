//! Bounded native CPU work; the browser harness has no worker runtime yet.

use super::Features;
use fastfill_rust::form_snap::FormFeatures;
use fastfill_rust::image_input::{MAX_IMAGE_PIXELS, decode_image};
use iced::widget::image::Handle;
use image::RgbaImage;
use std::sync::Arc;

pub(super) fn analyze(input: &Handle) -> Features {
    let pixels = match input {
        Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } => {
            let count = u64::from(*width) * u64::from(*height);
            if count == 0
                || count > MAX_IMAGE_PIXELS
                || usize::try_from(count * 4).ok() != Some(pixels.len())
            {
                return Err("Displayed image pixels are invalid.".into());
            }
            RgbaImage::from_raw(*width, *height, pixels.as_ref().to_vec())
                .ok_or("Displayed image pixels are invalid.")?
        }
        Handle::Bytes(_, bytes) => decode_image(bytes.as_ref())
            .map_err(|error| error.to_string())?
            .1
            .into_rgba8(),
        Handle::Path(_, _) => return Err("Image is not loaded.".into()),
    };
    FormFeatures::analyze(&pixels)
        .map(Arc::new)
        .map_err(|error| error.to_string())
}

#[cfg_attr(target_arch = "wasm32", allow(clippy::unused_async))]
pub(super) async fn run(input: Handle) -> Features {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (sender, receiver) = iced::futures::channel::oneshot::channel();
        std::thread::Builder::new()
            .name("fastfill-form-snap".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(
                    std::panic::AssertUnwindSafe(|| analyze(&input)),
                )
                .unwrap_or_else(|_| Err("Form analysis worker failed.".into()));
                let _ = sender.send(result);
            })
            .map_err(|error| {
                format!("Could not start form analysis: {error}")
            })?;
        receiver
            .await
            .map_err(|_| "Form analysis worker stopped.".to_string())?
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Same intentional UI-thread fallback as correction/PDF import.
        analyze(&input)
    }
}
