use fastfill_rust::form_snap::CornerFeatures;
use fastfill_rust::project::NormalizedPoint;
use image::{Rgba, RgbaImage};

fn rectangle() -> RgbaImage {
    let mut pixels = RgbaImage::from_pixel(320, 240, Rgba([35, 40, 45, 255]));
    for y in 50..190 {
        for x in 60..260 {
            pixels.put_pixel(x, y, Rgba([240, 235, 230, 255]));
        }
    }
    pixels
}

#[test]
fn finds_related_rectangle_corners_and_not_straight_edges() {
    let source = rectangle();
    let features = CornerFeatures::analyze(&source).unwrap();
    for (x, y) in [(60.0, 50.0), (259.0, 50.0), (259.0, 189.0), (60.0, 189.0)] {
        let hit = features
            .find_corner(
                NormalizedPoint {
                    x: x / 319.0,
                    y: y / 239.0,
                },
                0.03,
            )
            .unwrap();
        assert!((hit.x * 319.0 - x).abs() <= 3.0);
        assert!((hit.y * 239.0 - y).abs() <= 3.0);
    }
    assert!(
        features
            .find_corner(
                NormalizedPoint {
                    x: 160.0 / 319.0,
                    y: 50.0 / 239.0,
                },
                0.03
            )
            .is_none()
    );
    assert_eq!(source, rectangle());
}

#[test]
fn skewed_document_vertices_remain_local_corner_targets() {
    let vertices = [(80, 40), (250, 70), (230, 210), (40, 180)];
    let source = RgbaImage::from_fn(320, 240, |x, y| {
        let (x, y) = (i64::from(x), i64::from(y));
        let inside = (0..4).all(|i| {
            let (a, b) = (vertices[i], vertices[(i + 1) % 4]);
            i64::from(b.0 - a.0) * (y - i64::from(a.1))
                - i64::from(b.1 - a.1) * (x - i64::from(a.0))
                >= 0
        });
        let value = if inside { 240 } else { 40 };
        Rgba([value, value, value, 255])
    });
    let features = CornerFeatures::analyze(&source).unwrap();
    for (x, y) in vertices {
        #[allow(clippy::cast_precision_loss)]
        let (x, y) = (x as f32, y as f32);
        let corner = features
            .find_corner(
                NormalizedPoint {
                    x: x / 319.0,
                    y: y / 239.0,
                },
                0.03,
            )
            .unwrap();
        assert!((corner.x * 319.0 - x).abs() <= 4.0);
        assert!((corner.y * 239.0 - y).abs() <= 4.0);
    }
}

#[test]
fn transparent_hidden_rgb_blank_ramps_and_small_images_have_no_corners() {
    for (width, height) in [(1, 1), (15, 100), (100, 15), (80, 80)] {
        let pixels = RgbaImage::from_fn(width, height, |x, y| {
            Rgba([u8::try_from(x).unwrap(), u8::try_from(y).unwrap(), 255, 0])
        });
        assert_eq!(CornerFeatures::analyze(&pixels).unwrap().corners(), []);
        let pixels = RgbaImage::from_pixel(width, height, Rgba([127; 4]));
        assert_eq!(CornerFeatures::analyze(&pixels).unwrap().corners(), []);
    }
    let ramp = RgbaImage::from_fn(100, 80, |x, _| {
        let value = u8::try_from(x * 2).unwrap();
        Rgba([value, value, value, 255])
    });
    assert_eq!(CornerFeatures::analyze(&ramp).unwrap().corners(), []);
    assert!(CornerFeatures::analyze(&RgbaImage::new(0, 0)).is_err());
}

#[test]
fn corner_queries_validate_input_and_use_the_images_shorter_axis() {
    let features = CornerFeatures::analyze(&rectangle()).unwrap();
    let target = features.corners()[0];
    let query = NormalizedPoint {
        x: target.x + 0.04,
        y: target.y,
    };
    assert!(features.find_corner(query, 0.03).is_none());
    assert_eq!(features.find_corner(query, 0.06), Some(target));
    for (point, radius) in [
        (
            NormalizedPoint {
                x: f32::NAN,
                y: 0.2,
            },
            0.1,
        ),
        (
            NormalizedPoint {
                x: 0.2,
                y: f32::INFINITY,
            },
            0.1,
        ),
        (NormalizedPoint { x: -0.1, y: 0.2 }, 0.1),
        (target, -1.0),
        (target, f32::NAN),
        (target, f32::INFINITY),
    ] {
        assert!(features.find_corner(point, radius).is_none());
    }
}

#[test]
fn features_are_bounded_spaced_and_normalized_after_resize() {
    let source = rectangle();
    let query = NormalizedPoint { x: 0.19, y: 0.21 };
    let first = CornerFeatures::analyze(&source)
        .unwrap()
        .find_corner(query, 0.03)
        .unwrap();
    let large = image::imageops::resize(
        &source,
        3200,
        2400,
        image::imageops::FilterType::Nearest,
    );
    let features = CornerFeatures::analyze(&large).unwrap();
    assert_eq!(features.analysis_size(), (1600, 1200));
    let hit = features.find_corner(query, 0.03).unwrap();
    assert!((hit.x - first.x).abs() < 0.01);
    assert!((hit.y - first.y).abs() < 0.01);
    let (_, image) = fastfill_rust::image_input::decode_image(include_bytes!(
        "../../tests/samples/sample-form.png"
    ))
    .unwrap();
    let features = CornerFeatures::analyze(&image.into_rgba8()).unwrap();
    assert!(features.corners().len() > 20);
    assert!(features.corners().len() <= 300);
    let (width, height) = features.analysis_size();
    for (index, a) in features.corners().iter().enumerate() {
        for b in &features.corners()[index + 1..] {
            #[allow(clippy::cast_precision_loss)]
            let distance = ((a.x - b.x) * (width - 1) as f32)
                .hypot((a.y - b.y) * (height - 1) as f32);
            assert!(distance >= 7.99);
        }
    }
}
