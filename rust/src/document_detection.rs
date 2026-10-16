//! Bounded still-image framing. No GUI state, camera, or temporal decisions.

mod lines;
mod pixels;
mod polygons;

use crate::image_input::{ImageInputError, MAX_IMAGE_PIXELS};
use crate::project::{CropQuad, NormalizedPoint};
use image::{GrayImage, RgbImage, RgbaImage};
use polygons::{Point, area, candidates, ordered};
use serde::Serialize;

const ACCEPTANCE: f64 = 0.72;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDetection {
    pub corners: Option<CropQuad>,
    pub candidate_corners: Option<CropQuad>,
    pub confidence: f64,
    pub area_ratio: f64,
    pub sharpness: f64,
    pub analysis_width: u32,
    pub analysis_height: u32,
}

/// Find a document using related outlines, not four independent corner picks.
///
/// Uses edge/paper outlines and line-pair recovery with boundary scoring.
/// Pixel/segment enumeration may differ from native `OpenCV`.
///
/// # Errors
/// Rejects empty images and dimensions beyond the existing 50 MP limit.
pub fn detect_document(
    source: &RgbaImage,
) -> Result<DocumentDetection, ImageInputError> {
    let (width, height) = analysis_size(source.width(), source.height())?;
    let mut result = DocumentDetection {
        corners: None,
        candidate_corners: None,
        confidence: 0.0,
        area_ratio: 0.0,
        sharpness: 0.0,
        analysis_width: width,
        analysis_height: height,
    };
    // Tiny/sliver images cannot provide a useful four-sided document.
    if width < 16 || height < 16 {
        return Ok(result);
    }
    let (gray, mut paper, lab) = pixels::prepare(source, width, height);
    result.sharpness = pixels::sharpness(&gray);
    for dilate in [true, true, false, false] {
        paper = pixels::morph(&paper, 4, true, dilate);
    }
    paper = pixels::morph(&paper, 12, true, false);
    paper = pixels::morph(&paper, 12, true, true);
    let blurred = pixels::gaussian5(&gray);
    let mut outlines = candidates(&paper, false);
    let mut distance = Vec::new();
    for (low, high) in [(60, 180), (35, 120)] {
        let edges = pixels::canny(&blurred, low, high);
        // Two 5x5 rectangular close iterations equal one 9x9 close.
        let closed = pixels::morph(&edges, 4, false, true);
        let closed = pixels::morph(&closed, 4, false, false);
        outlines.extend(candidates(&closed, true));
        let (length, votes, gap) = if low == 60 {
            (0.18, 70, 60.0)
        } else {
            (0.12, 50, 80.0)
        };
        outlines.extend(lines::candidates(&closed, length, votes, gap));
        if low == 35 {
            distance = pixels::edge_distances(&closed);
        }
    }
    for polygon in outlines {
        let Some((confidence, ratio)) = score(polygon, &paper, &lab, &distance)
        else {
            continue;
        };
        if confidence <= result.confidence {
            continue;
        }
        let points = ordered(polygon).map(|[x, y]| {
            #[allow(clippy::cast_possible_truncation)]
            NormalizedPoint {
                x: (x / f64::from(width - 1)).clamp(0.0, 1.0) as f32,
                y: (y / f64::from(height - 1)).clamp(0.0, 1.0) as f32,
            }
        });
        let quad = CropQuad {
            top_left: points[0],
            top_right: points[1],
            bottom_right: points[2],
            bottom_left: points[3],
        };
        if !quad.is_valid() {
            continue;
        }
        result.confidence = confidence.clamp(0.0, 1.0);
        result.area_ratio = ratio;
        result.corners = (confidence >= ACCEPTANCE).then_some(quad);
        result.candidate_corners = (confidence < ACCEPTANCE).then_some(quad);
    }
    Ok(result)
}

fn analysis_size(
    width: u32,
    height: u32,
) -> Result<(u32, u32), ImageInputError> {
    if width == 0 || height == 0 {
        return Err(ImageInputError::Empty);
    }
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(ImageInputError::TooManyPixels);
    }
    let longest = width.max(height);
    if longest <= 1280 {
        return Ok((width, height));
    }
    let scaled = |axis| {
        u32::try_from(
            (u64::from(axis) * 1280 + u64::from(longest / 2))
                / u64::from(longest),
        )
        .unwrap_or(1280)
        .max(1)
    };
    Ok((scaled(width), scaled(height)))
}

fn score(
    polygon: [Point; 4],
    paper: &GrayImage,
    lab: &RgbImage,
    distance: &[f32],
) -> Option<(f64, f64)> {
    let area = area(&polygon);
    let ratio = area / (f64::from(paper.width()) * f64::from(paper.height()));
    if ratio < 0.05 || !polygons::convex(polygon) {
        return None;
    }
    let mut rectangle = (f64::INFINITY, 0.0, 0.0);
    for i in 0..4 {
        let a = polygon[i];
        let b = polygon[(i + 1) % 4];
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        if length < 1.0 {
            return None;
        }
        let (dx, dy) = ((b[0] - a[0]) / length, (b[1] - a[1]) / length);
        let mut extent = [f64::INFINITY, f64::NEG_INFINITY];
        let mut other = extent;
        for [x, y] in polygon {
            let (along, across) = (x * dx + y * dy, -x * dy + y * dx);
            extent = [extent[0].min(along), extent[1].max(along)];
            other = [other[0].min(across), other[1].max(across)];
        }
        let (width, height) = (extent[1] - extent[0], other[1] - other[0]);
        if width * height < rectangle.0 {
            rectangle = (width * height, width.min(height), width.max(height));
        }
    }
    if rectangle.1 <= 0.0 || rectangle.2 / rectangle.1 > 5.0 {
        return None;
    }
    let border = f64::from(paper.width().min(paper.height())) * 0.02;
    let border_count = polygon
        .iter()
        .filter(|[x, y]| {
            *x <= border
                || *y <= border
                || *x >= f64::from(paper.width()) - border
                || *y >= f64::from(paper.height()) - border
        })
        .count();
    let geometry =
        (ratio / 0.4).min(1.0) * 0.45 + (area / rectangle.0).min(1.0) * 0.55;
    let mut score = geometry * 0.6
        - f64::from(u32::try_from(border_count).unwrap_or(4)) * 0.12;
    if score + 0.4 >= ACCEPTANCE {
        let polygon = ordered(polygon);
        score += boundary(polygon, paper, lab) * 0.4;
        // Both sides continuing below a divider means it is not the bottom.
        score -= continuation(polygon, paper, distance) * 0.5;
    }
    Some((score, ratio))
}

fn boundary(polygon: [Point; 4], paper: &GrayImage, lab: &RgbImage) -> f64 {
    let offset =
        (f64::from(paper.width().min(paper.height())) * 0.015).max(3.0);
    let mut sides = [0.0; 4];
    let mut colors = [0.0; 4];
    for i in 0..4 {
        let [x, y] = polygon[i];
        let end = polygon[(i + 1) % 4];
        let (dx, dy) = (end[0] - x, end[1] - y);
        let length = dx.hypot(dy);
        let (nx, ny) = (-dy / length * offset, dx / length * offset);
        let (mut sum, mut color, mut count) = (0.0, 0.0, 0);
        for sample in 0..24 {
            let t = (f64::from(sample) + 0.5) / 24.0;
            let (x, y) = (x + dx * t, y + dy * t);
            if let (Some(inside), Some(outside)) = (
                pixels::at(paper, x + nx, y + ny),
                pixels::at(paper, x - nx, y - ny),
            ) {
                sum += f64::from(inside) - f64::from(outside);
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss
                )]
                let color_at = |x: f64, y: f64| {
                    // at() checked the same rounded coordinates above.
                    lab[(x.round() as u32, y.round() as u32)]
                };
                let first = color_at(x + nx, y + ny);
                let second = color_at(x - nx, y - ny);
                let delta = first
                    .0
                    .into_iter()
                    .zip(second.0)
                    .map(|(a, b)| (f64::from(a) - f64::from(b)).powi(2))
                    .sum::<f64>()
                    .sqrt();
                color += (delta / 45.0).clamp(0.0, 1.0);
                count += 1;
            }
        }
        sides[i] = if count == 0 {
            0.0
        } else {
            sum / f64::from(count)
        };
        colors[i] = if count == 0 {
            0.0
        } else {
            color / f64::from(count)
        };
    }
    let combined = |values: [f64; 4]| {
        values.iter().sum::<f64>() / 4.0 * 0.6
            + values.into_iter().fold(f64::INFINITY, f64::min) * 0.4
    };
    (combined(sides) / 0.6)
        .clamp(0.0, 1.0)
        .max(combined(colors))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn continuation(
    polygon: [Point; 4],
    image: &GrayImage,
    distance: &[f32],
) -> f64 {
    let radius =
        (f64::from(image.width().min(image.height())) * 0.006).max(2.0);
    let support = |first: Point, second: Point| {
        let (mut count, mut supported) = (0, 0);
        for sample in 0..16 {
            let t = 1.04 + 0.16 * (f64::from(sample) + 0.5) / 16.0;
            let x = (first[0] + (second[0] - first[0]) * t).round();
            let y = (first[1] + (second[1] - first[1]) * t).round();
            if x < 0.0
                || y < 0.0
                || x >= f64::from(image.width())
                || y >= f64::from(image.height())
            {
                continue;
            }
            supported += i32::from(
                f64::from(
                    distance[y as usize * image.width() as usize + x as usize],
                ) <= radius,
            );
            count += 1;
        }
        if count == 0 {
            0.0
        } else {
            f64::from(supported) / f64::from(count)
        }
    };
    support(polygon[0], polygon[3]).min(support(polygon[1], polygon[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_limits_are_checked_before_analysis() {
        assert_eq!(analysis_size(0, 1), Err(ImageInputError::Empty));
        assert_eq!(
            analysis_size(u32::MAX, u32::MAX),
            Err(ImageInputError::TooManyPixels)
        );
        assert_eq!(analysis_size(50_000_000, 1), Ok((1280, 1)));
        assert_eq!(analysis_size(1920, 1080), Ok((1280, 720)));
    }
}
