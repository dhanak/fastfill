use fastfill_rust::correction::{PageCorrection, PageMapping};
use fastfill_rust::geometry::{Point, Size};
use fastfill_rust::project::{CropQuad, NormalizedPoint};

#[test]
fn filter_changes_preserve_positions_and_sizes_exactly() {
    let old = PageCorrection::default();
    let new = PageCorrection {
        brightness: 25,
        ..old
    };
    let map = PageMapping::new(120, 180, old, new).unwrap();
    let point = Point::new(-32.1234, 920.0001);
    assert_eq!(map.placement(point), Some((point, Size::new(1.0, 1.0))));
}

#[test]
fn crop_changes_keep_image_anchors_and_scale_each_axis_independently() {
    let old = PageCorrection::default();
    let new = PageCorrection {
        crop: CropQuad {
            top_left: NormalizedPoint { x: 0.25, y: 0.25 },
            top_right: NormalizedPoint { x: 0.75, y: 0.25 },
            bottom_right: NormalizedPoint { x: 0.75, y: 0.75 },
            bottom_left: NormalizedPoint { x: 0.25, y: 0.75 },
        },
        ..old
    };
    let map = PageMapping::new(100, 200, old, new).unwrap();
    let (point, scale) = map.placement(Point::new(306.0, 612.0)).unwrap();
    assert!((point.x - 306.0).abs() < 0.001);
    assert!((point.y - 606.0).abs() < 0.001);
    assert!((scale.width - 2.0).abs() < 0.001);
    assert!((scale.height - 101.0 / 51.0).abs() < 0.001);
    let (off_page, _) = map.placement(Point::new(0.0, 0.0)).unwrap();
    assert!(
        off_page.x < 0.0 && off_page.y < 0.0,
        "mapping must not clamp an image anchor to a crop edge"
    );
}

#[test]
fn rotation_moves_centers_and_scales_without_changing_object_angles() {
    let old = PageCorrection::default();
    let new = PageCorrection {
        rotation_quarter_turns: 1,
        ..old
    };
    let map = PageMapping::new(1000, 500, old, new).unwrap();
    let (point, scale) = map.placement(Point::new(100.0, 50.0)).unwrap();
    assert!((point.x - 512.0).abs() < 0.001);
    assert!((point.y - 200.0).abs() < 0.001);
    assert!((scale.width - 2.0).abs() < 0.001);
    assert!((scale.height - 2.0).abs() < 0.001);
    let reverse = PageMapping::new(1000, 500, new, old).unwrap();
    let (back, _) = reverse.placement(point).unwrap();
    assert!((back.x - 100.0).abs() < 0.001);
    assert!((back.y - 50.0).abs() < 0.001);
}

#[test]
fn perspective_mapping_reaches_the_new_corner_and_accepts_extreme_turn_counts()
{
    let old = PageCorrection::default();
    let new = PageCorrection {
        crop: CropQuad {
            top_left: NormalizedPoint { x: 0.1, y: 0.1 },
            top_right: NormalizedPoint { x: 0.9, y: 0.2 },
            bottom_right: NormalizedPoint { x: 0.8, y: 0.9 },
            bottom_left: NormalizedPoint { x: 0.2, y: 0.8 },
        },
        ..old
    };
    let mapping = PageMapping::new(1000, 500, old, new).unwrap();
    let (corner, scale) = mapping.placement(Point::new(61.2, 30.6)).unwrap();
    assert!(corner.x.abs() < 0.001 && corner.y.abs() < 0.001);
    assert!(scale.width > 0.0 && scale.height > 0.0);
    let extreme = PageCorrection {
        rotation_quarter_turns: i32::MIN,
        ..old
    };
    let mapping = PageMapping::new(1000, 500, extreme, new).unwrap();
    assert!(mapping.placement(Point::new(100.0, 100.0)).is_some());
    assert!(mapping.placement(Point::new(f32::NAN, 100.0)).is_none());
}

#[test]
fn perspective_scale_round_trip_does_not_accumulate_size_drift() {
    let old = PageCorrection::default();
    let new = PageCorrection {
        crop: CropQuad {
            top_left: NormalizedPoint { x: 0.1, y: 0.1 },
            top_right: NormalizedPoint { x: 0.9, y: 0.2 },
            bottom_right: NormalizedPoint { x: 0.8, y: 0.9 },
            bottom_left: NormalizedPoint { x: 0.2, y: 0.8 },
        },
        ..old
    };
    let forward = PageMapping::new(1000, 500, old, new).unwrap();
    let reverse = PageMapping::new(1000, 500, new, old).unwrap();
    let original = Point::new(200.0, 120.0);
    let (point, scale) = forward.placement(original).unwrap();
    let (restored, inverse) = reverse.placement(point).unwrap();
    assert!((restored.x - original.x).abs() < 0.001);
    assert!((restored.y - original.y).abs() < 0.001);
    assert!((scale.width * inverse.width - 1.0).abs() < 0.001);
    assert!((scale.height * inverse.height - 1.0).abs() < 0.001);
}
