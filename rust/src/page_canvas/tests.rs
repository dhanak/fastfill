use super::clipping::{
    clip_polygon_to_rectangle, clip_segment, clipped_closed_polyline_runs,
    clipped_polyline_runs,
};
use super::render::mark_points;
use fastfill_rust::annotations::{ShapeKind, arrowhead_points};
use fastfill_rust::geometry::Point as PagePoint;
use iced::{Point, Rectangle};

#[test]
fn segment_clipping_handles_crossing_inside_and_outside_lines() {
    let clip =
        Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(30.0, 40.0));
    assert_eq!(
        clip_segment(Point::new(0.0, 30.0), Point::new(50.0, 30.0), clip),
        Some((Point::new(10.0, 30.0), Point::new(40.0, 30.0))),
    );
    assert_eq!(
        clip_segment(Point::new(20.0, 30.0), Point::new(30.0, 40.0), clip),
        Some((Point::new(20.0, 30.0), Point::new(30.0, 40.0))),
    );
    assert_eq!(
        clip_segment(Point::new(0.0, 10.0), Point::new(50.0, 10.0), clip),
        None,
    );
}

#[test]
fn polyline_clipping_keeps_every_run_inside_the_page() {
    let clip =
        Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(30.0, 40.0));
    let points = [
        Point::new(0.0, 30.0),
        Point::new(20.0, 30.0),
        Point::new(50.0, 50.0),
        Point::new(20.0, 70.0),
    ];
    let runs = clipped_polyline_runs(&points, clip);

    assert_eq!(runs.len(), 2);
    assert!(runs.iter().flatten().all(|point| {
        point.x >= clip.x
            && point.x <= clip.x + clip.width
            && point.y >= clip.y
            && point.y <= clip.y + clip.height
    }));
}

#[test]
fn closed_polyline_keeps_its_start_as_a_join_not_two_caps() {
    let clip =
        Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(100.0, 100.0));
    let points = [
        Point::new(10.0, 10.0),
        Point::new(90.0, 10.0),
        Point::new(50.0, 90.0),
    ];
    let runs = clipped_closed_polyline_runs(&points, clip);

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].first(), runs[0].last());
}

#[test]
fn arrowhead_dimensions_do_not_depend_on_direction_or_box_aspect() {
    let end = PagePoint::new(200.0, 200.0);
    let horizontal = arrowhead_points(PagePoint::new(0.0, 200.0), end).unwrap();
    let diagonal_offset = 200.0 / 2.0_f32.sqrt();
    let diagonal = arrowhead_points(
        PagePoint::new(end.x - diagonal_offset, end.y - diagonal_offset),
        end,
    )
    .unwrap();
    for (horizontal, diagonal) in [horizontal.0, horizontal.1]
        .into_iter()
        .zip([diagonal.0, diagonal.1])
    {
        let horizontal = (horizontal.x - end.x).hypot(horizontal.y - end.y);
        let diagonal = (diagonal.x - end.x).hypot(diagonal.y - end.y);
        assert!((horizontal - diagonal).abs() < 0.001);
    }
}

#[test]
fn mark_points_fill_their_selection_bounds() {
    let bounds =
        Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(100.0, 80.0));
    let check = mark_points(ShapeKind::Checkmark, bounds);
    assert_eq!(check[0], Point::new(10.0, 56.0));
    assert_eq!(check[1], Point::new(46.0, 100.0));
    assert_eq!(check[2], Point::new(110.0, 20.0));

    let cross = mark_points(ShapeKind::Cross, bounds);
    assert_eq!(cross[0], Point::new(10.0, 20.0));
    assert_eq!(cross[1], Point::new(110.0, 100.0));
    assert_eq!(cross[2], Point::new(110.0, 20.0));
    assert_eq!(cross[3], Point::new(10.0, 100.0));
}

#[test]
fn filled_polygon_clipping_keeps_every_point_inside_the_page() {
    let clip =
        Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(10.0, 10.0));
    let polygon = clip_polygon_to_rectangle(
        vec![
            Point::new(-5.0, -5.0),
            Point::new(15.0, -5.0),
            Point::new(15.0, 15.0),
            Point::new(-5.0, 15.0),
        ],
        clip,
    );

    assert_eq!(polygon.len(), 4);
    assert!(polygon.into_iter().all(|point| {
        point.x >= clip.x
            && point.x <= clip.x + clip.width
            && point.y >= clip.y
            && point.y <= clip.y + clip.height
    }));
}
