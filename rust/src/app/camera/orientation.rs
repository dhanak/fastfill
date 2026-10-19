//! Explicit sensor orientation, independent of window shape.

use fastfill_rust::camera::catalog::Device;
use image::RgbaImage;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Orientation {
    turns: u8,
    pub(super) mirrored: bool,
}

impl Orientation {
    pub(super) fn for_device(device: &Device) -> Self {
        // Adapters expose names, not facing metadata. Don't guess for USB
        // or generic integrated cameras; require an explicit Front label.
        let mut labels = device.name.split(|c: char| !c.is_alphanumeric());
        let mirrored = labels
            .clone()
            .any(|word| word.eq_ignore_ascii_case("front"))
            && !labels.any(|word| {
                word.eq_ignore_ascii_case("rear")
                    || word.eq_ignore_ascii_case("back")
            });
        Self {
            mirrored,
            ..Self::default()
        }
    }

    pub(super) fn rotate(&mut self) {
        self.turns = (self.turns + 1) % 4;
    }

    pub(super) fn degrees(self) -> u16 {
        u16::from(self.turns) * 90
    }

    pub(super) fn apply(self, pixels: RgbaImage) -> RgbaImage {
        // Mirroring is a live-preview aid, never a source transformation.
        match self.turns {
            1 => image::imageops::rotate90(&pixels),
            2 => image::imageops::rotate180(&pixels),
            3 => image::imageops::rotate270(&pixels),
            _ => pixels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn default_mirror_requires_an_explicit_unambiguous_front_label() {
        for (name, mirrored) in [
            ("Microsoft Camera Front", true),
            ("FRONT-facing Camera", true),
            ("camera_front", true),
            ("Microsoft Camera Rear", false),
            ("USB Camera", false),
            ("Integrated Camera", false),
            ("Frontier Camera", false),
            ("Front / Rear Camera", false),
            ("", false),
        ] {
            let orientation = Orientation::for_device(&Device {
                index: 0,
                name: name.into(),
            });
            assert_eq!(orientation.mirrored, mirrored, "{name}");
            assert_eq!(orientation.degrees(), 0);
        }
    }

    #[test]
    fn quarter_turns_are_lossless_and_mirroring_never_changes_source_pixels() {
        let pixels = RgbaImage::from_fn(3, 2, |x, y| {
            Rgba([u8::try_from(y * 3 + x).unwrap(), 20, 40, 128])
        });
        let pointer = pixels.as_raw().as_ptr();
        let pixels = Orientation::default().apply(pixels);
        assert_eq!(pixels.as_raw().as_ptr(), pointer);
        let mut orientation = Orientation::default();
        for (width, height, expected) in [
            (3, 2, vec![0, 1, 2, 3, 4, 5]),
            (2, 3, vec![3, 0, 4, 1, 5, 2]),
            (3, 2, vec![5, 4, 3, 2, 1, 0]),
            (2, 3, vec![2, 5, 1, 4, 0, 3]),
        ] {
            let result = orientation.apply(pixels.clone());
            assert_eq!(result.dimensions(), (width, height));
            assert_eq!(
                result.pixels().map(|p| p[0]).collect::<Vec<_>>(),
                expected
            );
            let mirrored = Orientation {
                mirrored: true,
                ..orientation
            }
            .apply(pixels.clone());
            assert_eq!(mirrored, result);
            assert!(result.pixels().all(|p| p.0[1..] == [20, 40, 128]));
            orientation.rotate();
        }
        assert_eq!(orientation, Orientation::default());
    }
}
