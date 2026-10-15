use fastfill_rust::correction::{
    CorrectionError, PageCorrection, correct_image, corrected_dimensions,
};
use fastfill_rust::image_input::decode_image;
use fastfill_rust::project::{
    CropQuad, NormalizedPoint, PageFilterMode, PageFilterSettings,
};
use image::{Rgba, RgbaImage};

fn filtered(
    source: &RgbaImage,
    mode: PageFilterMode,
    contrast: i32,
) -> RgbaImage {
    correct_image(
        source,
        PageCorrection {
            filter: PageFilterSettings { mode, contrast },
            ..PageCorrection::default()
        },
    )
    .unwrap()
}

#[test]
fn original_filter_is_exact_and_every_filter_leaves_source_and_alpha_untouched()
{
    let source = RgbaImage::from_fn(35, 33, |x, y| {
        Rgba([
            u8::try_from(x * 7).unwrap(),
            u8::try_from(y * 7).unwrap(),
            80,
            [0, 64, 128, 255][usize::try_from(x % 4).unwrap()],
        ])
    });
    let before = source.clone();
    assert_eq!(filtered(&source, PageFilterMode::Original, 0), source);
    for mode in [
        PageFilterMode::Original,
        PageFilterMode::EnhancedColor,
        PageFilterMode::Grayscale,
        PageFilterMode::BlackAndWhite,
    ] {
        let output = filtered(&source, mode, 15);
        assert_eq!(output.dimensions(), source.dimensions());
        for (a, b) in output.pixels().zip(source.pixels()) {
            assert_eq!(a[3], b[3]);
            if b[3] == 0 {
                assert_eq!(a, b);
            }
        }
        assert_eq!(source, before);
    }
}

#[test]
fn grayscale_matches_bt601_and_keeps_alpha() {
    let source = RgbaImage::from_fn(4, 1, |x, _| {
        [
            Rgba([255, 0, 0, 255]),
            Rgba([0, 255, 0, 128]),
            Rgba([0, 0, 255, 64]),
            Rgba([190, 190, 190, 255]),
        ][usize::try_from(x).unwrap()]
    });
    let output = filtered(&source, PageFilterMode::Grayscale, 0);
    for ((pixel, original), expected) in
        output.pixels().zip(source.pixels()).zip([76, 150, 29, 190])
    {
        assert_eq!(pixel.0, [expected, expected, expected, original[3]]);
    }
}

#[test]
fn contrast_uses_legacy_linear_gain_centered_at_128_not_squared_gain() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([64, 128, 192, 127]));
    assert_eq!(
        filtered(&source, PageFilterMode::Original, 100)[(0, 0)],
        Rgba([0, 128, 255, 127])
    );
    assert_eq!(
        filtered(&source, PageFilterMode::Original, -50)[(0, 0)],
        Rgba([96, 128, 160, 127])
    );
    assert_eq!(
        filtered(&source, PageFilterMode::Original, -100)[(0, 0)],
        Rgba([128, 128, 128, 127])
    );
}

#[test]
fn brightness_defaults_to_zero_and_roundtrips_preview_settings() {
    let legacy: PageCorrection =
        serde_json::from_str(r#"{"filter":{"mode":0,"contrast":0}}"#).unwrap();
    let source = RgbaImage::from_pixel(1, 1, Rgba([20, 100, 240, 127]));
    assert_eq!(correct_image(&source, legacy).unwrap(), source);
    let adjusted: PageCorrection = serde_json::from_str(
        r#"{"brightness":20,"filter":{"mode":0,"contrast":0}}"#,
    )
    .unwrap();
    assert_eq!(
        correct_image(&source, adjusted).unwrap()[(0, 0)],
        Rgba([71, 151, 255, 127])
    );
    assert_eq!(
        serde_json::from_str::<PageCorrection>(
            &serde_json::to_string(&adjusted).unwrap()
        )
        .unwrap(),
        adjusted
    );
}

#[test]
fn brightness_offsets_rgb_and_clamps_at_both_range_limits() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([20, 100, 240, 127]));
    for (brightness, expected) in [
        (-100, [0, 0, 0, 127]),
        (-20, [0, 49, 189, 127]),
        (0, [20, 100, 240, 127]),
        (20, [71, 151, 255, 127]),
        (100, [255, 255, 255, 127]),
    ] {
        let result = correct_image(
            &source,
            PageCorrection {
                brightness,
                ..PageCorrection::default()
            },
        )
        .unwrap();
        assert_eq!(result[(0, 0)].0, expected);
    }
}

#[test]
fn brightness_runs_after_every_mode_preserving_source_alpha_and_hidden_rgb() {
    let source = RgbaImage::from_fn(35, 33, |x, y| {
        Rgba([
            u8::try_from(x * 7).unwrap(),
            u8::try_from(y * 7).unwrap(),
            80,
            [0, 64, 128, 255][usize::try_from(x % 4).unwrap()],
        ])
    });
    let before = source.clone();
    for mode in [
        PageFilterMode::Original,
        PageFilterMode::Grayscale,
        PageFilterMode::EnhancedColor,
        PageFilterMode::BlackAndWhite,
    ] {
        let reference = filtered(&source, mode, 0);
        let output = correct_image(
            &source,
            PageCorrection {
                brightness: 20,
                filter: PageFilterSettings {
                    mode,
                    ..PageFilterSettings::default()
                },
                ..PageCorrection::default()
            },
        )
        .unwrap();
        assert_eq!(output.dimensions(), source.dimensions());
        for (result, mut expected) in
            output.pixels().zip(reference.pixels().copied())
        {
            if expected[3] != 0 {
                for channel in &mut expected.0[..3] {
                    *channel = channel.saturating_add(51);
                }
            }
            assert_eq!(*result, expected);
        }
        assert_eq!(source, before);
    }
}

#[test]
fn brightness_and_contrast_share_one_final_clamp() {
    for (brightness, input, expected) in [
        (60, [0, 64, 255, 127], [25, 153, 255, 127]),
        (-20, [0, 192, 255, 127], [0, 205, 255, 127]),
    ] {
        let source = RgbaImage::from_pixel(1, 1, Rgba(input));
        let output = correct_image(
            &source,
            PageCorrection {
                brightness,
                filter: PageFilterSettings {
                    contrast: 100,
                    ..PageFilterSettings::default()
                },
                ..PageCorrection::default()
            },
        )
        .unwrap();
        assert_eq!(output[(0, 0)].0, expected);
    }
}

#[test]
fn invalid_brightness_is_rejected_by_both_entry_points_and_json_is_strict() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([255; 4]));
    for brightness in [i32::MIN, -101, 101, i32::MAX] {
        let correction = PageCorrection {
            brightness,
            ..PageCorrection::default()
        };
        assert_eq!(
            corrected_dimensions(1, 1, correction),
            Err(CorrectionError::InvalidFilter)
        );
        assert_eq!(
            correct_image(&source, correction),
            Err(CorrectionError::InvalidFilter)
        );
    }
    for invalid in ["null", "1.5", "\"10\"", "2147483648"] {
        assert!(
            serde_json::from_str::<PageCorrection>(&format!(
                "{{\"brightness\":{invalid}}}"
            ))
            .is_err()
        );
    }
}

#[test]
fn out_of_range_contrast_is_rejected_by_both_entry_points() {
    let source = RgbaImage::from_pixel(1, 1, Rgba([255; 4]));
    for contrast in [i32::MIN, -101, 101, i32::MAX] {
        let correction = PageCorrection {
            filter: PageFilterSettings {
                contrast,
                ..PageFilterSettings::default()
            },
            ..PageCorrection::default()
        };
        assert_eq!(
            corrected_dimensions(1, 1, correction),
            Err(CorrectionError::InvalidFilter)
        );
        assert_eq!(
            correct_image(&source, correction),
            Err(CorrectionError::InvalidFilter)
        );
    }
}

#[test]
fn bw_uses_local_light_and_keeps_single_pixel_rules_dots_and_faint_strokes() {
    let mut source = RgbaImage::from_fn(128, 96, |x, _| {
        let level = u8::try_from(100 + x).unwrap();
        Rgba([level, level, level, 255])
    });
    for x in 8..120 {
        source[(x, 30)] = Rgba([0, 0, 0, 255]);
        let level = source[(x, 60)][0] - 40;
        source[(x, 60)] = Rgba([level, level, level, 255]);
    }
    for x in (8..120).step_by(4) {
        source[(x, 75)] = Rgba([25, 25, 25, 255]);
    }
    let output = filtered(&source, PageFilterMode::BlackAndWhite, 0);
    for x in 8..120 {
        assert_eq!(output[(x, 30)][0], 0);
        assert_eq!(output[(x, 60)][0], 0);
        assert_eq!(output[(x, 10)][0], 255);
        assert_eq!(output[(x, 90)][0], 255);
    }
    for x in (8..120).step_by(4) {
        assert_eq!(output[(x, 75)][0], 0);
    }
    assert!(
        output
            .pixels()
            .all(|pixel| pixel.0 == [0, 0, 0, 255] || pixel.0 == [255; 4])
    );
}

#[test]
fn contrast_is_applied_after_bw_classification() {
    let mut source = RgbaImage::from_pixel(40, 40, Rgba([220, 220, 220, 255]));
    source[(20, 20)] = Rgba([30, 30, 30, 255]);
    let output = filtered(&source, PageFilterMode::BlackAndWhite, -50);
    assert_eq!(output[(20, 20)], Rgba([64, 64, 64, 255]));
    assert_eq!(output[(0, 0)], Rgba([192, 192, 192, 255]));
}

#[test]
fn enhanced_color_removes_a_uniform_cast_without_blowing_flat_tiles_white() {
    let source = RgbaImage::from_pixel(64, 64, Rgba([200, 160, 120, 255]));
    let output = filtered(&source, PageFilterMode::EnhancedColor, 0);
    for pixel in output.pixels() {
        for channel in &pixel.0[..3] {
            assert!(channel.abs_diff(160) <= 2);
        }
    }
    // Hidden colored pixels must not change the white-balance statistics.
    let mut with_transparency = source.clone();
    for x in 0..32 {
        for y in 0..64 {
            with_transparency[(x, y)] = Rgba([0, 0, 255, 0]);
        }
    }
    let output = filtered(&with_transparency, PageFilterMode::EnhancedColor, 0);
    assert_eq!(output[(0, 0)], with_transparency[(0, 0)]);
    for channel in &output[(50, 50)].0[..3] {
        assert!(channel.abs_diff(160) <= 2);
    }
}

#[test]
fn enhanced_color_keeps_colored_marks_distinct_from_paper_and_each_other() {
    let mut source =
        RgbaImage::from_pixel(128, 128, Rgba([225, 225, 225, 255]));
    for y in 40..60 {
        for x in 20..40 {
            source[(x, y)] = Rgba([180, 35, 35, 255]);
            source[(x + 60, y)] = Rgba([35, 65, 180, 255]);
        }
    }
    let output = filtered(&source, PageFilterMode::EnhancedColor, 0);
    let red = output[(30, 50)];
    let blue = output[(90, 50)];
    assert!(red[0] > red[1] + 30 && red[0] > red[2] + 30);
    assert!(blue[2] > blue[0] + 30 && blue[2] > blue[1] + 30);
    assert!(output[(5, 5)][0] > red[1]);
}

#[test]
fn filters_accept_tiny_images_without_panicking_or_changing_dimensions() {
    for (width, height) in [(1, 1), (1, 17), (17, 1), (2, 2), (7, 5)] {
        let source =
            RgbaImage::from_pixel(width, height, Rgba([90, 110, 130, 128]));
        for mode in [
            PageFilterMode::Original,
            PageFilterMode::Grayscale,
            PageFilterMode::EnhancedColor,
            PageFilterMode::BlackAndWhite,
        ] {
            let output = filtered(&source, mode, 0);
            assert_eq!(output.dimensions(), source.dimensions());
            assert!(output.pixels().all(|pixel| pixel[3] == 128));
        }
    }
}

#[test]
fn filters_run_after_crop_rotation_and_use_the_existing_numeric_schema() {
    let source = RgbaImage::from_fn(30, 40, |x, _| {
        if x < 10 {
            Rgba([80, 80, 220, 255])
        } else {
            Rgba([180, 150, 120, 255])
        }
    });
    let mut correction = PageCorrection {
        crop: CropQuad {
            top_left: NormalizedPoint { x: 0.5, y: 0.0 },
            bottom_left: NormalizedPoint { x: 0.5, y: 1.0 },
            ..CropQuad::default()
        },
        rotation_quarter_turns: 1,
        ..PageCorrection::default()
    };
    let geometric = correct_image(&source, correction).unwrap();
    correction.filter = PageFilterSettings {
        mode: PageFilterMode::EnhancedColor,
        contrast: 10,
    };
    assert_eq!(
        correct_image(&source, correction).unwrap(),
        filtered(&geometric, PageFilterMode::EnhancedColor, 10)
    );
    let json = r#"{"filter":{"mode":3,"contrast":-50}}"#;
    let parsed: PageCorrection = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.filter.mode, PageFilterMode::BlackAndWhite);
    assert_eq!(parsed.filter.contrast, -50);
    assert!(
        serde_json::from_str::<PageCorrection>(r#"{"filter":{"mode":4}}"#)
            .is_err()
    );
}

#[test]
fn sample_form_bw_keeps_printed_ink_and_suppresses_flat_paper() {
    let (_, decoded) =
        decode_image(include_bytes!("../../tests/samples/sample-form.png"))
            .unwrap();
    let source = decoded.into_rgba8();
    let output = filtered(&source, PageFilterMode::BlackAndWhite, 0);
    let (dark, retained) = source
        .pixels()
        .zip(output.pixels())
        .filter(|(original, _)| original[0] < 80)
        .fold((0, 0), |(dark, retained), (_, result)| {
            (dark + 1, retained + usize::from(result[0] == 0))
        });
    assert!(retained * 100 > dark * 95);
    let paper = source
        .pixels()
        .zip(output.pixels())
        .filter(|(original, _)| original.0[..3] == [255; 3]);
    assert!(paper.clone().all(|(_, result)| result.0 == [255; 4]));
}
