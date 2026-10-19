use fastfill_rust::document_detection::{
    AutoCaptureGate, CaptureState, DetectionStabilizer, detect_document,
    detect_document_with_hint,
};
use fastfill_rust::project::{CropQuad, NormalizedPoint};
use image::{Rgba, RgbaImage};

fn document(width: u32, height: u32, corners: [(f64, f64); 4]) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let inside = (0..4).all(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            (b.0 - a.0) * (f64::from(y) - a.1)
                - (b.1 - a.1) * (f64::from(x) - a.0)
                >= 0.0
        });
        Rgba(if inside {
            [240, 245, 245, 255]
        } else {
            [45, 35, 25, 255]
        })
    })
}

fn distance(first: CropQuad, second: CropQuad) -> f32 {
    let points = |crop: CropQuad| {
        [
            crop.top_left,
            crop.top_right,
            crop.bottom_right,
            crop.bottom_left,
        ]
    };
    points(first)
        .into_iter()
        .zip(points(second))
        .map(|(a, b)| (a.x - b.x).hypot(a.y - b.y))
        .fold(0.0, f32::max)
}

#[test]
fn perspective_document_uses_four_related_sides_and_preserves_source() {
    // Same geometry, colors, and tolerances as the C# synthetic document.
    let mut source = document(
        1000,
        700,
        [
            (145.0, 80.0),
            (870.0, 105.0),
            (820.0, 625.0),
            (110.0, 590.0),
        ],
    );
    for y in 220..228 {
        for x in 250..730 {
            source[(x, y)] = Rgba([60, 60, 60, 255]);
        }
    }
    let before = source.clone();
    let detection = detect_document(&source).unwrap();
    let expected = CropQuad {
        top_left: NormalizedPoint { x: 0.145, y: 0.114 },
        top_right: NormalizedPoint { x: 0.871, y: 0.15 },
        bottom_right: NormalizedPoint { x: 0.821, y: 0.894 },
        bottom_left: NormalizedPoint { x: 0.11, y: 0.844 },
    };
    assert!(detection.confidence >= 0.8, "{detection:?}");
    assert!(detection.area_ratio >= 0.45);
    assert!(distance(detection.corners.unwrap(), expected) < 0.04);
    assert!(detection.sharpness > 0.0);
    assert_eq!(source, before);
}

#[test]
fn white_form_with_internal_rules_is_not_a_visible_document_boundary() {
    let mut source = RgbaImage::from_pixel(800, 600, Rgba([245; 4]));
    for y in [80, 200, 320, 450] {
        for x in 70..730 {
            source[(x, y)] = Rgba([20, 20, 20, 255]);
        }
    }
    for x in [70, 400, 729] {
        for y in 80..451 {
            source[(x, y)] = Rgba([20, 20, 20, 255]);
        }
    }
    let detection = detect_document(&source).unwrap();
    assert!(detection.corners.is_none(), "{detection:?}");
    assert!(detection.confidence < 0.72);
}

#[test]
fn blank_transparent_and_tiny_images_do_not_produce_accepted_quads() {
    for value in [0, 120, 255] {
        let pixels = RgbaImage::from_pixel(240, 180, Rgba([value; 4]));
        let detection = detect_document(&pixels).unwrap();
        assert!(detection.corners.is_none(), "{detection:?}");
        assert_eq!(detection.sharpness, 0.0);
    }
    let transparent = RgbaImage::from_fn(240, 180, |x, y| {
        let value = if (x + y) % 2 == 0 { 0 } else { 255 };
        Rgba([value, 0, 255, 0])
    });
    assert!(detect_document(&transparent).unwrap().corners.is_none());
    assert_eq!(detect_document(&transparent).unwrap().sharpness, 0.0);
    for dimensions in [(1, 1), (3, 100), (100, 3)] {
        let source = RgbaImage::new(dimensions.0, dimensions.1);
        assert!(detect_document(&source).unwrap().corners.is_none());
    }
    assert!(detect_document(&RgbaImage::new(0, 0)).is_err());
}

#[test]
fn resolution_and_alpha_do_not_change_the_document_geometry() {
    let source = document(
        640,
        480,
        [(90.0, 60.0), (560.0, 80.0), (530.0, 430.0), (70.0, 400.0)],
    );
    let first = detect_document(&source).unwrap();
    let resized = image::imageops::resize(
        &source,
        2560,
        1920,
        image::imageops::FilterType::Nearest,
    );
    let second = detect_document(&resized).unwrap();
    assert_eq!((second.analysis_width, second.analysis_height), (1280, 960));
    assert!(distance(first.corners.unwrap(), second.corners.unwrap()) < 0.02);
    let mut translucent = source;
    for pixel in translucent.pixels_mut() {
        pixel[3] = 200;
    }
    let alpha = detect_document(&translucent).unwrap();
    assert!(distance(first.corners.unwrap(), alpha.corners.unwrap()) < 0.02);
}

#[test]
fn subthreshold_candidate_is_not_silently_an_accepted_document() {
    let source = document(
        640,
        480,
        [(0.0, 50.0), (639.0, 50.0), (639.0, 430.0), (0.0, 430.0)],
    );
    let result = detect_document(&source).unwrap();
    assert!(result.corners.is_none());
    assert!(result.candidate_corners.is_some());
    assert!(result.confidence < 0.72);
    let json = serde_json::to_value(&result).unwrap();
    assert!(json["corners"].is_null());
    assert!(json["candidateCorners"].is_object());
    let hinted =
        detect_document_with_hint(&source, result.candidate_corners).unwrap();
    assert!(hinted.corners.is_none());
    assert_eq!(hinted.confidence, result.confidence);
}

#[test]
fn hints_are_advisory_and_invalid_or_distant_hints_are_ignored() {
    let source = document(
        640,
        480,
        [(90.0, 60.0), (560.0, 80.0), (530.0, 430.0), (70.0, 400.0)],
    );
    let baseline = detect_document(&source).unwrap();
    let mut invalid = CropQuad::default();
    invalid.top_left.x = f32::NAN;
    for hint in [
        None,
        Some(invalid),
        Some(CropQuad::default()),
        baseline.corners,
    ] {
        let actual = detect_document_with_hint(&source, hint).unwrap();
        assert_eq!(actual.corners, baseline.corners);
        assert_eq!(actual.confidence, baseline.confidence);
    }
}

fn stroke(image: &mut RgbaImage, a: (f64, f64), b: (f64, f64), width: f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let (x, y) = (f64::from(x) - a.0, f64::from(y) - a.1);
        let amount = ((x * dx + y * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        if (x - dx * amount).hypot(y - dy * amount) <= width / 2.0 {
            *pixel = Rgba([225, 225, 225, 255]);
        }
    }
}

#[test]
fn background_extensions_and_status_banner_do_not_inflate_the_document() {
    let mut source = document(
        1280,
        720,
        [(303.0, 59.0), (923.0, 59.0), (923.0, 678.0), (303.0, 669.0)],
    );
    for (a, b) in [
        ((923.0, 59.0), (1041.0, 59.0)),
        ((1041.0, 59.0), (1220.0, 681.0)),
        ((923.0, 678.0), (1220.0, 681.0)),
    ] {
        stroke(&mut source, a, b, 6.0);
    }
    for y in 570..606 {
        for x in 55..1105 {
            source[(x, y)] = Rgba([75, 70, 65, 255]);
        }
    }
    for x in (230..1000).step_by(20) {
        for y in 578..599 {
            source[(x, y)] = Rgba([230, 230, 230, 255]);
        }
    }
    let detection = detect_document(&source).unwrap();
    let expected = CropQuad {
        top_left: NormalizedPoint {
            x: 303.0 / 1279.0,
            y: 59.0 / 719.0,
        },
        top_right: NormalizedPoint {
            x: 923.0 / 1279.0,
            y: 59.0 / 719.0,
        },
        bottom_right: NormalizedPoint {
            x: 923.0 / 1279.0,
            y: 678.0 / 719.0,
        },
        bottom_left: NormalizedPoint {
            x: 303.0 / 1279.0,
            y: 669.0 / 719.0,
        },
    };
    assert!(detection.corners.is_some(), "{detection:?}");
    assert!(
        distance(detection.corners.unwrap(), expected) < 0.02,
        "{detection:?}"
    );
}

#[test]
fn saturated_document_uses_color_boundary_not_only_white_paper_mask() {
    let mut source = document(
        640,
        480,
        [(90.0, 60.0), (560.0, 80.0), (530.0, 430.0), (70.0, 400.0)],
    );
    let white = detect_document(&source).unwrap().corners.unwrap();
    for pixel in source.pixels_mut() {
        if pixel[0] == 240 {
            *pixel = Rgba([35, 100, 210, 255]);
        }
    }
    let detection = detect_document(&source).unwrap();
    assert!(detection.corners.is_some(), "{detection:?}");
    assert!(distance(detection.corners.unwrap(), white) < 0.02);
}

fn recorded_frames() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    // Only the trusted committed TAR, not a general-purpose archive reader.
    let archive = include_bytes!("../../tests/samples/folded-paper-frames.tar");
    let mut offset = 0;
    std::iter::from_fn(move || {
        if offset + 512 > archive.len() {
            return None;
        }
        let header = &archive[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            return None;
        }
        let name = std::str::from_utf8(&header[..100])
            .unwrap()
            .trim_end_matches('\0');
        let size = usize::from_str_radix(
            std::str::from_utf8(&header[124..136])
                .unwrap()
                .trim_matches(['\0', ' ']),
            8,
        )
        .unwrap();
        offset += 512;
        let bytes = &archive[offset..offset + size];
        offset += size.div_ceil(512) * 512;
        Some((name, bytes))
    })
    .filter(|(name, _)| {
        std::path::Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jpg"))
    })
}

fn recorded_corners() -> CropQuad {
    CropQuad {
        top_left: NormalizedPoint { x: 0.28, y: 0.09 },
        top_right: NormalizedPoint { x: 0.86, y: 0.15 },
        bottom_right: NormalizedPoint { x: 0.86, y: 0.89 },
        bottom_left: NormalizedPoint { x: 0.26, y: 0.89 },
    }
}

#[test]
fn recorded_laptop_frames_find_the_outer_sheet_not_the_fold() {
    let mut checked = 0;
    for (name, bytes) in recorded_frames().filter(|(name, _)| {
        ["0059.jpg", "0112.jpg", "0168.jpg"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
    }) {
        let (_, image) =
            fastfill_rust::image_input::decode_image(bytes).unwrap();
        let single = detect_document(&image.into_rgba8()).unwrap();
        assert!(single.corners.is_some(), "{name}: {single:?}");
        assert!(
            distance(single.corners.unwrap(), recorded_corners()) < 0.1,
            "{name}: {single:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 3);
}

#[test]
#[ignore = "Slow 224-frame replay; run explicitly with --ignored"]
fn recorded_laptop_sequence_finds_the_outer_sheet_and_reaches_capture() {
    // Keep the C# geometry/quality gates intact across the complete recording.
    // The countdown alone uses the user's one-second handheld calibration.
    let expected = recorded_corners();
    let mut checked = 0;
    let mut tracker = DetectionStabilizer::default();
    let mut gate = AutoCaptureGate::default();
    let mut raw_counts = [0; 2];
    let mut stable_counts = [0; 2];
    let mut capture_frame = None;
    for (name, bytes) in recorded_frames() {
        let (_, image) =
            fastfill_rust::image_input::decode_image(bytes).unwrap();
        let raw =
            detect_document_with_hint(&image.into_rgba8(), tracker.hint())
                .unwrap();
        let stable = tracker.update(&raw);
        for (detection, counts) in
            [(&raw, &mut raw_counts), (&stable, &mut stable_counts)]
        {
            if let Some(corners) = detection.corners {
                let correct = distance(corners, expected) < 0.1;
                counts[usize::from(!correct)] += 1;
                if !correct {
                    eprintln!("{name}: {detection:?}");
                }
            }
        }
        let timestamp = std::time::Duration::from_millis(checked * 33);
        if gate.evaluate(&stable, timestamp) == CaptureState::Ready {
            assert!(distance(stable.corners.unwrap(), expected) < 0.1);
            capture_frame.get_or_insert(checked + 1);
        }
        checked += 1;
    }
    eprintln!(
        "Frames {checked}, raw {raw_counts:?}, stable \
        {stable_counts:?}, capture {capture_frame:?}"
    );
    assert_eq!(checked, 224);
    assert!(raw_counts[0] >= 160);
    assert_eq!(raw_counts[1], 0);
    assert!(stable_counts[0] >= 180);
    assert_eq!(stable_counts[1], 0);
    assert!(capture_frame.is_some());
}
