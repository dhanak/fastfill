//! Marquees intersect selectable geometry, not an object's outer bounds.

use super::geometry::{
    arrowhead_points, contains, distance, point_in_polygon,
    point_segment_distance, rotate_point,
};
use super::{AnnotationGeometry, LineKind, RESIZE_CORNERS, ShapeAnnotation};
use crate::geometry::{Point, Rect};

pub(super) fn hits_shape(bounds: Rect, annotation: &ShapeAnnotation) -> bool {
    match &annotation.geometry {
        AnnotationGeometry::Bounded { .. } => annotation
            .frame_corners()
            .is_some_and(|corners| hits_frame(bounds, &corners)),
        AnnotationGeometry::Linear { kind, start, end } => {
            let stroke = |start, end| {
                hits_stroke(bounds, start, end, annotation.stroke_width / 2.0)
            };
            stroke(*start, *end)
                || *kind == LineKind::Arrow
                    && arrowhead_points(*start, *end).is_some_and(
                        |(left, right)| {
                            stroke(left, *end) || stroke(*end, right)
                        },
                    )
        }
        AnnotationGeometry::Ink { points, .. } => {
            let center = annotation.bounds().center();
            let points = points
                .iter()
                .map(|point| rotate_point(*point, center, annotation.rotation))
                .collect::<Vec<_>>();
            let stroke = |start, end| {
                hits_stroke(bounds, start, end, annotation.stroke_width / 2.0)
            };
            points.windows(2).any(|pair| stroke(pair[0], pair[1]))
                || annotation.filled
                    && (hits_frame(bounds, &points)
                        || points
                            .last()
                            .zip(points.first())
                            .is_some_and(|(last, first)| stroke(*last, *first)))
        }
    }
}

pub(super) fn hits_frame(bounds: Rect, polygon: &[Point]) -> bool {
    polygon.iter().any(|point| contains(bounds, *point))
        || point_in_polygon(bounds.center(), polygon)
        || polygon
            .windows(2)
            .any(|pair| hits_segment(bounds, pair[0], pair[1]))
        || polygon
            .last()
            .zip(polygon.first())
            .is_some_and(|(last, first)| hits_segment(bounds, *last, *first))
}

fn hits_stroke(bounds: Rect, start: Point, end: Point, radius: f32) -> bool {
    // Round endpoint hit areas are intentional, as in point picking.
    hits_segment(bounds, start, end)
        || RESIZE_CORNERS.into_iter().any(|corner| {
            point_segment_distance(corner.position(bounds), start, end)
                <= radius
        })
        || [start, end].into_iter().any(|point| {
            let nearest = Point::new(
                point.x.clamp(bounds.x, bounds.x + bounds.width),
                point.y.clamp(bounds.y, bounds.y + bounds.height),
            );
            distance(point, nearest) <= radius
        })
}

fn hits_segment(bounds: Rect, start: Point, end: Point) -> bool {
    if start.x.max(end.x) < bounds.x
        || start.x.min(end.x) > bounds.x + bounds.width
        || start.y.max(end.y) < bounds.y
        || start.y.min(end.y) > bounds.y + bounds.height
    {
        return false;
    }
    // Separating axes: rectangle X/Y above, then the segment's normal.
    let (low, high) = RESIZE_CORNERS
        .map(|corner| {
            let point = corner.position(bounds);
            (end.x - start.x) * (point.y - start.y)
                - (end.y - start.y) * (point.x - start.x)
        })
        .into_iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), side| {
            (low.min(side), high.max(side))
        });
    low <= 0.0 && high >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filled_polygon_overlap_covers_containment_and_crossing_edges() {
        let triangle = [
            Point::new(10.0, 10.0),
            Point::new(110.0, 10.0),
            Point::new(60.0, 110.0),
        ];
        assert!(hits_frame(Rect::new(50.0, 40.0, 20.0, 20.0), &triangle));
        assert!(hits_frame(Rect::new(0.0, 0.0, 120.0, 120.0), &triangle));
        assert!(hits_frame(Rect::new(0.0, 50.0, 40.0, 5.0), &triangle));
        assert!(!hits_frame(Rect::new(0.0, 90.0, 10.0, 10.0), &triangle));
    }
}
