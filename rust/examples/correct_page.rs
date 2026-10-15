//! Native correction/filter probe, without page editing or project mutation.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("The correction preview command needs a native filesystem.");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use atomicwrites::{AtomicFile, DisallowOverwrite};
    use fastfill_rust::correction::{PageCorrection, correct_image};
    use fastfill_rust::image_input::{MAX_IMAGE_BYTES, decode_image};
    use image::{ImageFormat, RgbaImage};
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{self, Read};
    use std::path::{Path, PathBuf};

    const USAGE: &str =
        "Usage: correct_page INPUT OUTPUT.png [CORRECTION.json]";
    const SETTINGS_LIMIT: usize = 16 * 1024;

    struct Arguments {
        input: PathBuf,
        output: PathBuf,
        settings: Option<PathBuf>,
    }

    impl Arguments {
        fn parse(arguments: &[OsString]) -> Result<Self, &'static str> {
            if !(2..=3).contains(&arguments.len())
                || arguments.iter().any(|argument| argument.is_empty())
            {
                return Err(USAGE);
            }
            let output = PathBuf::from(&arguments[1]);
            if !output
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
            {
                return Err("Choose a new .png output filename.");
            }
            Ok(Self {
                input: arguments[0].clone().into(),
                output,
                settings: arguments.get(2).map(PathBuf::from),
            })
        }
    }

    pub(super) fn run() -> Result<(), Box<dyn Error>> {
        let arguments =
            Arguments::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
        let dimensions = preview(&arguments)?;
        println!(
            "Wrote {} ({} × {}).",
            arguments.output.display(),
            dimensions.0,
            dimensions.1
        );
        Ok(())
    }

    fn read_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
        let file = File::open(path)?;
        let limit_bytes = u64::try_from(limit).expect("usize fits in u64");
        if file.metadata()?.len() > limit_bytes {
            return Err(io::Error::other("Input exceeds its byte limit."));
        }
        let mut bytes = Vec::new();
        file.take(limit_bytes + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::other("Input exceeds its byte limit."));
        }
        Ok(bytes)
    }

    fn preview(arguments: &Arguments) -> Result<(u32, u32), Box<dyn Error>> {
        if arguments.output.try_exists()? {
            return Err("Output already exists; choose a new filename.".into());
        }
        let correction = arguments.settings.as_ref().map_or_else(
            || Ok(PageCorrection::default()),
            |path| -> Result<_, Box<dyn Error>> {
                Ok(serde_json::from_slice(&read_bounded(
                    path,
                    SETTINGS_LIMIT,
                )?)?)
            },
        )?;
        let bytes = read_bounded(&arguments.input, MAX_IMAGE_BYTES)?;
        let (_, source) = decode_image(&bytes)?;
        let corrected = correct_image(&source.into_rgba8(), correction)?;
        let dimensions = corrected.dimensions();
        write_png(&arguments.output, corrected)?;
        Ok(dimensions)
    }

    fn write_png(path: &Path, image: RgbaImage) -> Result<(), Box<dyn Error>> {
        AtomicFile::new(path, DisallowOverwrite).write(|file| {
            image::DynamicImage::ImageRgba8(image)
                .write_to(file, ImageFormat::Png)
        })?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        struct Directory(PathBuf);

        impl Directory {
            fn new() -> Self {
                let path = std::env::temp_dir().join(format!(
                    "fastfill-correction-test-{}",
                    uuid::Uuid::new_v4()
                ));
                std::fs::create_dir(&path).unwrap();
                Self(path)
            }
        }

        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn arguments_reject_missing_extra_empty_and_non_png_destinations() {
            for arguments in [
                vec![],
                vec!["in.png"],
                vec!["", "out.png"],
                vec!["in.png", ""],
                vec!["in.png", "out.png", ""],
                vec!["in.png", "out.png", "a", "b"],
                vec!["in.png", "out.jpg"],
            ] {
                let arguments = arguments
                    .into_iter()
                    .map(OsString::from)
                    .collect::<Vec<_>>();
                assert!(Arguments::parse(&arguments).is_err());
            }
            let args = ["sample.png", "result.PNG", "settings.json"]
                .map(OsString::from);
            assert!(Arguments::parse(&args).is_ok());
        }

        #[test]
        fn preview_uses_settings_and_never_changes_existing_input_or_output() {
            let directory = Directory::new();
            let input = directory.0.join("input.png");
            let settings = directory.0.join("settings.json");
            let output = directory.0.join("output.png");
            let source =
                RgbaImage::from_pixel(9, 7, image::Rgba([30, 50, 90, 255]));
            write_png(&input, source.clone()).unwrap();
            std::fs::write(&settings, br#"{"rotationQuarterTurns":1}"#)
                .unwrap();
            let arguments = Arguments {
                input: input.clone(),
                output: output.clone(),
                settings: Some(settings.clone()),
            };
            assert_eq!(preview(&arguments).unwrap(), (7, 9));
            let (_, decoded) =
                decode_image(&std::fs::read(&output).unwrap()).unwrap();
            assert_eq!(
                decoded.into_rgba8(),
                image::imageops::rotate90(&source)
            );
            let saved = std::fs::read(&output).unwrap();
            assert!(preview(&arguments).is_err());
            assert!(write_png(&output, source.clone()).is_err());
            assert_eq!(std::fs::read(&output).unwrap(), saved);
            let (_, decoded) =
                decode_image(&std::fs::read(&input).unwrap()).unwrap();
            assert_eq!(decoded.into_rgba8(), source);
            let bad = Arguments {
                output: directory.0.join("bad.png"),
                ..arguments
            };
            std::fs::write(settings, b"invalid JSON").unwrap();
            assert!(preview(&bad).is_err());
            assert!(!bad.output.exists());
        }

        #[test]
        fn bounded_reads_reject_oversized_and_invalid_settings_without_output()
        {
            let directory = Directory::new();
            let input = directory.0.join("too-large");
            std::fs::write(&input, b"12345").unwrap();
            assert!(read_bounded(&input, 4).is_err());
            assert_eq!(read_bounded(&input, 5).unwrap(), b"12345");
            let output = directory.0.join("unused.png");
            let settings = directory.0.join("settings.json");
            let arguments = Arguments {
                input,
                output: output.clone(),
                settings: Some(settings.clone()),
            };
            for invalid in [
                b"".as_slice(),
                b"{\"rotationQuarterTurns\":1.5}",
                b"{\"filter\":1}",
            ] {
                std::fs::write(&settings, invalid).unwrap();
                assert!(preview(&arguments).is_err());
                assert!(!output.exists());
            }
            std::fs::write(settings, vec![b' '; SETTINGS_LIMIT + 1]).unwrap();
            assert!(preview(&arguments).is_err());
            assert!(!output.exists());
        }

        #[test]
        fn preview_applies_tone_settings_and_refuses_invalid_adjustments() {
            let directory = Directory::new();
            let input = directory.0.join("source.png");
            let settings = directory.0.join("filter.json");
            let output = directory.0.join("filtered.png");
            write_png(
                &input,
                RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 127])),
            )
            .unwrap();
            std::fs::write(
                &settings,
                br#"{"brightness":20,"filter":{"mode":2,"contrast":0}}"#,
            )
            .unwrap();
            let arguments = Arguments {
                input,
                output,
                settings: Some(settings.clone()),
            };
            assert_eq!(preview(&arguments).unwrap(), (1, 1));
            let (_, pixels) =
                decode_image(&std::fs::read(&arguments.output).unwrap())
                    .unwrap();
            assert_eq!(
                pixels.into_rgba8()[(0, 0)],
                image::Rgba([127, 127, 127, 127])
            );
            let invalid = Arguments {
                output: directory.0.join("invalid.png"),
                ..arguments
            };
            for bad in [
                br#"{"filter":{"mode":2,"contrast":101}}"#.as_slice(),
                br#"{"brightness":101}"#,
                br#"{"brightness":-101}"#,
            ] {
                std::fs::write(&settings, bad).unwrap();
                let error = preview(&invalid).unwrap_err().to_string();
                assert!(error.contains("between -100 and 100"));
                assert!(!invalid.output.exists());
            }
        }
    }
}
