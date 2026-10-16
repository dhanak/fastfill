//! Native candidate overlay/JSON diagnostic; does not mutate source images.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The form probe needs a native filesystem.");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use atomicwrites::{AtomicFile, DisallowOverwrite};
    use fastfill_rust::form_snap::FormFeatures;
    use fastfill_rust::image_input::{MAX_IMAGE_BYTES, decode_image};
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};

    const USAGE: &str = "Usage: form_probe INPUT OUTPUT.png";

    fn arguments(
        args: &[OsString],
    ) -> Result<(PathBuf, PathBuf), &'static str> {
        if args.len() != 2 || args.iter().any(|arg| arg.is_empty()) {
            return Err(USAGE);
        }
        let output = PathBuf::from(&args[1]);
        if !output
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            return Err("Choose a new .png output filename.");
        }
        Ok((PathBuf::from(&args[0]), output))
    }

    pub(super) fn run() -> Result<(), Box<dyn Error>> {
        let args = std::env::args_os().skip(1).collect::<Vec<_>>();
        let (input, output) = arguments(&args)?;
        let features = probe(&input, &output)?;
        println!("{}", serde_json::to_string_pretty(&features)?);
        eprintln!(
            "Wrote {}: {} blue boxes, {} orange rules.",
            output.display(),
            features.boxes().len(),
            features.horizontal_lines().len(),
        );
        Ok(())
    }

    fn probe(
        input: &Path,
        output: &Path,
    ) -> Result<FormFeatures, Box<dyn Error>> {
        if output.try_exists()? {
            return Err("Output already exists; choose a new filename.".into());
        }
        let source = File::open(input)?;
        let limit = u64::try_from(MAX_IMAGE_BYTES)?;
        if source.metadata()?.len() > limit {
            return Err("Input exceeds the 100 MiB limit.".into());
        }
        let mut bytes = Vec::new();
        source.take(limit + 1).read_to_end(&mut bytes)?;
        let (_, source) = decode_image(&bytes)?;
        let mut source = source.into_rgba8();
        let features = FormFeatures::analyze(&source)?;
        overlay(&mut source, &features);
        AtomicFile::new(output, DisallowOverwrite)
            .write(|file| source.write_to(file, ImageFormat::Png))?;
        Ok(features)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn pixel(normalized: f32, extent: u32) -> u32 {
        // Detector output is finite in 0..=1; clamp endpoint rounding only.
        #[allow(clippy::cast_precision_loss)]
        let value = normalized * extent as f32;
        (value.round() as u32).min(extent)
    }

    fn overlay(image: &mut RgbaImage, features: &FormFeatures) {
        let (width, height) = image.dimensions();
        let orange = Rgba([245, 125, 0, 255]);
        let blue = Rgba([0, 125, 245, 255]);
        for line in features.horizontal_lines() {
            horizontal(
                image,
                pixel(line.start.x, width - 1),
                pixel(line.end.x, width - 1),
                pixel(line.start.y, height - 1),
                orange,
            );
        }
        for bounds in features.boxes() {
            let left = pixel(bounds.x, width).min(width - 1);
            let top = pixel(bounds.y, height).min(height - 1);
            let right = pixel(bounds.x + bounds.width, width)
                .saturating_sub(1)
                .min(width - 1);
            let bottom = pixel(bounds.y + bounds.height, height)
                .saturating_sub(1)
                .min(height - 1);
            horizontal(image, left, right, top, blue);
            horizontal(image, left, right, bottom, blue);
            for y in top..=bottom {
                image.put_pixel(left, y, blue);
                image.put_pixel(right, y, blue);
            }
        }
    }

    fn horizontal(
        image: &mut RgbaImage,
        left: u32,
        right: u32,
        y: u32,
        color: Rgba<u8>,
    ) {
        for x in left..=right {
            image.put_pixel(x, y, color);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn args_reject_missing_empty_extra_and_non_png_outputs() {
            for args in [
                vec![],
                vec!["in.png"],
                vec!["", "out.png"],
                vec!["in.png", ""],
                vec!["in.png", "out.jpg"],
                vec!["in.png", "out.png", "extra"],
            ] {
                let args =
                    args.into_iter().map(OsString::from).collect::<Vec<_>>();
                assert!(arguments(&args).is_err());
            }
            assert!(arguments(&["in.jpg".into(), "out.PNG".into()]).is_ok());
        }

        #[test]
        fn overlay_keeps_source_dimensions_and_marks_candidates() {
            let (_, source) = decode_image(include_bytes!(
                "../../tests/samples/sample-form.png"
            ))
            .unwrap();
            let mut source = source.into_rgba8();
            let features = FormFeatures::analyze(&source).unwrap();
            overlay(&mut source, &features);
            assert_eq!(source.dimensions(), (1241, 1754));
            assert!(source.pixels().any(|p| p.0 == [0, 125, 245, 255]));
            assert!(source.pixels().any(|p| p.0 == [245, 125, 0, 255]));
        }

        #[test]
        fn existing_output_and_invalid_input_never_write_files() {
            let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
            let before = std::fs::read(&input).unwrap();
            assert!(
                probe(&input, &input)
                    .unwrap_err()
                    .to_string()
                    .contains("already exists")
            );
            assert_eq!(std::fs::read(&input).unwrap(), before);
            let output = std::env::temp_dir().join(format!(
                "fastfill-form-probe-{}.png",
                uuid::Uuid::new_v4()
            ));
            assert!(!output.exists());
            assert!(probe(&input, &output).is_err());
            assert!(!output.exists());
        }
    }
}
