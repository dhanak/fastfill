use fastfill_rust::geometry::Size as PageSize;
use fastfill_rust::image_input::ImageInfo;
use fastfill_rust::image_input::MAX_IMAGE_BYTES;
use fastfill_rust::image_input::{decode_image, validate_image_length};
use iced::widget::image::Handle;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct ImportedImage {
    pub(crate) handle: Handle,
    // Display decoded pixels, but keep the original encoded handle for saves.
    pub(crate) display: Option<Handle>,
    pub(crate) name: String,
    pub(crate) size: PageSize,
    pub(crate) info: ImageInfo,
    pub(crate) directory: Option<PathBuf>,
}

pub(crate) async fn pick_image(
    directory: Option<PathBuf>,
) -> Result<Option<ImportedImage>, String> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Open image")
        .add_filter("Image", &["png", "jpg", "jpeg"]);
    if let Some(directory) = directory.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(directory);
    }
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };

    #[cfg(not(target_arch = "wasm32"))]
    let directory = file.path().parent().map(std::path::Path::to_path_buf);
    // Browsers do not expose the selected file's directory.
    #[cfg(target_arch = "wasm32")]
    let directory = None;

    #[cfg(not(target_arch = "wasm32"))]
    {
        let length = std::fs::metadata(file.path())
            .map_err(|error| format!("Could not inspect image: {error}"))?
            .len();
        if length > MAX_IMAGE_BYTES as u64 {
            return Err("The image file is larger than 100 MiB.".into());
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let limit = u32::try_from(MAX_IMAGE_BYTES).expect("100 MiB fits u32");
        if file.inner().size() > f64::from(limit) {
            return Err("The image file is larger than 100 MiB.".into());
        }
    }

    let name = file.file_name();
    let bytes = file.read().await;
    ImportedImage::from_bytes(name, bytes, directory).map(Some)
}

impl ImportedImage {
    pub(crate) fn from_bytes(
        name: String,
        bytes: Vec<u8>,
        directory: Option<PathBuf>,
    ) -> Result<Self, String> {
        validate_image_length(bytes.len())
            .map_err(|error| error.to_string())?;
        let (info, pixels) =
            decode_image(&bytes).map_err(|error| error.to_string())?;
        let mut image = Self::from_validated(name, bytes, info, directory);
        image.set_pixels(pixels.into_rgba8());
        Ok(image)
    }

    pub(crate) fn from_validated(
        name: String,
        bytes: Vec<u8>,
        info: ImageInfo,
        directory: Option<PathBuf>,
    ) -> Self {
        Self {
            handle: Handle::from_bytes(bytes),
            display: None,
            name,
            size: page_size_from_pixels(info),
            info,
            directory,
        }
    }

    pub(crate) fn set_pixels(&mut self, pixels: image::RgbaImage) {
        self.display = Some(Handle::from_rgba(
            pixels.width(),
            pixels.height(),
            pixels.into_raw(),
        ));
    }

    pub(crate) fn display_handle(&self) -> &Handle {
        self.display.as_ref().unwrap_or(&self.handle)
    }
}

#[allow(clippy::cast_precision_loss)]
fn page_size_from_pixels(info: ImageInfo) -> PageSize {
    // iced uses f32 coordinates; integer precision above 16 MP is immaterial.
    PageSize::new(info.width as f32, info.height as f32)
}
