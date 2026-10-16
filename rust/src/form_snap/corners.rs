//! Shi–Tomasi corner hints; independent of form-rule thresholding and the GUI.

use super::{analysis_image, normalize_point, paper_gray, valid_query};
use crate::image_input::{ImageInputError, MAX_IMAGE_PIXELS};
use crate::project::NormalizedPoint;
use image::{GrayImage, RgbaImage};

#[derive(Debug)]
pub struct CornerFeatures {
    width: u32,
    height: u32,
    points: Vec<NormalizedPoint>,
}

impl CornerFeatures {
    /// Retain C# calibration: 300 corners, quality 0.02, 8 px spacing, block 5.
    /// Uses original grayscale pixels, not corrected/binarized page pixels.
    ///
    /// # Errors
    /// Rejects empty images and source dimensions beyond the 50 MP limit.
    pub fn analyze(source: &RgbaImage) -> Result<Self, ImageInputError> {
        if source.width() == 0 || source.height() == 0 {
            return Err(ImageInputError::Empty);
        }
        if u64::from(source.width()) * u64::from(source.height())
            > MAX_IMAGE_PIXELS
        {
            return Err(ImageInputError::TooManyPixels);
        }
        let gray = paper_gray(source);
        let image = analysis_image(&gray);
        let mut features = Self {
            width: image.width(),
            height: image.height(),
            points: Vec::new(),
        };
        if image.width() < 16 || image.height() < 16 {
            return Ok(features);
        }
        let width = usize::try_from(image.width())
            .map_err(|_| ImageInputError::TooManyPixels)?;
        let height = usize::try_from(image.height())
            .map_err(|_| ImageInputError::TooManyPixels)?;
        let scores = responses(&image, width, height);
        let peak = scores.iter().copied().fold(0.0_f32, f32::max);
        let mut candidates = Vec::new();
        for y in 2..height - 2 {
            for x in 2..width - 2 {
                let index = y * width + x;
                let score = scores[index];
                if score <= peak * 0.02 {
                    continue;
                }
                let maximum = (y - 1..=y + 1).all(|row| {
                    (x - 1..=x + 1).all(|column| {
                        let other = row * width + column;
                        let order = score.total_cmp(&scores[other]);
                        order.is_gt() || order.is_eq() && index <= other
                    })
                });
                if maximum {
                    candidates.push((index, score));
                }
            }
        }
        candidates
            .sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut selected: Vec<(usize, usize)> = Vec::new();
        for (index, _) in candidates {
            let (x, y) = (index % width, index / width);
            if selected.iter().any(|(a, b)| {
                x.abs_diff(*a).pow(2) + y.abs_diff(*b).pow(2) < 64
            }) {
                continue;
            }
            selected.push((x, y));
            features.points.push(normalize_point(x, y, width, height));
            if selected.len() == 300 {
                break;
            }
        }
        Ok(features)
    }

    #[must_use]
    pub const fn analysis_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[must_use]
    pub fn corners(&self) -> &[NormalizedPoint] {
        &self.points
    }

    /// Radius is a fraction of the image's shorter side, as in form snapping.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn find_corner(
        &self,
        point: NormalizedPoint,
        radius: f32,
    ) -> Option<NormalizedPoint> {
        if !valid_query(point, radius) {
            return None;
        }
        self.points
            .iter()
            .copied()
            .filter_map(|corner| {
                // Both analysis dimensions are at most 1600, exact in f32.
                let distance = ((point.x - corner.x) * self.width as f32)
                    .hypot((point.y - corner.y) * self.height as f32)
                    / self.width.min(self.height) as f32;
                (distance <= radius).then_some((corner, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(corner, _)| corner)
    }
}

fn responses(image: &GrayImage, width: usize, height: usize) -> Vec<f32> {
    let source = image.as_raw();
    let mut moments = vec![[0.0_f32; 3]; width * height];
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let (top, mid, bottom) =
                ((y - 1) * width + x, y * width + x, (y + 1) * width + x);
            let at = |i| f32::from(source[i]);
            let dx = at(top + 1) + 2.0 * at(mid + 1) + at(bottom + 1)
                - at(top - 1)
                - 2.0 * at(mid - 1)
                - at(bottom - 1);
            let dy = at(bottom - 1) + 2.0 * at(bottom) + at(bottom + 1)
                - at(top - 1)
                - 2.0 * at(top)
                - at(top + 1);
            moments[mid] = [dx * dx, dy * dy, dx * dy];
        }
    }
    // Separable box sums avoid a 25-sample tensor loop at every pixel.
    let mut row = vec![[0.0; 3]; width];
    for line in moments.chunks_exact_mut(width) {
        row.copy_from_slice(line);
        for x in 2..width - 2 {
            line[x] = [0.0; 3];
            for sample in &row[x - 2..=x + 2] {
                for (sum, value) in line[x].iter_mut().zip(*sample) {
                    *sum += value;
                }
            }
        }
    }
    let mut scores = vec![0.0; width * height];
    for y in 2..height - 2 {
        for x in 2..width - 2 {
            let mut tensor = [0.0_f32; 3];
            for row in y - 2..=y + 2 {
                for (sum, value) in
                    tensor.iter_mut().zip(moments[row * width + x])
                {
                    *sum += value;
                }
            }
            let [xx, yy, xy] = tensor;
            scores[y * width + x] =
                ((xx + yy - (xx - yy).hypot(2.0 * xy)) / 2.0).max(0.0);
        }
    }
    scores
}
