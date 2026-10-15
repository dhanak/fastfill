use image::{ImageFormat, ImageReader, Limits};
use std::fmt;
use std::io::Cursor;

pub const MAX_IMAGE_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 50_000_000;
const MAX_IMAGE_DIMENSION: u32 = 50_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormatKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageFormatKind {
    Jpeg,
    Png,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageInputError {
    Empty,
    TooLarge,
    TooManyPixels,
    UnsupportedFormat,
    InvalidData,
}

impl fmt::Display for ImageInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "The image is empty.",
            Self::TooLarge => "The image file is larger than 100 MiB.",
            Self::TooManyPixels => "The image contains more than 50 MP.",
            Self::UnsupportedFormat => {
                "Only PNG and JPEG images are supported."
            }
            Self::InvalidData => "The image data is incomplete or invalid.",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ImageInputError {}

/// Checks the encoded file size before allocating or decoding image data.
///
/// # Errors
///
/// Returns an error for empty data or data larger than 100 MiB.
pub fn validate_image_length(length: usize) -> Result<(), ImageInputError> {
    match length {
        0 => Err(ImageInputError::Empty),
        length if length > MAX_IMAGE_BYTES => Err(ImageInputError::TooLarge),
        _ => Ok(()),
    }
}

/// Validates and fully decodes a PNG or JPEG within the import limits.
///
/// # Errors
///
/// Returns an error for empty, oversized, unsupported, or corrupt images.
pub fn validate_image(bytes: &[u8]) -> Result<ImageInfo, ImageInputError> {
    decode_image(bytes).map(|(info, _)| info)
}

/// Validate an image and retain its pixels for display without decoding twice.
///
/// # Errors
/// Rejects the same invalid inputs and allocation limits as `validate_image`.
pub fn decode_image(
    bytes: &[u8],
) -> Result<(ImageInfo, image::DynamicImage), ImageInputError> {
    let info = inspect_image(bytes)?;
    let format = match info.format {
        ImageFormatKind::Jpeg => ImageFormat::Jpeg,
        ImageFormatKind::Png => ImageFormat::Png,
    };
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(decode_limits());
    let pixels = reader.decode().map_err(|_| ImageInputError::InvalidData)?;
    Ok((info, pixels))
}

/// Inspect bounded dimensions/format without claiming the pixels are valid.
///
/// # Errors
/// Rejects invalid headers, unsupported formats, and excessive sizes.
pub fn inspect_image(bytes: &[u8]) -> Result<ImageInfo, ImageInputError> {
    validate_image_length(bytes.len())?;

    let guessed = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageInputError::InvalidData)?;
    let format = guessed.format().ok_or(ImageInputError::UnsupportedFormat)?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(ImageInputError::UnsupportedFormat);
    }

    let mut dimensions_reader =
        ImageReader::with_format(Cursor::new(bytes), format);
    dimensions_reader.limits(decode_limits());
    let (width, height) = dimensions_reader
        .into_dimensions()
        .map_err(|_| ImageInputError::InvalidData)?;
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(ImageInputError::TooManyPixels);
    }

    Ok(ImageInfo {
        width,
        height,
        format: match format {
            ImageFormat::Jpeg => ImageFormatKind::Jpeg,
            ImageFormat::Png => ImageFormatKind::Png,
            _ => unreachable!("unsupported formats were rejected"),
        },
    })
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_PIXELS * 4 + 64 * 1024 * 1024);
    limits
}
