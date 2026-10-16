//! Offline sequence inspection; virtual timestamps, no camera or GUI runtime.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("Frame replay needs a native filesystem.");
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use fastfill_rust::document_detection::{
        AutoCaptureGate, CaptureState, DetectionStabilizer, DocumentDetection,
        detect_document_with_hint,
    };
    use fastfill_rust::image_input::{
        ImageFormatKind, MAX_IMAGE_BYTES, decode_image,
    };
    use fastfill_rust::project::CropQuad;
    use serde::Serialize;
    use std::error::Error;
    use std::ffi::OsString;
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    const MAX_FRAMES: usize = 300;
    const USAGE: &str =
        "Usage: frame_replay INPUT_DIR NEW_OUTPUT_DIR [INTERVAL_MS=33]";

    struct Arguments {
        input: PathBuf,
        output: PathBuf,
        interval_ms: u64,
    }

    impl Arguments {
        fn parse(args: &[OsString]) -> Result<Self, &'static str> {
            if !(2..=3).contains(&args.len())
                || args.iter().any(|arg| arg.is_empty())
            {
                return Err(USAGE);
            }
            let interval_ms = args.get(2).map_or(Ok(33), |value| {
                value.to_str().and_then(|s| s.parse().ok()).ok_or(USAGE)
            })?;
            if !(25..=5000).contains(&interval_ms) {
                return Err("Frame interval must be 25–5000 ms.");
            }
            Ok(Self {
                input: args[0].clone().into(),
                output: args[1].clone().into(),
                interval_ms,
            })
        }
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Replay {
        interval_ms: u64,
        countdown_ms: u64,
        frames: Vec<Frame>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Frame {
        label: String,
        file: String,
        timestamp_ms: u64,
        hint: Option<CropQuad>,
        raw: DocumentDetection,
        stable: DocumentDetection,
        state: CaptureState,
        progress: f64,
    }

    pub(super) fn run() -> Result<(), Box<dyn Error>> {
        let args =
            Arguments::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
        match replay(&args) {
            Ok(report) => {
                println!(
                    "Wrote {} frames to {}.",
                    report.frames.len(),
                    args.output.join("index.html").display()
                );
                Ok(())
            }
            Err(error) => Err(format!(
                "{error} No existing files were replaced. \
                 Any partial diagnostics remain in {}.",
                args.output.display()
            )
            .into()),
        }
    }

    fn inputs(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_file()
                && path.extension().is_some_and(|e| {
                    ["jpg", "jpeg", "png"]
                        .iter()
                        .any(|s| e.eq_ignore_ascii_case(s))
                })
            {
                files.push(path);
                if files.len() > MAX_FRAMES {
                    return Err(
                        "Replay is limited to 300 source frames.".into()
                    );
                }
            }
        }
        files.sort();
        if files.is_empty() {
            return Err("Choose a folder with 1–300 JPEG/PNG frames.".into());
        }
        Ok(files)
    }

    fn replay(args: &Arguments) -> Result<Replay, Box<dyn Error>> {
        if args.output.try_exists()? {
            return Err("Output already exists; choose a new folder.".into());
        }
        let files = inputs(&args.input)?;
        fs::create_dir(&args.output)?;
        let mut report = Replay {
            interval_ms: args.interval_ms,
            countdown_ms: u64::try_from(
                AutoCaptureGate::COUNTDOWN.as_millis(),
            )?,
            frames: Vec::with_capacity(files.len()),
        };
        let mut tracker = DetectionStabilizer::default();
        let mut gate = AutoCaptureGate::default();
        let mut dimensions = None;
        for (index, path) in files.iter().enumerate() {
            eprintln!(
                "Analyzing {}/{}: {}",
                index + 1,
                files.len(),
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            let source = File::open(path)?;
            let limit = u64::try_from(MAX_IMAGE_BYTES)?;
            if source.metadata()?.len() > limit {
                return Err("Source frame exceeds the 100 MiB limit.".into());
            }
            let mut bytes = Vec::new();
            source.take(limit + 1).read_to_end(&mut bytes)?;
            let (info, image) = decode_image(&bytes)?;
            let current = (info.width, info.height);
            if dimensions.is_some_and(|previous| previous != current) {
                tracker.reset();
                gate.reset();
            }
            dimensions = Some(current);
            let hint = tracker.hint();
            let raw = detect_document_with_hint(&image.into_rgba8(), hint)?;
            let stable = tracker.update(&raw);
            let timestamp_ms = u64::try_from(index)? * args.interval_ms;
            let state =
                gate.evaluate(&stable, Duration::from_millis(timestamp_ms));
            let extension = match info.format {
                ImageFormatKind::Jpeg => "jpg",
                ImageFormatKind::Png => "png",
            };
            let file = format!("frame-{index:04}.{extension}");
            write_new(&args.output.join(&file), &bytes)?;
            report.frames.push(Frame {
                label: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                file,
                timestamp_ms,
                hint,
                raw,
                stable,
                state,
                progress: gate.progress(),
            });
        }
        let json = serde_json::to_string_pretty(&report)?;
        write_new(&args.output.join("replay.json"), json.as_bytes())?;
        write_new(&args.output.join("index.html"), page(&json).as_bytes())?;
        Ok(report)
    }

    fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?
            .write_all(bytes)
    }

    fn page(json: &str) -> String {
        // Labels are data, not HTML or executable script, even for local files.
        include_str!("frame_replay/view.html").replacen(
            "__REPLAY_JSON__",
            &json.replace('<', "\\u003c"),
            1,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn arguments_validate_count_empty_paths_and_interval() {
            for args in [
                vec![],
                vec!["in"],
                vec!["", "out"],
                vec!["in", ""],
                vec!["in", "out", ""],
                vec!["in", "out", "no"],
                vec!["in", "out", "0"],
                vec!["in", "out", "24"],
                vec!["in", "out", "5001"],
                vec!["in", "out", "18446744073709551616"],
                vec!["in", "out", "33", "extra"],
            ] {
                let args =
                    args.into_iter().map(OsString::from).collect::<Vec<_>>();
                assert!(Arguments::parse(&args).is_err());
            }
            for interval in ["25", "33", "5000"] {
                assert!(
                    Arguments::parse(&[
                        "in".into(),
                        "out".into(),
                        interval.into()
                    ])
                    .is_ok()
                );
            }
            assert_eq!(
                Arguments::parse(&["in".into(), "out".into()])
                    .unwrap()
                    .interval_ms,
                33
            );
        }

        #[test]
        fn embedded_json_cannot_terminate_its_data_element() {
            let html = page(r#"{"label":"</script><img src=x>"}"#);
            assert!(html.contains(r#""label":"\u003c/script>"#));
            assert!(!html.contains("<img src=x>"));
            assert!(!html.contains("__REPLAY_JSON__"));
        }

        #[test]
        fn replay_preserves_sorted_sources_and_refuses_existing_output() {
            let root = std::env::temp_dir()
                .join(format!("fastfill-replay-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            let output = root.join("report");
            let mut encoded = std::io::Cursor::new(Vec::new());
            image::RgbaImage::from_fn(160, 120, |x, y| {
                if (20..140).contains(&x) && (15..105).contains(&y) {
                    image::Rgba([240, 245, 245, 255])
                } else {
                    image::Rgba([40, 30, 20, 255])
                }
            })
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
            let bytes = encoded.into_inner();
            for name in ["d.png", "b.png", "a.png", "c.png"] {
                fs::write(root.join(name), &bytes).unwrap();
            }
            let args = Arguments {
                input: root.clone(),
                output: output.clone(),
                interval_ms: 1000,
            };
            let report = replay(&args).unwrap();
            assert_eq!(report.frames.len(), 4);
            assert_eq!(report.frames[0].label, "a.png");
            assert_eq!(report.countdown_ms, 1000);
            assert_eq!(report.frames[2].state, CaptureState::Ready);
            assert_eq!(report.frames[3].state, CaptureState::Captured);
            assert_eq!(report.frames[3].timestamp_ms, 3000);
            for frame in report.frames {
                assert_eq!(fs::read(root.join(frame.label)).unwrap(), bytes);
                assert_eq!(fs::read(output.join(frame.file)).unwrap(), bytes);
            }
            let before = fs::read(output.join("index.html")).unwrap();
            assert!(
                replay(&args)
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("already exists")
            );
            assert_eq!(fs::read(output.join("index.html")).unwrap(), before);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn invalid_and_excessive_inputs_never_publish_a_replay() {
            let root = std::env::temp_dir().join(format!(
                "fastfill-replay-invalid-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&root).unwrap();
            let args = Arguments {
                input: root.clone(),
                output: root.join("report"),
                interval_ms: 33,
            };
            assert!(replay(&args).is_err());
            assert!(!args.output.exists());
            fs::write(root.join("bad.png"), b"not an image").unwrap();
            assert!(replay(&args).is_err());
            assert!(!args.output.join("index.html").exists());
            assert_eq!(
                fs::read(root.join("bad.png")).unwrap(),
                b"not an image"
            );
            // A separate new destination keeps the partial report recoverable.
            let output = root.join("second-report");
            for index in 0..MAX_FRAMES {
                fs::write(root.join(format!("{index}.png")), []).unwrap();
            }
            assert!(
                replay(&Arguments {
                    output: output.clone(),
                    ..args
                })
                .is_err()
            );
            assert!(!output.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }
}
