//! sRGB/D65 <-> 8-bit CIE Lab; L spans 0..255, a/b are offset by 128.

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub(super) fn linear_table() -> [f32; 256] {
    std::array::from_fn(|index| {
        let value = f32::from(u8::try_from(index).expect("byte index")) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    })
}

pub(super) fn to_lab(rgb: [u8; 3], linear: &[f32; 256]) -> [u8; 3] {
    let [red, green, blue] = rgb.map(|channel| linear[usize::from(channel)]);
    let x = (0.412_456_4 * red + 0.357_576_1 * green + 0.180_437_5 * blue)
        / 0.95047;
    let y = 0.212_672_9 * red + 0.715_152_2 * green + 0.072_175 * blue;
    let z =
        (0.019_333_9 * red + 0.119_192 * green + 0.950_304_1 * blue) / 1.08883;
    let curve = |value: f32| {
        if value > 216.0 / 24389.0 {
            value.cbrt()
        } else {
            ((24389.0 / 27.0) * value + 16.0) / 116.0
        }
    };
    let [x, y, z] = [x, y, z].map(curve);
    [
        byte((116.0 * y - 16.0) * 2.55),
        byte(500.0 * (x - y) + 128.0),
        byte(200.0 * (y - z) + 128.0),
    ]
}

pub(super) fn to_rgb(lab: [u8; 3]) -> [u8; 3] {
    let y = (f32::from(lab[0]) / 2.55 + 16.0) / 116.0;
    let x = y + (f32::from(lab[1]) - 128.0) / 500.0;
    let z = y - (f32::from(lab[2]) - 128.0) / 200.0;
    let inverse = |value: f32| {
        let cube = value * value * value;
        if cube > 216.0 / 24389.0 {
            cube
        } else {
            (116.0 * value - 16.0) / (24389.0 / 27.0)
        }
    };
    let [x, y, z] = [x, y, z].map(inverse);
    let x = x * 0.95047;
    let z = z * 1.08883;
    let rgb = [
        3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z,
        -0.969_266 * x + 1.876_010_8 * y + 0.041_556 * z,
        0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z,
    ];
    rgb.map(|value| {
        byte(
            255.0
                * if value <= 0.003_130_8 {
                    12.92 * value
                } else {
                    1.055 * value.powf(1.0 / 2.4) - 0.055
                },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_reference_colors_and_midtone_round_trips() {
        let linear = linear_table();
        for (rgb, expected) in [
            ([0, 0, 0], [0, 128, 128]),
            ([255, 255, 255], [255, 128, 128]),
            ([255, 0, 0], [136, 208, 195]),
            ([0, 255, 0], [224, 42, 211]),
            ([0, 0, 255], [82, 207, 20]),
        ] {
            for (channel, expected) in
                to_lab(rgb, &linear).into_iter().zip(expected)
            {
                assert!(channel.abs_diff(expected) <= 1);
            }
        }
        for rgb in [[60, 110, 160], [180, 140, 90], [128; 3], [240; 3]] {
            for (channel, original) in
                to_rgb(to_lab(rgb, &linear)).into_iter().zip(rgb)
            {
                assert!(channel.abs_diff(original) <= 3);
            }
        }
    }
}
