//! Contrast-limited local Lab luminance equalization with blended tile LUTs.

use super::byte;
use image::RgbaImage;

pub(super) fn equalize(image: &mut RgbaImage) {
    let columns = image.width().min(8);
    let rows = image.height().min(8);
    let tile_width = image.width().div_ceil(columns);
    let tile_height = image.height().div_ceil(rows);
    let mut tables =
        Vec::with_capacity(usize::try_from(columns * rows).unwrap());
    for row in 0..rows {
        for column in 0..columns {
            let mut histogram = [0_u32; 256];
            // Edge tiles use real pixels, not a padded full-size image.
            for y in
                row * tile_height..((row + 1) * tile_height).min(image.height())
            {
                for x in column * tile_width
                    ..((column + 1) * tile_width).min(image.width())
                {
                    let pixel = image[(x, y)];
                    if pixel[3] != 0 {
                        histogram[usize::from(pixel[0])] += 1;
                    }
                }
            }
            tables.push(table(histogram));
        }
    }
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }
        let (left, right, fx) = axis(x, tile_width, columns);
        let (top, bottom, fy) = axis(y, tile_height, rows);
        let value = |column, row| {
            let index = usize::try_from(row * columns + column).unwrap();
            f32::from(tables[index][usize::from(pixel[0])])
        };
        let first = value(left, top) * (1.0 - fx) + value(right, top) * fx;
        let second =
            value(left, bottom) * (1.0 - fx) + value(right, bottom) * fx;
        pixel[0] = byte(first * (1.0 - fy) + second * fy);
    }
}

fn table(mut histogram: [u32; 256]) -> [u8; 256] {
    let count = histogram.iter().sum::<u32>();
    if histogram.iter().filter(|count| **count != 0).count() <= 1 {
        // Empty/flat tiles must not blow tiny or uniform images to white.
        return std::array::from_fn(|index| u8::try_from(index).unwrap());
    }
    let limit = (2 * count / 256).max(1);
    let mut excess = 0;
    for bin in &mut histogram {
        excess += bin.saturating_sub(limit);
        *bin = (*bin).min(limit);
    }
    for bin in &mut histogram {
        *bin += excess / 256;
    }
    let remaining = excess % 256;
    for index in 0..remaining {
        histogram[usize::try_from(index * 256 / remaining).unwrap()] += 1;
    }
    let mut cumulative = 0;
    std::array::from_fn(|index| {
        cumulative += histogram[index];
        u8::try_from(
            (u64::from(cumulative) * 255 + u64::from(count) / 2)
                / u64::from(count),
        )
        .expect("normalized histogram byte")
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn axis(position: u32, size: u32, count: u32) -> (u32, u32, f32) {
    // At most eight tiles: the signed floor lies in -1..7.
    let coordinate = f64::from(position) / f64::from(size) - 0.5;
    let lower = coordinate.floor() as i32;
    let maximum = i32::try_from(count - 1).unwrap();
    (
        lower.clamp(0, maximum) as u32,
        (lower + 1).clamp(0, maximum) as u32,
        (coordinate - coordinate.floor()) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipped_histogram_stays_monotone_and_boosts_local_separation() {
        let mut histogram = [0; 256];
        histogram[100] = 512;
        histogram[110] = 512;
        let lut = table(histogram);
        assert!(lut.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(lut[255], 255);
        assert!(lut[110] - lut[100] > 10);
    }

    #[test]
    fn blended_tile_boundaries_do_not_introduce_hard_seams() {
        let mut lab = RgbaImage::from_fn(127, 123, |x, _| {
            image::Rgba([u8::try_from(80 + x).unwrap(), 128, 128, 255])
        });
        equalize(&mut lab);
        for x in 1..127 {
            assert!(lab[(x, 60)][0].abs_diff(lab[(x - 1, 60)][0]) <= 8);
        }
    }
}
