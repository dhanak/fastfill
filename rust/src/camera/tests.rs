use super::*;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use std::ffi::OsString;
use std::io::Cursor;

fn parse_strings(args: &[&str]) -> Result<Command, Box<dyn Error>> {
    parse(&args.iter().map(OsString::from).collect::<Vec<_>>())
}

#[test]
fn arguments_reject_missing_empty_extra_and_invalid_settings() {
    for args in [
        vec![],
        vec![""],
        vec!["list", "extra"],
        vec!["formats"],
        vec!["formats", ""],
        vec!["formats", "-1"],
        vec!["capture", "0"],
        vec!["capture", "0", ""],
        vec!["capture", "0", "out.jpg"],
        vec!["capture", "0", "out.png", "640"],
        vec!["capture", "0", "out.png", "0", "480", "30", "MJPG"],
        vec!["capture", "0", "out.png", "640", "0", "30", "MJPG"],
        vec!["capture", "0", "out.png", "640", "480", "0", "MJPG"],
        vec!["capture", "0", "out.png", "640", "480", "121", "MJPG"],
        vec!["capture", "0", "out.png", "10000", "10000", "30", "MJPG"],
        vec!["capture", "0", "out.png", "640", "480", "30", "H264"],
        vec!["capture", "0", "out.png", "641", "480", "30", "YUYV"],
        vec!["capture", "0", "out.png", "640", "481", "30", "NV12"],
    ] {
        assert!(parse_strings(&args).is_err(), "{args:?}");
    }
}

#[test]
fn arguments_keep_explicit_devices_formats_and_unicode_paths() {
    assert!(matches!(parse_strings(&["list"]).unwrap(), Command::List));
    assert!(matches!(
        parse_strings(&["formats", "2"]).unwrap(),
        Command::Formats(2)
    ));
    let Command::Capture {
        index,
        output,
        spec,
    } = parse_strings(&["capture", "2", "árvíz.PNG"]).unwrap()
    else {
        panic!("capture expected");
    };
    assert_eq!(index, 2);
    assert_eq!(output.to_str(), Some("árvíz.PNG"));
    assert_eq!((spec.width, spec.height, spec.fps), (640, 480, 30));
    assert_eq!(spec.fourcc, *b"MJPG");
    for fourcc in ["MJPG", "YUYV", "NV12", "RGB3", "BGR3", "GREY"] {
        assert!(
            parse_strings(&[
                "capture", "0", "out.png", "640", "480", "15", fourcc,
            ])
            .is_ok()
        );
    }
}

fn spec(width: u32, height: u32, fourcc: [u8; 4]) -> Spec {
    Spec {
        width,
        height,
        fps: 30,
        fourcc,
    }
}

#[test]
fn rgb_and_jpeg_frames_preserve_dimensions_orientation_and_channels() {
    let pixels = [255, 0, 0, 0, 255, 0];
    let rgb = decode(&spec(2, 1, *b"RGB3"), &pixels).unwrap();
    assert_eq!(rgb.dimensions(), (2, 1));
    assert_eq!(rgb[(0, 0)].0, [255, 0, 0, 255]);
    assert_eq!(rgb[(1, 0)].0, [0, 255, 0, 255]);
    let source = DynamicImage::ImageRgb8(RgbImage::from_pixel(
        8,
        4,
        Rgb([40, 120, 220]),
    ));
    let mut jpeg = Cursor::new(Vec::new());
    source.write_to(&mut jpeg, ImageFormat::Jpeg).unwrap();
    let decoded = decode(&spec(8, 4, *b"MJPG"), jpeg.get_ref()).unwrap();
    assert_eq!(decoded.dimensions(), (8, 4));
    assert!(decoded[(0, 0)][2] > decoded[(0, 0)][0] + 100);
    assert!(decode(&spec(4, 8, *b"MJPG"), jpeg.get_ref()).is_err());
}

#[test]
fn malformed_padded_and_oversized_frames_are_rejected() {
    for fourcc in [*b"RGB3", *b"BGR3", *b"GREY", *b"YUYV", *b"NV12"] {
        assert!(decode(&spec(2, 2, fourcc), &[]).is_err());
        assert!(decode(&spec(2, 2, fourcc), &[0; 100]).is_err());
    }
    assert!(decode(&spec(2, 2, *b"MJPG"), b"bad jpeg").is_err());
    assert!(decode(&spec(0, 2, *b"RGB3"), &[]).is_err());
    assert!(decode(&spec(u32::MAX, u32::MAX, *b"RGB3"), &[]).is_err());
    assert!(decode(&spec(3, 2, *b"YUYV"), &[0; 12]).is_err());
    assert!(decode(&spec(2, 3, *b"NV12"), &[0; 9]).is_err());
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(RgbImage::new(2, 2))
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    assert!(decode(&spec(2, 2, *b"MJPG"), png.get_ref()).is_err());
}

#[test]
fn raw_color_formats_keep_neutral_levels_and_channel_order() {
    let bgr = decode(&spec(1, 1, *b"BGR3"), &[12, 34, 56]).unwrap();
    assert_eq!(bgr[(0, 0)].0, [56, 34, 12, 255]);
    let gray = decode(&spec(1, 1, *b"GREY"), &[42]).unwrap();
    assert_eq!(gray[(0, 0)].0, [42, 42, 42, 255]);
    for (fourcc, bytes) in [
        (*b"YUYV", vec![16, 128, 235, 128, 16, 128, 235, 128]),
        (*b"NV12", vec![16, 235, 16, 235, 128, 128]),
    ] {
        let image = decode(&spec(2, 2, fourcc), &bytes).unwrap();
        assert_eq!(image.dimensions(), (2, 2));
        assert!(image[(0, 0)].0[..3].iter().all(|v| *v < 24));
        assert!(image[(1, 0)].0[..3].iter().all(|v| *v > 224));
    }
}

#[test]
fn live_preview_returns_the_first_frame_without_waiting_for_exposure() {
    let reads = std::cell::Cell::new(0);
    let first = first_frame(
        false,
        || {
            reads.set(reads.get() + 1);
            Ok(reads.get())
        },
        || panic!("Live preview must not wait on the exposure clock"),
    )
    .unwrap();
    assert_eq!(first, 1);
    assert_eq!(reads.get(), 1);
}

#[test]
fn exposure_warmup_drains_frames_for_two_seconds_at_different_rates() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct Frame(Rc<Cell<u32>>);
    impl Drop for Frame {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    assert_eq!(EXPOSURE_WARMUP, Duration::from_secs(2));
    for fps in [5, 30, 120] {
        let reads = Cell::new(0);
        let drops = Rc::new(Cell::new(0));
        let saved = first_frame(
            true,
            || {
                // Return every discarded driver buffer before reading again.
                assert_eq!(drops.get(), reads.get());
                reads.set(reads.get() + 1);
                Ok(Frame(Rc::clone(&drops)))
            },
            || Duration::from_secs_f64(f64::from(reads.get()) / f64::from(fps)),
        )
        .unwrap();
        assert_eq!(reads.get(), 2 * fps + 2);
        assert_eq!(drops.get(), reads.get() - 1);
        drop(saved);
        assert_eq!(drops.get(), reads.get());
    }
}

#[test]
fn slow_startup_does_not_count_toward_exposure_warmup() {
    use std::cell::Cell;

    let reads = Cell::new(0_u32);
    let saved = first_frame(
        true,
        || {
            reads.set(reads.get() + 1);
            Ok(reads.get())
        },
        || {
            if reads.get() == 0 {
                Duration::ZERO
            } else {
                // First frame takes 3 s, subsequent frames arrive at 10 FPS.
                Duration::from_millis(3000 + u64::from(reads.get() - 1) * 100)
            }
        },
    )
    .unwrap();
    assert_eq!(saved, 22);
}

#[test]
fn warmup_propagates_startup_stream_and_final_frame_errors() {
    use std::cell::Cell;

    for fail_at in [1, 4, 202] {
        let reads = Cell::new(0_u32);
        let result = first_frame(
            true,
            || {
                reads.set(reads.get() + 1);
                if reads.get() == fail_at {
                    Err("synthetic read failure".into())
                } else {
                    Ok(())
                }
            },
            || Duration::from_millis(u64::from(reads.get()) * 10),
        );
        assert_eq!(result.unwrap_err().to_string(), "synthetic read failure");
        assert_eq!(reads.get(), fail_at);
    }
}

#[test]
fn worker_waits_only_in_timeout_fixture() {
    match std::env::var("FASTFILL_CAMERA_PROBE_TEST_WORKER").as_deref() {
        Ok("wait") => std::thread::sleep(Duration::from_secs(10)),
        Ok("fail") => panic!("intentional child failure"),
        _ => (),
    }
}

#[test]
fn supervision_handles_success_failure_and_stalled_workers() {
    for mode in ["success", "fail", "wait"] {
        let mut command =
            std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "camera::tests::worker_waits_only_in_timeout_fixture",
            ])
            .env("FASTFILL_CAMERA_PROBE_TEST_WORKER", mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let result = supervise(
            &mut command,
            if mode == "wait" {
                Duration::from_millis(50)
            } else {
                Duration::from_secs(5)
            },
        );
        match mode {
            "success" => assert!(result.is_ok()),
            "fail" => assert!(
                result.unwrap_err().to_string().contains("Worker exited")
            ),
            _ => assert!(result.unwrap_err().to_string().contains("timed out")),
        }
    }
}

#[test]
fn png_output_never_replaces_existing_files() {
    let output = std::env::temp_dir()
        .join(format!("fastfill-camera-{}.png", uuid::Uuid::new_v4()));
    let pixels = image::RgbaImage::from_pixel(2, 1, image::Rgba([42; 4]));
    write_png(&output, &pixels).unwrap();
    let before = std::fs::read(&output).unwrap();
    assert!(write_png(&output, &pixels).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), before);
    assert_eq!(image::open(&output).unwrap().into_rgba8(), pixels);
    std::fs::remove_file(output).unwrap();
}
