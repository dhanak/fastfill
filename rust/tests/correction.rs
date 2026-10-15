use fastfill_rust::correction::{
    CorrectionError, PageCorrection, correct_image, corrected_dimensions,
};
use fastfill_rust::image_input::decode_image;
use fastfill_rust::project::{CropQuad, NormalizedPoint};
use image::{Rgba, RgbaImage};

fn crop(points: [(f32, f32); 4]) -> CropQuad {
    let points = points.map(|(x, y)| NormalizedPoint { x, y });
    CropQuad {
        top_left: points[0],
        top_right: points[1],
        bottom_right: points[2],
        bottom_left: points[3],
    }
}

fn grid(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        Rgba([
            u8::try_from(x * 20).unwrap(),
            u8::try_from(y * 30).unwrap(),
            100,
            255,
        ])
    })
}

#[test]
fn full_frame_is_byte_exact_without_off_by_one_or_source_mutation() {
    let mut source = grid(7, 5);
    source[(0, 0)] = Rgba([91, 82, 73, 0]);
    source[(2, 2)] = Rgba([62, 51, 40, 127]);
    let original = source.clone();
    let correction = PageCorrection::default();
    assert_eq!(corrected_dimensions(7, 5, correction), Ok((7, 5)));
    assert_eq!(correct_image(&source, correction).unwrap(), source);
    assert_eq!(source, original);
}

#[test]
fn rotations_match_image_ops_including_negative_and_large_turn_counts() {
    let source = grid(7, 5);
    for turns in [-101_i32, -4, -3, -2, -1, 0, 1, 2, 3, 4, 101, i32::MIN] {
        let correction = PageCorrection {
            rotation_quarter_turns: turns,
            ..PageCorrection::default()
        };
        let expected = match turns.rem_euclid(4) {
            0 => source.clone(),
            1 => image::imageops::rotate90(&source),
            2 => image::imageops::rotate180(&source),
            _ => image::imageops::rotate270(&source),
        };
        assert_eq!(correct_image(&source, correction).unwrap(), expected);
        assert_eq!(
            corrected_dimensions(7, 5, correction),
            Ok(expected.dimensions())
        );
    }
}

#[test]
fn axis_aligned_crop_preserves_each_endpoint_and_interior_sample() {
    let source = grid(9, 7);
    let correction = PageCorrection {
        crop: crop([(0.25, 0.0), (0.75, 0.0), (0.75, 1.0), (0.25, 1.0)]),
        ..PageCorrection::default()
    };
    let expected = image::imageops::crop_imm(&source, 2, 0, 5, 7).to_image();
    let corrected = correct_image(&source, correction).unwrap();
    assert_eq!(corrected, expected);
    let rotated = PageCorrection {
        rotation_quarter_turns: 1,
        ..correction
    };
    assert_eq!(corrected_dimensions(9, 7, rotated), Ok((7, 5)));
    assert_eq!(
        correct_image(&source, rotated).unwrap(),
        image::imageops::rotate90(&expected)
    );
}

#[test]
fn perspective_uses_all_four_related_corners_not_linear_corner_blending() {
    let source = grid(11, 9);
    let correction = PageCorrection {
        crop: crop([(0.0, 0.0), (1.0, 0.0), (0.8, 1.0), (0.2, 1.0)]),
        ..PageCorrection::default()
    };
    let result = correct_image(&source, correction).unwrap();
    assert_eq!(result.dimensions(), (11, 9));
    for ((x, y), expected) in [
        ((0, 0), source[(0, 0)]),
        ((10, 0), source[(10, 0)]),
        ((10, 8), source[(8, 8)]),
        ((0, 8), source[(2, 8)]),
        // Projective midpoint samples y=5, not bilinear-quad y=4.
        ((5, 4), source[(5, 5)]),
    ] {
        for (actual, expected) in result[(x, y)].0.into_iter().zip(expected.0) {
            assert!(actual.abs_diff(expected) <= 1);
        }
    }
}

#[test]
fn fractional_sampling_avoids_dark_fringes_around_transparency() {
    let source = RgbaImage::from_fn(3, 3, |x, _| {
        if x == 0 {
            Rgba([0, 0, 0, 0])
        } else {
            Rgba([200, 50, 20, 255])
        }
    });
    let correction = PageCorrection {
        crop: crop([(0.25, 0.0), (1.0, 0.0), (1.0, 1.0), (0.25, 1.0)]),
        ..PageCorrection::default()
    };
    let result = correct_image(&source, correction).unwrap();
    assert_eq!(result[(0, 0)], Rgba([200, 50, 20, 128]));
}

#[test]
fn reversed_winding_remains_supported_as_in_existing_project_validation() {
    let source = grid(7, 5);
    let correction = PageCorrection {
        crop: crop([(1.0, 0.0), (0.0, 0.0), (0.0, 1.0), (1.0, 1.0)]),
        ..PageCorrection::default()
    };
    assert_eq!(
        correct_image(&source, correction).unwrap(),
        image::imageops::flip_horizontal(&source)
    );
}

#[test]
fn tiny_images_and_single_pixel_axes_do_not_panic_or_drop_pixels() {
    for (width, height) in [(1, 1), (1, 7), (7, 1), (2, 2)] {
        let source = grid(width, height);
        for turns in 0..4 {
            let correction = PageCorrection {
                rotation_quarter_turns: turns,
                ..PageCorrection::default()
            };
            let output = correct_image(&source, correction).unwrap();
            assert_eq!(
                u64::from(output.width()) * u64::from(output.height()),
                u64::from(width) * u64::from(height)
            );
        }
    }
}

#[test]
fn single_pixel_crop_axes_sample_the_center_without_division_by_zero() {
    let source = grid(1, 5);
    let correction = PageCorrection {
        crop: crop([(0.0, 0.25), (1.0, 0.25), (1.0, 0.75), (0.0, 0.75)]),
        ..PageCorrection::default()
    };
    let output = correct_image(&source, correction).unwrap();
    assert_eq!(output.dimensions(), (1, 3));
    for y in 0..3 {
        assert_eq!(output[(0, y)], source[(0, y + 1)]);
    }
    let source = grid(3, 3);
    let correction = PageCorrection {
        crop: crop([(0.4, 0.4), (0.6, 0.4), (0.6, 0.6), (0.4, 0.6)]),
        ..PageCorrection::default()
    };
    let output = correct_image(&source, correction).unwrap();
    assert_eq!(output.dimensions(), (1, 1));
    assert_eq!(output[(0, 0)], source[(1, 1)]);
}

#[test]
fn skewed_crop_uses_each_corner_even_when_no_edges_are_axis_aligned() {
    let source = grid(11, 9);
    let correction = PageCorrection {
        crop: crop([(0.1, 0.125), (0.9, 0.25), (0.8, 0.875), (0.2, 0.75)]),
        ..PageCorrection::default()
    };
    let output = correct_image(&source, correction).unwrap();
    assert_eq!(output.dimensions(), (9, 6));
    for (actual, expected) in [
        ((0, 0), (1, 1)),
        ((8, 0), (9, 2)),
        ((8, 5), (8, 7)),
        ((0, 5), (2, 6)),
    ] {
        assert_eq!(output[actual], source[expected]);
    }
}

#[test]
fn ill_conditioned_projection_is_rejected_instead_of_emitting_bad_samples() {
    let correction = PageCorrection {
        crop: crop([(0.0, 0.0), (1e-15, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        ..PageCorrection::default()
    };
    assert_eq!(
        corrected_dimensions(11, 9, correction),
        Err(CorrectionError::InvalidCrop)
    );
}

#[test]
fn json_settings_use_the_existing_crop_schema_and_round_trip() {
    let correction: PageCorrection =
        serde_json::from_str(include_str!("../examples/correction.json"))
            .unwrap();
    assert_eq!(correction.rotation_quarter_turns, 1);
    assert_eq!(
        correction.crop.top_left,
        NormalizedPoint { x: 0.04, y: 0.04 }
    );
    assert_eq!(
        serde_json::from_str::<PageCorrection>(
            &serde_json::to_string(&correction).unwrap()
        )
        .unwrap(),
        correction
    );
    assert_eq!(
        serde_json::from_str::<PageCorrection>("{}").unwrap(),
        PageCorrection::default()
    );
}

#[test]
fn malformed_crops_are_rejected_before_processing() {
    for invalid in [
        crop([(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)]),
        crop([(0.0, 0.0), (1.0, 0.0), (0.5, 0.2), (0.0, 1.0)]),
        crop([(0.0, 0.0), (1.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
        crop([(0.0, 0.0), (1.1, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        crop([(f32::NAN, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        crop([(0.0, 0.0), (f32::INFINITY, 0.0), (1.0, 1.0), (0.0, 1.0)]),
    ] {
        let correction = PageCorrection {
            crop: invalid,
            ..PageCorrection::default()
        };
        assert_eq!(
            corrected_dimensions(7, 5, correction),
            Err(CorrectionError::InvalidCrop)
        );
        assert_eq!(
            correct_image(&grid(7, 5), correction),
            Err(CorrectionError::InvalidCrop)
        );
    }
}

#[test]
fn dimension_limits_are_checked_without_allocating_images() {
    let correction = PageCorrection::default();
    for (width, height) in
        [(0, 5), (5, 0), (10_000, 10_000), (u32::MAX, u32::MAX)]
    {
        assert_eq!(
            corrected_dimensions(width, height, correction),
            Err(CorrectionError::InvalidDimensions)
        );
    }
    assert_eq!(
        corrected_dimensions(10_000, 5_000, correction),
        Ok((10_000, 5_000))
    );
    let slanted = PageCorrection {
        crop: crop([(0.0, 0.0), (1.0, 0.2), (0.9, 1.0), (0.1, 0.8)]),
        ..correction
    };
    assert_eq!(
        corrected_dimensions(50_000_000, 1, slanted),
        Err(CorrectionError::OutputTooLarge)
    );
}

#[test]
fn sample_form_identity_and_crop_stay_sharp_and_keep_original_size() {
    let (_, decoded) =
        decode_image(include_bytes!("../../tests/samples/sample-form.png"))
            .unwrap();
    let source = decoded.into_rgba8();
    let original = correct_image(&source, PageCorrection::default()).unwrap();
    assert_eq!(original, source);
    let width = source.width() - 1;
    let height = source.height() - 1;
    #[allow(clippy::cast_precision_loss)]
    let crop = CropQuad {
        top_left: NormalizedPoint {
            x: 40.0 / width as f32,
            y: 60.0 / height as f32,
        },
        top_right: NormalizedPoint {
            x: 1.0,
            y: 60.0 / height as f32,
        },
        bottom_left: NormalizedPoint {
            x: 40.0 / width as f32,
            y: 1.0,
        },
        ..CropQuad::default()
    };
    let correction = PageCorrection {
        crop,
        ..PageCorrection::default()
    };
    let output = correct_image(&source, correction).unwrap();
    let expected = image::imageops::crop_imm(
        &source,
        40,
        60,
        source.width() - 40,
        source.height() - 60,
    )
    .to_image();
    assert_eq!(output, expected);
    assert_eq!(source, original);
}
