//! Image preparation, calibrated binary masks, and edge hysteresis.

use crate::image_color;
use image::{GrayImage, Luma, Rgb, RgbImage, RgbaImage};

pub(super) fn prepare(
    source: &RgbaImage,
    width: u32,
    height: u32,
) -> (GrayImage, GrayImage, RgbImage) {
    let rgb = RgbImage::from_fn(source.width(), source.height(), |x, y| {
        let pixel = source[(x, y)];
        let alpha = u32::from(pixel[3]);
        Rgb(std::array::from_fn(|channel| {
            u8::try_from(
                (u32::from(pixel[channel]) * alpha + 255 * (255 - alpha) + 127)
                    / 255,
            )
            .unwrap_or(255)
        }))
    });
    let rgb = if rgb.dimensions() == (width, height) {
        rgb
    } else {
        // Installed resampler; thresholds stay calibrated, pixels may differ.
        image::imageops::resize(
            &rgb,
            width,
            height,
            image::imageops::FilterType::Triangle,
        )
    };
    let gray = GrayImage::from_fn(width, height, |x, y| {
        let [r, g, b] = rgb[(x, y)].0.map(u32::from);
        Luma([u8::try_from((299 * r + 587 * g + 114 * b + 500) / 1000)
            .unwrap_or(255)])
    });
    let paper = GrayImage::from_fn(width, height, |x, y| {
        let [r, g, b] = rgb[(x, y)].0.map(u32::from);
        let (value, minimum) = (r.max(g).max(b), r.min(g).min(b));
        // OpenCV HSV: value >= 115, saturation <= 105 (byte scale).
        Luma([u8::from(
            value >= 115 && (value - minimum) * 255 <= value * 105,
        )])
    });
    let linear = image_color::linear_table();
    let lab = RgbImage::from_fn(width, height, |x, y| {
        Rgb(image_color::to_lab(rgb[(x, y)].0, &linear))
    });
    let lab = image::imageops::blur_advanced(
        &lab,
        image::imageops::GaussianBlurParameters::new_from_kernel_size(9.0),
    );
    (gray, paper, lab)
}

pub(super) fn morph(
    image: &GrayImage,
    radius: i32,
    ellipse: bool,
    dilate: bool,
) -> GrayImage {
    let (width, height) = image.dimensions();
    let stride = width as usize + 1;
    let mut prefix = Vec::with_capacity(stride * height as usize);
    for row in image.as_raw().chunks_exact(width as usize) {
        let mut sum = 0_u32;
        prefix.push(sum);
        for value in row {
            sum += u32::from(*value != 0);
            prefix.push(sum);
        }
    }
    let spans = (-radius..=radius)
        .map(|dy| {
            // dy lies within the radius: the square-root argument is >= 0.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let dx = if ellipse {
                (f64::from(radius * radius - dy * dy).sqrt().round()) as u32
            } else {
                radius.unsigned_abs()
            };
            (dy, dx)
        })
        .collect::<Vec<_>>();
    GrayImage::from_fn(width, height, |x, y| {
        let mut found = !dilate;
        for &(dy, dx) in &spans {
            let Some(row) =
                y.checked_add_signed(dy).filter(|row| *row < height)
            else {
                continue;
            };
            let (left, right) =
                (x.saturating_sub(dx), x.saturating_add(dx + 1).min(width));
            let start = row as usize * stride;
            let count =
                prefix[start + right as usize] - prefix[start + left as usize];
            let active = if dilate {
                count > 0
            } else {
                count == right - left
            };
            if active == dilate {
                found = active;
                break;
            }
        }
        Luma([u8::from(found)])
    })
}

pub(super) fn gaussian5(image: &GrayImage) -> GrayImage {
    // OpenCV's sigma=0, size=5 uses exact binomial weights, not sigma=1.1.
    let weights = [1_u32, 4, 6, 4, 1];
    let (width, height) = (image.width(), image.height());
    let mirror = |index: i64, size: u32| {
        let size = i64::from(size);
        let value = if index < 0 {
            -index
        } else if index >= size {
            2 * size - index - 2
        } else {
            index
        };
        u32::try_from(value).unwrap_or(0)
    };
    let mut horizontal = vec![0_u32; width as usize * height as usize];
    for y in 0..height {
        for x in 0..width {
            horizontal[y as usize * width as usize + x as usize] = weights
                .iter()
                .zip(-2..=2)
                .map(|(weight, dx)| {
                    *weight
                        * u32::from(
                            image[(mirror(i64::from(x) + dx, width), y)][0],
                        )
                })
                .sum();
        }
    }
    GrayImage::from_fn(width, height, |x, y| {
        let sum = weights
            .iter()
            .zip(-2..=2)
            .map(|(weight, dy)| {
                *weight
                    * horizontal[mirror(i64::from(y) + dy, height) as usize
                        * width as usize
                        + x as usize]
            })
            .sum::<u32>();
        Luma([u8::try_from((sum + 128) / 256).unwrap_or(255)])
    })
}

pub(super) fn canny(image: &GrayImage, low: i32, high: i32) -> GrayImage {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut magnitude = vec![0_i32; width * height];
    let mut directions = vec![0_u8; width * height];
    let source = image.as_raw();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let index = y * width + x;
            let at = |i| i32::from(source[i]);
            let (top, bottom) = (index - width, index + width);
            let dx = at(top + 1) + 2 * at(index + 1) + at(bottom + 1)
                - at(top - 1)
                - 2 * at(index - 1)
                - at(bottom - 1);
            let dy = at(bottom - 1) + 2 * at(bottom) + at(bottom + 1)
                - at(top - 1)
                - 2 * at(top)
                - at(top + 1);
            magnitude[index] = dx.abs() + dy.abs();
            directions[index] = if f64::from(dy.abs())
                <= f64::from(dx.abs()) * 0.414_213_562_373_095
            {
                0
            } else if f64::from(dx.abs())
                <= f64::from(dy.abs()) * 0.414_213_562_373_095
            {
                1
            } else if dx.signum() == dy.signum() {
                2
            } else {
                3
            };
        }
    }
    let mut flags = vec![0_u8; width * height];
    let mut queue = Vec::new();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let i = y * width + x;
            let (a, b) = match directions[i] {
                0 => (i - 1, i + 1),
                1 => (i - width, i + width),
                2 => (i - width - 1, i + width + 1),
                _ => (i - width + 1, i + width - 1),
            };
            if magnitude[i] > low
                && magnitude[i] > magnitude[a]
                && magnitude[i] >= magnitude[b]
            {
                flags[i] = 1;
                if magnitude[i] > high {
                    flags[i] = 2;
                    queue.push(i);
                }
            }
        }
    }
    let mut next = 0;
    while next < queue.len() {
        let index = queue[next];
        next += 1;
        let (x, y) = (index % width, index / width);
        for row in y.saturating_sub(1)..=(y + 1).min(height - 1) {
            for col in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                let neighbor = row * width + col;
                if flags[neighbor] == 1 {
                    flags[neighbor] = 2;
                    queue.push(neighbor);
                }
            }
        }
    }
    GrayImage::from_fn(image.width(), image.height(), |x, y| {
        Luma([u8::from(flags[y as usize * width + x as usize] == 2)])
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn at(image: &GrayImage, x: f64, y: f64) -> Option<u8> {
    let (x, y) = (x.round(), y.round());
    if x < 0.0
        || y < 0.0
        || x >= f64::from(image.width())
        || y >= f64::from(image.height())
    {
        None
    } else {
        Some(image[(x as u32, y as u32)][0])
    }
}

pub(super) fn sharpness(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    let (mut sum, mut squares) = (0.0, 0.0);
    for y in 0..height {
        for x in 0..width {
            let (left, right) = (
                if x == 0 { 1 } else { x - 1 },
                if x + 1 == width { x - 1 } else { x + 1 },
            );
            let (top, bottom) = (
                if y == 0 { 1 } else { y - 1 },
                if y + 1 == height { y - 1 } else { y + 1 },
            );
            let value = f64::from(image[(left, y)][0])
                + f64::from(image[(right, y)][0])
                + f64::from(image[(x, top)][0])
                + f64::from(image[(x, bottom)][0])
                - 4.0 * f64::from(image[(x, y)][0]);
            sum += value;
            squares += value * value;
        }
    }
    let count = f64::from(width) * f64::from(height);
    (squares / count - (sum / count).powi(2)).max(0.0)
}

pub(super) fn edge_distances(image: &GrayImage) -> Vec<f32> {
    // OpenCV L2 mask-3 chamfer weights, without a native distance transform.
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut distance = image
        .as_raw()
        .iter()
        .map(|v| if *v == 0 { f32::INFINITY } else { 0.0 })
        .collect::<Vec<_>>();
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if x > 0 {
                distance[i] = distance[i].min(distance[i - 1] + 0.955);
            }
            if y > 0 {
                distance[i] = distance[i].min(distance[i - width] + 0.955);
                if x > 0 {
                    distance[i] =
                        distance[i].min(distance[i - width - 1] + 1.3693);
                }
                if x + 1 < width {
                    distance[i] =
                        distance[i].min(distance[i - width + 1] + 1.3693);
                }
            }
        }
    }
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let i = y * width + x;
            if x + 1 < width {
                distance[i] = distance[i].min(distance[i + 1] + 0.955);
            }
            if y + 1 < height {
                distance[i] = distance[i].min(distance[i + width] + 0.955);
                if x > 0 {
                    distance[i] =
                        distance[i].min(distance[i + width - 1] + 1.3693);
                }
                if x + 1 < width {
                    distance[i] =
                        distance[i].min(distance[i + width + 1] + 1.3693);
                }
            }
        }
    }
    distance
}
