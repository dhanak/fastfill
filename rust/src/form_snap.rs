//! Form rules, checkboxes, and corner hints in normalized image space.

mod components;
mod corners;
mod lines;

pub use corners::CornerFeatures;

use crate::image_input::{ImageInputError, MAX_IMAGE_PIXELS};
use crate::project::{NormalizedPoint, NormalizedRect};
use components::PixelRect;
use image::{GrayImage, Luma, RgbaImage, imageops};
use serde::Serialize;
use std::borrow::Cow;

const ANALYSIS_EDGE: u32 = 1600;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SnapLine {
    pub start: NormalizedPoint,
    pub end: NormalizedPoint,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFeatures {
    width: u32,
    height: u32,
    boxes: Vec<NormalizedRect>,
    horizontal_lines: Vec<SnapLine>,
}

impl FormFeatures {
    /// Analyze corrected page pixels, retaining the C# lab's size calibration.
    /// Transparent pixels are composited on white, never treated as black ink.
    ///
    /// # Errors
    /// Rejects empty images or source dimensions exceeding the import limit.
    pub fn analyze(source: &RgbaImage) -> Result<Self, ImageInputError> {
        let (width, height) = source.dimensions();
        if width == 0 || height == 0 {
            return Err(ImageInputError::Empty);
        }
        if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
            return Err(ImageInputError::TooManyPixels);
        }
        let gray = paper_gray(source);
        let source = analysis_image(&gray);
        let (width, height) = source.dimensions();
        let mut features = Self {
            width,
            height,
            boxes: Vec::new(),
            horizontal_lines: Vec::new(),
        };
        if width < 16 || height < 16 {
            return Ok(features);
        }
        let binary = ink_mask(&source);
        let width = usize::try_from(width)
            .map_err(|_| ImageInputError::TooManyPixels)?;
        let height = usize::try_from(height)
            .map_err(|_| ImageInputError::TooManyPixels)?;
        let parts = components::bounds(&binary, width, height);
        features.boxes = boxes(&binary, width, height, &parts)
            .into_iter()
            .map(|bounds| normalize_box(bounds, width, height))
            .collect();
        features.horizontal_lines =
            lines::detect(&binary, width, height, &parts)
                .into_iter()
                .map(|bounds| SnapLine {
                    start: normalize_point(bounds.x, bounds.y, width, height),
                    end: normalize_point(
                        bounds.right() - 1,
                        bounds.y,
                        width,
                        height,
                    ),
                })
                .collect();
        Ok(features)
    }

    #[must_use]
    pub const fn analysis_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub fn boxes(&self) -> &[NormalizedRect] {
        &self.boxes
    }

    #[must_use]
    pub fn horizontal_lines(&self) -> &[SnapLine] {
        &self.horizontal_lines
    }

    /// Radius is a fraction of the image's shorter side, never screen pixels.
    #[must_use]
    pub fn find_box(
        &self,
        point: NormalizedPoint,
        radius: f32,
    ) -> Option<NormalizedRect> {
        if !valid_query(point, radius) {
            return None;
        }
        self.boxes
            .iter()
            .copied()
            .filter_map(|bounds| {
                let nearest = NormalizedPoint {
                    x: point.x.clamp(bounds.x, bounds.right()),
                    y: point.y.clamp(bounds.y, bounds.bottom()),
                };
                let distance = self.distance(point, nearest);
                let center = NormalizedPoint {
                    x: bounds.x + bounds.width / 2.0,
                    y: bounds.y + bounds.height / 2.0,
                };
                (distance <= radius).then_some((
                    bounds,
                    distance,
                    self.distance(point, center),
                ))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
            .map(|(bounds, ..)| bounds)
    }

    /// Radius uses the same short-axis units as `find_box`.
    /// Prefer a table cell over its full row rule at the same distance.
    #[must_use]
    pub fn find_horizontal_line(
        &self,
        point: NormalizedPoint,
        radius: f32,
    ) -> Option<SnapLine> {
        if !valid_query(point, radius) {
            return None;
        }
        self.horizontal_lines
            .iter()
            .copied()
            .filter_map(|line| {
                let nearest = NormalizedPoint {
                    x: point.x.clamp(line.start.x, line.end.x),
                    y: line.start.y,
                };
                let distance = self.distance(point, nearest);
                (distance <= radius).then_some((line, distance))
            })
            .min_by(|a, b| {
                a.1.total_cmp(&b.1).then_with(|| {
                    (a.0.end.x - a.0.start.x)
                        .total_cmp(&(b.0.end.x - b.0.start.x))
                })
            })
            .map(|(line, _)| line)
    }

    #[allow(clippy::cast_precision_loss)]
    fn distance(&self, a: NormalizedPoint, b: NormalizedPoint) -> f32 {
        // Analysis dimensions are at most 1600, exactly representable in f32.
        ((a.x - b.x) * self.width as f32)
            .hypot((a.y - b.y) * self.height as f32)
            / self.width.min(self.height) as f32
    }
}

fn valid_query(point: NormalizedPoint, radius: f32) -> bool {
    (0.0..=1.0).contains(&point.x)
        && (0.0..=1.0).contains(&point.y)
        && radius.is_finite()
        && radius >= 0.0
}

fn analysis_image(source: &GrayImage) -> Cow<'_, GrayImage> {
    let (width, height) = source.dimensions();
    let longest = width.max(height);
    if longest <= ANALYSIS_EDGE {
        return Cow::Borrowed(source);
    }
    let axis = |length| {
        u32::try_from(
            (u64::from(length) * u64::from(ANALYSIS_EDGE)
                + u64::from(longest / 2))
                / u64::from(longest),
        )
        .expect("analysis edge fits u32")
        .max(1)
    };
    Cow::Owned(imageops::resize(
        source,
        axis(width),
        axis(height),
        imageops::FilterType::Triangle,
    ))
}

fn paper_gray(source: &RgbaImage) -> GrayImage {
    // Composite before resizing: hidden RGB must not make dark ink fringes.
    GrayImage::from_fn(source.width(), source.height(), |x, y| {
        let pixel = source[(x, y)];
        let gray = (299 * u32::from(pixel[0])
            + 587 * u32::from(pixel[1])
            + 114 * u32::from(pixel[2])
            + 500)
            / 1000;
        let alpha = u32::from(pixel[3]);
        Luma([
            u8::try_from((gray * alpha + 255 * (255 - alpha) + 127) / 255)
                .expect("composited luminance is a byte"),
        ])
    })
}

fn ink_mask(gray: &GrayImage) -> Vec<u8> {
    let mean = imageops::blur_advanced(
        gray,
        imageops::GaussianBlurParameters::new_from_kernel_size(31.0),
    );
    gray.as_raw()
        .iter()
        .zip(mean.as_raw())
        .map(|(ink, mean)| u8::from(i16::from(*ink) <= i16::from(*mean) - 10))
        .collect()
}

#[allow(clippy::cast_precision_loss)]
fn normalize_box(
    bounds: PixelRect,
    width: usize,
    height: usize,
) -> NormalizedRect {
    NormalizedRect {
        x: bounds.x as f32 / width as f32,
        y: bounds.y as f32 / height as f32,
        width: bounds.width as f32 / width as f32,
        height: bounds.height as f32 / height as f32,
    }
}

#[allow(clippy::cast_precision_loss)]
fn normalize_point(x: usize, y: usize, w: usize, h: usize) -> NormalizedPoint {
    NormalizedPoint {
        x: x.min(w - 1) as f32 / (w - 1) as f32,
        y: y.min(h - 1) as f32 / (h - 1) as f32,
    }
}

#[allow(clippy::cast_precision_loss)]
fn boxes(
    binary: &[u8],
    width: usize,
    height: usize,
    parts: &[PixelRect],
) -> Vec<PixelRect> {
    let minimum = 8.0_f32.max(width.min(height) as f32 * 0.006);
    let maximum = width.min(height) as f32 * 0.12;
    // Include rectangular white holes so boxes attached to borders survive.
    // Test the white interior too: padding any letter counter fakes a frame.
    let inverse = binary.iter().map(|value| 1 - value).collect::<Vec<_>>();
    let holes = components::bounds(&inverse, width, height);
    let mut candidates = parts
        .iter()
        .copied()
        .chain(
            holes
                .into_iter()
                .filter(|b| {
                    b.x > 0
                        && b.y > 0
                        && b.right() < width
                        && b.bottom() < height
                        && rectangular_edges(&inverse, width, *b)
                })
                .map(|b| PixelRect {
                    x: b.x - 1,
                    y: b.y - 1,
                    width: b.width + 2,
                    height: b.height + 2,
                }),
        )
        .filter(|b| {
            b.width as f32 >= minimum
                && b.height as f32 >= minimum
                && b.width as f32 <= maximum
                && b.height as f32 <= maximum
                && (0.65..=1.55).contains(&(b.width as f32 / b.height as f32))
                && rectangular_edges(binary, width, *b)
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|b| std::cmp::Reverse(b.width * b.height));
    let mut distinct: Vec<PixelRect> = Vec::new();
    for candidate in candidates {
        if !distinct.iter().any(|existing| existing.same_box(candidate)) {
            distinct.push(candidate);
        }
    }
    distinct
}

fn rectangular_edges(binary: &[u8], width: usize, bounds: PixelRect) -> bool {
    // Intentional axis-aligned simplification after page correction. Checking
    // all four edge bands rejects rounded letters without a contour library.
    let band = 1.max(bounds.width.min(bounds.height) / 12);
    let horizontal = |top| {
        (bounds.x..bounds.right())
            .filter(|x| (top..top + band).any(|y| binary[y * width + x] != 0))
            .count()
    };
    let vertical = |left| {
        (bounds.y..bounds.bottom())
            .filter(|y| (left..left + band).any(|x| binary[y * width + x] != 0))
            .count()
    };
    horizontal(bounds.y) * 5 >= bounds.width * 4
        && horizontal(bounds.bottom() - band) * 5 >= bounds.width * 4
        && vertical(bounds.x) * 5 >= bounds.height * 4
        && vertical(bounds.right() - band) * 5 >= bounds.height * 4
}
