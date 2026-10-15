//! Color filters and tone adjustments; no document-format changes.

mod clahe;
mod color;

use super::CorrectionError;
use crate::project::{PageFilterMode, PageFilterSettings};
use image::{DynamicImage, GrayImage, Luma, Rgba, RgbaImage};

pub(super) fn validate(
    settings: PageFilterSettings,
    brightness: i32,
) -> Result<(), CorrectionError> {
    if (-100..=100).contains(&settings.contrast)
        && (-100..=100).contains(&brightness)
    {
        Ok(())
    } else {
        Err(CorrectionError::InvalidFilter)
    }
}

pub(super) fn apply(
    image: &mut RgbaImage,
    settings: PageFilterSettings,
    brightness: i32,
) {
    match settings.mode {
        PageFilterMode::Original => {}
        PageFilterMode::EnhancedColor => enhance(image),
        PageFilterMode::Grayscale => {
            for pixel in image.pixels_mut().filter(|pixel| pixel[3] != 0) {
                let gray = luminance(*pixel);
                pixel.0[..3].fill(gray);
            }
        }
        PageFilterMode::BlackAndWhite => black_and_white(image),
    }
    if settings.contrast != 0 || brightness != 0 {
        let contrast = i16::try_from(settings.contrast)
            .expect("contrast was validated before filtering");
        let brightness = i16::try_from(brightness)
            .expect("brightness was validated before filtering");
        let gain = 1.0 + f32::from(contrast) / 100.0;
        let offset = f32::from(brightness) * 255.0 / 100.0;
        let table: [u8; 256] = std::array::from_fn(|index| {
            let value = f32::from(u8::try_from(index).expect("byte index"));
            // Clamp once, so contrast clipping cannot erase tonal detail
            // that the brightness offset would bring back into range.
            byte((value - 128.0) * gain + 128.0 + offset)
        });
        for pixel in image.pixels_mut().filter(|pixel| pixel[3] != 0) {
            for channel in &mut pixel.0[..3] {
                *channel = table[usize::from(*channel)];
            }
        }
    }
}

fn luminance(pixel: Rgba<u8>) -> u8 {
    // BT.601 matches the existing scanner; imageops grayscale uses BT.709.
    let sum = 299 * u32::from(pixel[0])
        + 587 * u32::from(pixel[1])
        + 114 * u32::from(pixel[2]);
    u8::try_from((sum + 500) / 1_000).expect("luminance is within 0..255")
}

fn paper_luminance(pixel: Rgba<u8>) -> u8 {
    let alpha = u32::from(pixel[3]);
    u8::try_from(
        (u32::from(luminance(pixel)) * alpha + 255 * (255 - alpha) + 127) / 255,
    )
    .expect("composited byte")
}

fn black_and_white(image: &mut RgbaImage) {
    // Keep the old Gaussian window/sigma/offset. Omit blanket median blur:
    // it removes single-pixel form rules, dotted fields, and fine print.
    let gray = GrayImage::from_fn(image.width(), image.height(), |x, y| {
        Luma([paper_luminance(image[(x, y)])])
    });
    let mean = DynamicImage::ImageLuma8(gray)
        .blur_advanced(
            image::imageops::GaussianBlurParameters::new_from_kernel_size(31.0),
        )
        .into_luma8();
    for (pixel, mean) in image.pixels_mut().zip(mean.pixels()) {
        if pixel[3] != 0 {
            let gray = if i16::from(paper_luminance(*pixel))
                > i16::from(mean[0]) - 12
            {
                255
            } else {
                0
            };
            pixel.0[..3].fill(gray);
        }
    }
}

fn enhance(image: &mut RgbaImage) {
    let mut sums = [0.0; 3];
    let mut weight = 0.0;
    for pixel in image.pixels() {
        let alpha = f64::from(pixel[3]);
        weight += alpha;
        for (sum, channel) in sums.iter_mut().zip(pixel.0) {
            *sum += f64::from(channel) * alpha;
        }
    }
    if weight == 0.0 {
        return;
    }
    let means = sums.map(|sum| sum / weight);
    let target = means.iter().sum::<f64>() / 3.0;
    // The gain is finite and bounded by 255; f32 is sufficient for u8 RGB.
    #[allow(clippy::cast_possible_truncation)]
    let scales = means.map(|mean| (target / mean.max(1.0)) as f32);
    let linear = color::linear_table();
    for pixel in image.pixels_mut().filter(|pixel| pixel[3] != 0) {
        let balanced = std::array::from_fn(|index| {
            byte(f32::from(pixel[index]) * scales[index])
        });
        pixel.0[..3].copy_from_slice(&color::to_lab(balanced, &linear));
    }
    // Reuse the RGB slots as Lab while equalizing L; no float-image copy.
    clahe::equalize(image);
    for pixel in image.pixels_mut().filter(|pixel| pixel[3] != 0) {
        let rgb = color::to_rgb([pixel[0], pixel[1], pixel[2]]);
        pixel.0[..3].copy_from_slice(&rgb);
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}
