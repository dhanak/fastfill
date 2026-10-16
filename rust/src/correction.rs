//! Platform-independent image correction; no document mutation or file I/O.

mod filters;
mod perspective;
mod placement;

pub use placement::PageMapping;

use crate::image_input::MAX_IMAGE_PIXELS;
use crate::project::{CropQuad, PageFilterSettings, ProjectPage};
use image::RgbaImage;
use perspective::Projection;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct PageCorrection {
    pub crop: CropQuad,
    pub rotation_quarter_turns: i32,
    pub filter: PageFilterSettings,
    /// Additive RGB adjustment, -100..=100; zero leaves brightness unchanged.
    /// Schema-v2 persists this separately from the legacy filter DTO.
    pub brightness: i32,
}

impl From<&ProjectPage> for PageCorrection {
    fn from(page: &ProjectPage) -> Self {
        Self {
            crop: page.crop,
            rotation_quarter_turns: page.rotation_quarter_turns,
            filter: page.filter,
            brightness: page.brightness,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorrectionError {
    InvalidDimensions,
    InvalidCrop,
    OutputTooLarge,
    InvalidFilter,
}

impl fmt::Display for CorrectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidDimensions => "Source dimensions exceed image limits.",
            Self::InvalidCrop => {
                "Crop must be usable, finite, convex, and inside the image."
            }
            Self::OutputTooLarge => "Corrected image exceeds the 50 MP limit.",
            Self::InvalidFilter => {
                "Brightness and contrast must be between -100 and 100."
            }
        })
    }
}

impl std::error::Error for CorrectionError {}

/// Output dimensions, including both endpoint pixel centers.
///
/// # Errors
/// Rejects invalid dimensions, crops, filter settings, and oversized output.
pub fn corrected_dimensions(
    width: u32,
    height: u32,
    correction: PageCorrection,
) -> Result<(u32, u32), CorrectionError> {
    filters::validate(correction.filter, correction.brightness)?;
    let (width, height) = crop_dimensions(width, height, correction.crop)?;
    Ok(if correction.rotation_quarter_turns.rem_euclid(2) == 0 {
        (width, height)
    } else {
        (height, width)
    })
}

/// Correct perspective, rotate clockwise, then apply the page filter.
///
/// # Errors
/// Rejects the same invalid inputs as `corrected_dimensions`.
pub fn correct_image(
    source: &RgbaImage,
    correction: PageCorrection,
) -> Result<RgbaImage, CorrectionError> {
    filters::validate(correction.filter, correction.brightness)?;
    let (width, height) =
        crop_dimensions(source.width(), source.height(), correction.crop)?;
    let corrected = if correction.crop == CropQuad::default() {
        // Do not resample an identity crop, including transparent pixels.
        Cow::Borrowed(source)
    } else {
        Cow::Owned(
            Projection::new(correction.crop)?.warp(source, width, height),
        )
    };
    let mut corrected = match correction.rotation_quarter_turns.rem_euclid(4) {
        0 => corrected.into_owned(),
        1 => image::imageops::rotate90(corrected.as_ref()),
        2 => image::imageops::rotate180(corrected.as_ref()),
        _ => image::imageops::rotate270(corrected.as_ref()),
    };
    filters::apply(&mut corrected, correction.filter, correction.brightness);
    Ok(corrected)
}

fn crop_dimensions(
    width: u32,
    height: u32,
    crop: CropQuad,
) -> Result<(u32, u32), CorrectionError> {
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return Err(CorrectionError::InvalidDimensions);
    }
    Projection::new(crop)?;
    let points = crop.points().map(|point| {
        (
            f64::from(point.x) * f64::from(width - 1),
            f64::from(point.y) * f64::from(height - 1),
        )
    });
    let distance = |a: usize, b: usize| {
        (points[a].0 - points[b].0).hypot(points[a].1 - points[b].1)
    };
    // Inclusive pixel-center endpoints avoid the old W-1/H-1 full crop.
    let width = distance(0, 1).max(distance(3, 2)).round() + 1.0;
    let height = distance(0, 3).max(distance(1, 2)).round() + 1.0;
    // Source/crop bounds keep each edge below 51 million pixels, far below
    // u32::MAX. These positive values were rounded to whole pixels above.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = (width as u32, height as u32);
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(CorrectionError::OutputTooLarge);
    }
    Ok((width, height))
}
