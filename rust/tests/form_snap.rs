use fastfill_rust::form_snap::FormFeatures;
use fastfill_rust::image_input::decode_image;
use fastfill_rust::project::NormalizedPoint;
use image::{Rgba, RgbaImage};

fn point(x: f32, y: f32, width: f32, height: f32) -> NormalizedPoint {
    NormalizedPoint {
        x: x / (width - 1.0),
        y: y / (height - 1.0),
    }
}

fn ink(image: &mut RgbaImage, left: u32, top: u32, width: u32, height: u32) {
    for y in top..top + height {
        for x in left..left + width {
            image.put_pixel(x, y, Rgba([20, 20, 20, 255]));
        }
    }
}

fn synthetic_form() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(800, 600, Rgba([245; 4]));
    ink(&mut image, 120, 100, 44, 4);
    ink(&mut image, 120, 140, 44, 4);
    ink(&mut image, 120, 100, 4, 44);
    ink(&mut image, 160, 100, 4, 44);
    ink(&mut image, 240, 359, 401, 3);
    for x in (100..380).step_by(16) {
        ink(&mut image, x, 450, 12, 2);
    }
    ink(&mut image, 520, 450, 181, 2);
    for x in (130..430).step_by(6) {
        ink(&mut image, x, 519, 2, 2);
    }
    for x in (560..602).step_by(6) {
        ink(&mut image, x, 559, 2, 2);
    }
    image
}

#[test]
fn finds_small_checkbox_and_solid_rule_with_legacy_distance_units() {
    let features = FormFeatures::analyze(&synthetic_form()).unwrap();
    let bounds = features
        .find_box(point(142.0, 122.0, 800.0, 600.0), 0.04)
        .unwrap();
    assert!((bounds.width * 800.0 - 44.0).abs() < 4.0);
    assert!((bounds.height * 600.0 - 44.0).abs() < 4.0);
    assert_eq!(features.boxes().len(), 1, "deduplicate inner/outer edges");
    let line = features
        .find_horizontal_line(point(440.0, 355.0, 800.0, 600.0), 0.04)
        .unwrap();
    assert!(line.start.x < 0.31 && line.end.x > 0.79);
    assert!(
        features
            .find_box(point(400.0, 200.0, 800.0, 600.0), 0.01)
            .is_none()
    );
}

#[test]
fn broken_rules_keep_separate_fields_and_short_dotted_runs() {
    let features = FormFeatures::analyze(&synthetic_form()).unwrap();
    let dashed = features
        .find_horizontal_line(point(250.0, 450.0, 800.0, 600.0), 0.04)
        .unwrap();
    assert!(dashed.start.x < 0.16 && (0.44..0.51).contains(&dashed.end.x));
    let separate = features
        .find_horizontal_line(point(620.0, 450.0, 800.0, 600.0), 0.04)
        .unwrap();
    assert!(separate.start.x > 0.62 && separate.end.x > 0.85);
    let short = features
        .find_horizontal_line(point(580.0, 560.0, 800.0, 600.0), 0.02)
        .unwrap();
    assert!((0.69..0.72).contains(&short.start.x));
    assert!((0.73..0.77).contains(&short.end.x));
}

#[test]
fn sample_form_finds_checkbox_inside_border_and_part_b_dots() {
    let (_, image) =
        decode_image(include_bytes!("../../tests/samples/sample-form.png"))
            .unwrap();
    let features = FormFeatures::analyze(&image.into_rgba8()).unwrap();
    let bounds = features
        .find_box(point(113.0, 654.0, 1241.0, 1754.0), 0.015)
        .unwrap();
    assert!((20.0..40.0).contains(&(bounds.width * 1241.0)));
    assert!((20.0..40.0).contains(&(bounds.height * 1754.0)));
    let dotted = features
        .find_horizontal_line(point(980.0, 718.0, 1241.0, 1754.0), 0.02)
        .unwrap();
    assert!((0.64..0.7).contains(&dotted.start.x));
    assert!(dotted.end.x > 0.88);
    assert_eq!(
        features.boxes().len(),
        14,
        "reject enclosed letter counters"
    );
}

#[test]
fn datum_label_stays_outside_dotted_field_and_short_fields_survive() {
    let (_, image) =
        decode_image(include_bytes!("../../tests/samples/datum-form.png"))
            .unwrap();
    let features = FormFeatures::analyze(&image.into_rgba8()).unwrap();
    assert_eq!(
        features.boxes().len(),
        5,
        "only the five printed checkboxes"
    );
    let date = features
        .find_horizontal_line(point(280.0, 1247.0, 1241.0, 1755.0), 0.025)
        .unwrap();
    assert!(date.start.x * 1240.0 > 170.0, "exclude Dátum label");
    assert!(date.start.x * 1240.0 < 200.0);
    assert!(date.end.x * 1240.0 > 440.0);
    for (x, left, right) in [
        (680.0, 640.0, 710.0),
        (780.0, 735.0, 817.0),
        (910.0, 880.0, 933.0),
    ] {
        let line = features
            .find_horizontal_line(point(x, 691.0, 1241.0, 1755.0), 0.012)
            .unwrap();
        assert!((line.start.x * 1240.0 - left).abs() < 16.0);
        assert!((line.end.x * 1240.0 - right).abs() < 16.0);
    }
    assert!(
        features
            .find_horizontal_line(point(620.0, 635.0, 1241.0, 1755.0), 0.004)
            .is_none(),
        "ordinary text must not become a rule"
    );
}

#[test]
fn transparent_ink_is_paper_and_tiny_images_have_no_features() {
    for (width, height) in [(1, 1), (15, 100), (100, 15), (100, 100)] {
        let image = RgbaImage::from_pixel(width, height, Rgba([0; 4]));
        let features = FormFeatures::analyze(&image).unwrap();
        assert_eq!(features.boxes(), []);
        assert_eq!(features.horizontal_lines(), []);
    }
    assert!(FormFeatures::analyze(&RgbaImage::new(0, 0)).is_err());
}

#[test]
fn resizing_composites_alpha_before_sampling_to_avoid_false_ink_edges() {
    let mut source = RgbaImage::from_pixel(2000, 1000, Rgba([0; 4]));
    for y in 201..601 {
        for x in 501..1301 {
            source.put_pixel(x, y, Rgba([255; 4]));
        }
    }
    let features = FormFeatures::analyze(&source).unwrap();
    assert_eq!(features.boxes(), []);
    assert_eq!(features.horizontal_lines(), []);
}

#[test]
fn invalid_queries_never_snap() {
    let features = FormFeatures::analyze(&synthetic_form()).unwrap();
    for (point, distance) in [
        (
            NormalizedPoint {
                x: f32::NAN,
                y: 0.5,
            },
            0.1,
        ),
        (
            NormalizedPoint {
                x: 0.5,
                y: f32::INFINITY,
            },
            0.1,
        ),
        (NormalizedPoint { x: -1.0, y: 0.5 }, 0.1),
        (NormalizedPoint { x: 0.5, y: 0.5 }, -0.1),
        (NormalizedPoint { x: 0.5, y: 0.5 }, f32::NAN),
        (NormalizedPoint { x: 0.5, y: 0.5 }, f32::INFINITY),
    ] {
        assert!(features.find_box(point, distance).is_none());
        assert!(features.find_horizontal_line(point, distance).is_none());
    }
}

#[test]
fn detection_stays_normalized_after_image_resizing() {
    let source = synthetic_form();
    let base = FormFeatures::analyze(&source).unwrap();
    let location = point(142.0, 122.0, 800.0, 600.0);
    let original = base.find_box(location, 0.04).unwrap();
    for factor in [2, 3] {
        let larger = image::imageops::resize(
            &source,
            800 * factor,
            600 * factor,
            image::imageops::FilterType::Nearest,
        );
        let features = FormFeatures::analyze(&larger).unwrap();
        let (width, height) = features.analysis_size();
        assert!(width <= 1600 && height <= 1600);
        let bounds = features.find_box(location, 0.04).unwrap();
        assert!((bounds.x - original.x).abs() < 0.005);
        assert!((bounds.width - original.width).abs() < 0.005);
        let line = features
            .find_horizontal_line(point(250.0, 450.0, 800.0, 600.0), 0.04)
            .unwrap();
        assert!(line.start.x < 0.16 && (0.44..0.51).contains(&line.end.x));
    }
}

#[test]
fn boxes_attached_to_large_borders_keep_their_inside_contour() {
    let mut source = synthetic_form();
    ink(&mut source, 40, 40, 720, 3);
    ink(&mut source, 40, 40, 3, 530);
    ink(&mut source, 40, 567, 720, 3);
    ink(&mut source, 757, 40, 3, 530);
    ink(&mut source, 40, 100, 84, 3);
    let features = FormFeatures::analyze(&source).unwrap();
    let bounds = features
        .find_box(point(142.0, 122.0, 800.0, 600.0), 0.01)
        .unwrap();
    assert!((35.0..48.0).contains(&(bounds.width * 800.0)));
    assert!((35.0..48.0).contains(&(bounds.height * 600.0)));
}

#[test]
fn candidates_on_last_image_row_stay_inside_normalized_bounds() {
    let mut source = RgbaImage::from_pixel(400, 200, Rgba([245; 4]));
    // Three-pixel gaps fit the calibrated bridge at this analysis width.
    for x in (50..100).step_by(5) {
        ink(&mut source, x, 199, 2, 1);
    }
    let features = FormFeatures::analyze(&source).unwrap();
    let line = features
        .find_horizontal_line(point(75.0, 199.0, 400.0, 200.0), 0.01)
        .unwrap();
    for line in features.horizontal_lines().iter().chain([&line]) {
        assert!((0.0..=1.0).contains(&line.start.x));
        assert!((0.0..=1.0).contains(&line.end.x));
        assert!((0.0..=1.0).contains(&line.start.y));
        assert!((0.0..=1.0).contains(&line.end.y));
    }
}

fn stacked_table(stroke: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(800, 600, Rgba([255; 4]));
    for y in [100, 200, 300, 400] {
        ink(&mut image, 100, y, 600 + stroke, stroke);
    }
    for x in [100, 300, 500, 700] {
        ink(&mut image, x, 100, stroke, 300 + stroke);
    }
    image
}

#[test]
fn table_dividers_split_stacked_rows_without_losing_full_rules() {
    for (stroke, factor) in [(1, 1), (1, 2), (1, 3), (3, 1), (3, 2), (3, 3)] {
        let source = stacked_table(stroke);
        let image = image::imageops::resize(
            &source,
            800 * factor,
            600 * factor,
            image::imageops::FilterType::Nearest,
        );
        let features = FormFeatures::analyze(&image).unwrap();
        for y in [100.0, 200.0, 300.0, 400.0] {
            for (x, left, right) in [
                (200.0, 100.0, 300.0),
                (400.0, 300.0, 500.0),
                (600.0, 500.0, 700.0),
            ] {
                let line = features
                    .find_horizontal_line(point(x, y - 5.0, 800.0, 600.0), 0.02)
                    .unwrap();
                assert!(
                    (line.start.x * 799.0 - left).abs() < 4.0
                        && (line.end.x * 799.0 - right).abs() < 4.0,
                    "expected cell {left}..{right}, row {y}, scale {factor}: \
                     {line:?}"
                );
            }
            assert!(
                features.horizontal_lines().iter().any(|line| {
                    line.start.x * 799.0 < 105.0
                        && line.end.x * 799.0 > 695.0
                        && (line.start.y * 599.0 - y).abs() < 4.0
                }),
                "retain the unsplit table rule"
            );
        }
    }
}

#[test]
fn table_t_junctions_split_only_the_rows_they_touch() {
    let mut source = stacked_table(3);
    // A merged lower row: its dividers end at the preceding row border.
    for y in 303..400 {
        for x in [300, 500] {
            for dx in 0..3 {
                source.put_pixel(x + dx, y, Rgba([255; 4]));
            }
        }
    }
    let features = FormFeatures::analyze(&source).unwrap();
    let cell = features
        .find_horizontal_line(point(400.0, 295.0, 800.0, 600.0), 0.02)
        .unwrap();
    assert!((cell.start.x * 799.0 - 300.0).abs() < 4.0);
    assert!((cell.end.x * 799.0 - 500.0).abs() < 4.0);
    let merged = features
        .find_horizontal_line(point(400.0, 395.0, 800.0, 600.0), 0.02)
        .unwrap();
    assert!(merged.start.x * 799.0 < 105.0);
    assert!(merged.end.x * 799.0 > 695.0);
}

#[test]
fn printed_stems_touching_one_rule_do_not_split_a_field() {
    let mut source = RgbaImage::from_pixel(800, 600, Rgba([255; 4]));
    ink(&mut source, 100, 200, 603, 3);
    ink(&mut source, 100, 400, 603, 3);
    for x in [150, 300, 450, 600] {
        ink(&mut source, x, 170, 3, 33);
    }
    let features = FormFeatures::analyze(&source).unwrap();
    assert_eq!(features.horizontal_lines().len(), 2);
    let line = features
        .find_horizontal_line(point(400.0, 195.0, 800.0, 600.0), 0.02)
        .unwrap();
    assert!(line.start.x * 799.0 < 105.0);
    assert!(line.end.x * 799.0 > 695.0);
}
