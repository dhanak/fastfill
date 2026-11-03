use fastfill_rust::geometry::Size as PageSize;
use fastfill_rust::image_input::ImageInfo;
use fastfill_rust::image_input::MAX_IMAGE_BYTES;
use fastfill_rust::image_input::decode_import_image;
use iced::widget::image::Handle;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct ImportedImage {
    pub(crate) handle: Handle,
    // Display decoded pixels; keep the matching encoded asset for saves.
    pub(crate) display: Option<Handle>,
    pub(crate) name: String,
    // Display pixel dimensions; info retains the original source dimensions.
    pub(crate) size: PageSize,
    pub(crate) info: ImageInfo,
    pub(crate) directory: Option<PathBuf>,
}

pub(crate) async fn pick_image(
    directory: Option<PathBuf>,
    pixel_budget: u64,
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
    #[cfg(not(target_arch = "wasm32"))]
    let bytes = {
        use std::io::Read;
        let mut reader = std::fs::File::open(file.path())
            .map_err(|error| format!("Could not read image: {error}"))?
            .take(MAX_IMAGE_BYTES as u64 + 1);
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read image: {error}"))?;
        bytes
    };
    #[cfg(target_arch = "wasm32")]
    let bytes = file.read().await;
    let info = fastfill_rust::image_input::inspect_image(&bytes)
        .map_err(|error| error.to_string())?;
    if u64::from(info.width) * u64::from(info.height) > pixel_budget {
        return Err("Project images exceed the 200 MP memory limit.".into());
    }
    ImportedImage::from_bytes(name, bytes, directory).map(Some)
}

impl ImportedImage {
    pub(crate) fn from_bytes(
        name: String,
        bytes: Vec<u8>,
        directory: Option<PathBuf>,
    ) -> Result<Self, String> {
        let (info, pixels, bytes) =
            decode_import_image(bytes).map_err(|error| error.to_string())?;
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
            size: page_size_from_pixels(info.width, info.height),
            info,
            directory,
        }
    }

    pub(crate) fn set_pixels(&mut self, pixels: image::RgbaImage) {
        self.size = page_size_from_pixels(pixels.width(), pixels.height());
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
fn page_size_from_pixels(width: u32, height: u32) -> PageSize {
    // iced uses f32 coordinates; integer precision above 16 MP is immaterial.
    PageSize::new(width as f32, height as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastfill_rust::image_input::{ImageFormatKind, decode_image};

    #[test]
    fn standalone_exif_import_keeps_display_and_saved_asset_aligned() {
        let pixels = image::RgbImage::from_pixel(3, 2, image::Rgb([120; 3]));
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode_image(&pixels)
            .unwrap();
        bytes.splice(
            2..2,
            *b"\xff\xe1\0\x22Exif\0\0II\x2a\0\x08\0\0\0\x01\0\
               \x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0",
        );
        let (raw_info, _) = decode_image(&bytes).unwrap();
        let legacy = ImportedImage::from_validated(
            "legacy.jpg".into(),
            bytes.clone(),
            raw_info,
            None,
        );
        assert_eq!(legacy.info, raw_info);
        assert_eq!(legacy.size, PageSize::new(3.0, 2.0));

        let imported =
            ImportedImage::from_bytes("photo.jpg".into(), bytes, None).unwrap();
        assert_eq!(imported.info.format, ImageFormatKind::Png);
        assert_eq!((imported.info.width, imported.info.height), (2, 3));
        assert_eq!(imported.size, PageSize::new(2.0, 3.0));
        let Handle::Bytes(_, bytes) = &imported.handle else {
            panic!("encoded asset must remain available for saving");
        };
        let (saved_info, saved_pixels) = decode_image(bytes).unwrap();
        assert_eq!(saved_info, imported.info);
        let Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = imported.display_handle()
        else {
            panic!("display must use decoded pixels");
        };
        assert_eq!((*width, *height), (2, 3));
        assert_eq!(pixels.as_ref(), saved_pixels.to_rgba8().as_raw());
    }
}
