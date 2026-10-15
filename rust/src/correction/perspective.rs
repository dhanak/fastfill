//! Inverse projective mapping from output pixel centers to source samples.

use super::CorrectionError;
use crate::project::CropQuad;
use image::{Rgba, RgbaImage};

pub(super) struct Projection {
    x: [f64; 3],
    y: [f64; 3],
    denominator: [f64; 2],
}

impl Projection {
    pub(super) fn new(crop: CropQuad) -> Result<Self, CorrectionError> {
        if !crop.is_valid() {
            return Err(CorrectionError::InvalidCrop);
        }
        let [tl, tr, br, bl] = crop
            .points()
            .map(|point| [f64::from(point.x), f64::from(point.y)]);
        let dx1 = tr[0] - br[0];
        let dx2 = bl[0] - br[0];
        let dx3 = tl[0] - tr[0] + br[0] - bl[0];
        let dy1 = tr[1] - br[1];
        let dy2 = bl[1] - br[1];
        let dy3 = tl[1] - tr[1] + br[1] - bl[1];
        let determinant = dx1 * dy2 - dx2 * dy1;
        let g = (dx3 * dy2 - dx2 * dy3) / determinant;
        let h = (dx1 * dy3 - dx3 * dy1) / determinant;
        // The denominator is linear: checking its four extrema is enough.
        if [1.0, 1.0 + g, 1.0 + h, 1.0 + g + h]
            .iter()
            .any(|value| !value.is_finite() || *value <= 1e-12)
        {
            return Err(CorrectionError::InvalidCrop);
        }
        Ok(Self {
            x: [tr[0] * (g + 1.0) - tl[0], bl[0] * (h + 1.0) - tl[0], tl[0]],
            y: [tr[1] * (g + 1.0) - tl[1], bl[1] * (h + 1.0) - tl[1], tl[1]],
            denominator: [g, h],
        })
    }

    fn map(&self, u: f64, v: f64) -> (f64, f64) {
        let denominator =
            self.denominator[0] * u + self.denominator[1] * v + 1.0;
        let apply = |row: [f64; 3]| {
            ((row[0] * u + row[1] * v + row[2]) / denominator).clamp(0.0, 1.0)
        };
        (apply(self.x), apply(self.y))
    }

    pub(super) fn warp(
        &self,
        source: &RgbaImage,
        width: u32,
        height: u32,
    ) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            // A one-pixel dimension represents the center of that crop axis.
            let axis = |position, length: u32| {
                if length == 1 {
                    0.5
                } else {
                    f64::from(position) / f64::from(length - 1)
                }
            };
            let (u, v) = self.map(axis(x, width), axis(y, height));
            bilinear(
                source,
                u * f64::from(source.width() - 1),
                v * f64::from(source.height() - 1),
            )
        })
    }
}

fn bilinear(source: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    // Bounds were checked before allocating; x/y are clamped source centers.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (left, top) = (x.floor() as u32, y.floor() as u32);
    let right = (left + 1).min(source.width() - 1);
    let bottom = (top + 1).min(source.height() - 1);
    let fx = x - f64::from(left);
    let fy = y - f64::from(top);
    let samples = [
        (source[(left, top)], (1.0 - fx) * (1.0 - fy)),
        (source[(right, top)], fx * (1.0 - fy)),
        (source[(left, bottom)], (1.0 - fx) * fy),
        (source[(right, bottom)], fx * fy),
    ];
    // imageops' bilinear sampler interpolates straight alpha, causing dark
    // fringes. Accumulate premultiplied channels without a float-image copy.
    let mut alpha = 0.0;
    let mut rgb = [0.0; 3];
    for (pixel, weight) in samples {
        let weight = weight * f64::from(pixel[3]);
        alpha += weight;
        for (sum, channel) in rgb.iter_mut().zip(pixel.0) {
            *sum += weight * f64::from(channel);
        }
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = |value: f64| value.round().clamp(0.0, 255.0) as u8;
    let rgb =
        rgb.map(|value| byte(if alpha > 0.0 { value / alpha } else { 0.0 }));
    Rgba([rgb[0], rgb[1], rgb[2], byte(alpha)])
}
