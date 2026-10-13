use fastfill_rust::geometry::Size as PageSize;
use fastfill_rust::image_input::ImageInfo;
#[cfg(not(target_arch = "wasm32"))]
use fastfill_rust::image_input::MAX_IMAGE_BYTES;
use fastfill_rust::image_input::{validate_image, validate_image_length};
use iced::widget::image::Handle;

#[derive(Clone, Debug)]
pub(crate) struct ImportedImage {
    pub(crate) handle: Handle,
    pub(crate) name: String,
    pub(crate) size: PageSize,
}

pub(crate) async fn pick_image() -> Result<Option<ImportedImage>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Open image")
        .add_filter("Image", &["png", "jpg", "jpeg"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let length = std::fs::metadata(file.path())
            .map_err(|error| format!("Could not inspect image: {error}"))?
            .len();
        if length > MAX_IMAGE_BYTES as u64 {
            return Err("The image file is larger than 100 MiB.".into());
        }
    }

    let name = file.file_name();
    let bytes = file.read().await;
    validate_image_length(bytes.len()).map_err(|error| error.to_string())?;
    let info = validate_image(&bytes).map_err(|error| error.to_string())?;

    Ok(Some(ImportedImage {
        handle: Handle::from_bytes(bytes),
        name,
        size: page_size_from_pixels(info),
    }))
}

#[allow(clippy::cast_precision_loss)]
fn page_size_from_pixels(info: ImageInfo) -> PageSize {
    // iced uses f32 coordinates; integer precision above 16 MP is immaterial.
    PageSize::new(info.width as f32, info.height as f32)
}
