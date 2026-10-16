//! Offline framing overlay/JSON; never replaces the source or an output file.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The frame probe needs a native filesystem.");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use atomicwrites::{AtomicFile, DisallowOverwrite};
    use fastfill_rust::document_detection::{
        DocumentDetection, detect_document,
    };
    use fastfill_rust::image_input::{MAX_IMAGE_BYTES, decode_image};
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    fn arguments(
        args: &[OsString],
    ) -> Result<(PathBuf, PathBuf), &'static str> {
        if args.len() != 2 || args.iter().any(|arg| arg.is_empty()) {
            return Err("Usage: frame_probe INPUT OUTPUT.png");
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
        let start = Instant::now();
        let detection = probe(&input, &output)?;
        println!("{}", serde_json::to_string_pretty(&detection)?);
        eprintln!(
            "Wrote {}: {}, confidence {:.3}, {:?}.",
            output.display(),
            if detection.corners.is_some() {
                "accepted"
            } else if detection.candidate_corners.is_some() {
                "candidate only"
            } else {
                "no candidate"
            },
            detection.confidence,
            start.elapsed(),
        );
        Ok(())
    }

    fn probe(
        input: &Path,
        output: &Path,
    ) -> Result<DocumentDetection, Box<dyn Error>> {
        if output.try_exists()? {
            return Err("Output already exists; choose a new filename.".into());
        }
        let file = File::open(input)?;
        let limit = u64::try_from(MAX_IMAGE_BYTES)?;
        if file.metadata()?.len() > limit {
            return Err("Input exceeds the 100 MiB limit.".into());
        }
        let mut bytes = Vec::new();
        file.take(limit + 1).read_to_end(&mut bytes)?;
        let (_, source) = decode_image(&bytes)?;
        let mut source = source.into_rgba8();
        let detection = detect_document(&source)?;
        overlay(&mut source, &detection);
        AtomicFile::new(output, DisallowOverwrite)
            .write(|file| source.write_to(file, ImageFormat::Png))?;
        Ok(detection)
    }

    fn overlay(image: &mut RgbaImage, detection: &DocumentDetection) {
        let Some(quad) = detection.corners.or(detection.candidate_corners)
        else {
            return;
        };
        let color = if detection.corners.is_some() {
            [0, 190, 80]
        } else {
            [245, 145, 0]
        };
        let (width, height) = image.dimensions();
        let points = [
            quad.top_left,
            quad.top_right,
            quad.bottom_right,
            quad.bottom_left,
        ]
        .map(|p| {
            (
                f64::from(p.x) * f64::from(width - 1),
                f64::from(p.y) * f64::from(height - 1),
            )
        });
        // Rasterize a translucent fill and 3 px outline, no font dependency.
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let (x, y) = (f64::from(x), f64::from(y));
            let mut inside = true;
            let mut edge = false;
            for i in 0..4 {
                let (a, b) = (points[i], points[(i + 1) % 4]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let cross = dx * (y - a.1) - dy * (x - a.0);
                inside &= cross >= 0.0;
                let amount = ((x - a.0) * dx + (y - a.1) * dy)
                    / (dx * dx + dy * dy).max(1.0);
                edge |= (0.0..=1.0).contains(&amount)
                    && cross.abs() / dx.hypot(dy).max(1.0) <= 1.5;
                edge |= (x - a.0).hypot(y - a.1) <= 5.0;
            }
            if edge {
                *pixel = Rgba([color[0], color[1], color[2], 255]);
            } else if inside {
                for (channel, tint) in pixel.0[..3].iter_mut().zip(color) {
                    *channel = u8::try_from(
                        (u16::from(*channel) * 7 + u16::from(tint)) / 8,
                    )
                    .unwrap_or(255);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn arguments_reject_missing_empty_extra_and_wrong_extensions() {
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
        fn existing_output_and_invalid_input_do_not_write_files() {
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
                "fastfill-frame-probe-{}.png",
                uuid::Uuid::new_v4()
            ));
            assert!(probe(&input, &output).is_err());
            assert!(!output.exists());
        }

        #[test]
        fn overlay_distinguishes_candidate_and_accepted_without_resizing() {
            let quad = fastfill_rust::project::CropQuad::default();
            let mut detection = DocumentDetection {
                corners: Some(quad),
                candidate_corners: None,
                confidence: 1.0,
                area_ratio: 1.0,
                sharpness: 0.0,
                analysis_width: 40,
                analysis_height: 30,
            };
            let mut source = RgbaImage::from_pixel(40, 30, Rgba([255; 4]));
            overlay(&mut source, &detection);
            assert_eq!(source.dimensions(), (40, 30));
            assert_eq!(source[(0, 0)].0, [0, 190, 80, 255]);
            assert_ne!(source[(20, 15)].0, [255; 4]);
            detection.corners = None;
            detection.candidate_corners = Some(quad);
            overlay(&mut source, &detection);
            assert_eq!(source[(0, 0)].0, [245, 145, 0, 255]);
            detection.candidate_corners = None;
            let before = source.clone();
            overlay(&mut source, &detection);
            assert_eq!(source, before);
        }
    }
}
