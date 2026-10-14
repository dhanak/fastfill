use super::{
    AnnotationGeometry, LineKind, Pointer, RESIZE_CORNERS,
    ROTATION_HANDLE_OFFSET, ShapeAnnotation,
};
use crate::geometry::{Point, Rect, Size};

impl ShapeAnnotation {
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match &self.geometry {
            AnnotationGeometry::Bounded { bounds, .. } => *bounds,
            AnnotationGeometry::Linear { kind, start, end } => {
                linear_bounds(*kind, *start, *end, self.stroke_width)
            }
            AnnotationGeometry::Ink { points, .. } => {
                ink_bounds(points, self.stroke_width)
            }
        }
    }

    #[must_use]
    pub fn supports_rotation(&self) -> bool {
        !matches!(self.geometry, AnnotationGeometry::Linear { .. })
    }

    #[must_use]
    pub fn frame_corners(&self) -> Option<[Point; 4]> {
        self.supports_rotation().then(|| {
            let bounds = self.bounds();
            RESIZE_CORNERS
                .map(|corner| self.rotated_point(corner.position(bounds)))
        })
    }

    #[must_use]
    pub fn rotation_handle(&self, scale: f32) -> Option<Point> {
        if !self.supports_rotation() || !valid_scale(scale) {
            return None;
        }
        let bounds = self.bounds();
        let center = bounds.center();
        Some(self.rotated_point(Point::new(
            center.x,
            bounds.y - ROTATION_HANDLE_OFFSET / scale,
        )))
    }

    #[must_use]
    pub fn rotated_point(&self, point: Point) -> Point {
        rotate_point(point, self.bounds().center(), self.rotation)
    }

    pub(super) fn unrotated_point(&self, point: Point) -> Point {
        rotate_point(point, self.bounds().center(), -self.rotation)
    }

    pub(super) fn visible_bounds(&self) -> Rect {
        self.frame_corners()
            .map_or_else(|| self.bounds(), |corners| point_bounds(&corners))
    }

    pub(super) fn translated(&self, dx: f32, dy: f32) -> Self {
        let geometry = match &self.geometry {
            AnnotationGeometry::Bounded { kind, bounds } => {
                AnnotationGeometry::Bounded {
                    kind: *kind,
                    bounds: Rect::new(
                        bounds.x + dx,
                        bounds.y + dy,
                        bounds.width,
                        bounds.height,
                    ),
                }
            }
            AnnotationGeometry::Linear { kind, start, end } => {
                AnnotationGeometry::Linear {
                    kind: *kind,
                    start: Point::new(start.x + dx, start.y + dy),
                    end: Point::new(end.x + dx, end.y + dy),
                }
            }
            AnnotationGeometry::Ink { kind, points } => {
                AnnotationGeometry::Ink {
                    kind: *kind,
                    points: points
                        .iter()
                        .map(|point| Point::new(point.x + dx, point.y + dy))
                        .collect(),
                }
            }
        };
        Self {
            geometry,
            stroke_width: self.stroke_width,
            color: self.color,
            filled: self.filled,
            rotation: self.rotation,
        }
    }
}

pub(super) fn valid(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

pub(super) fn valid_scale(scale: f32) -> bool {
    scale.is_finite() && scale > 0.0
}

pub(super) fn contains(bounds: Rect, point: Point) -> bool {
    point.x >= bounds.x
        && point.x <= bounds.x + bounds.width
        && point.y >= bounds.y
        && point.y <= bounds.y + bounds.height
}

pub(super) fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

pub(super) fn point_segment_distance(
    point: Point,
    start: Point,
    end: Point,
) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return distance(point, start);
    }
    let projection =
        ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    distance(
        point,
        Point::new(start.x + projection * dx, start.y + projection * dy),
    )
}

pub(super) fn line_hit_radius(pointer: Pointer) -> f32 {
    match pointer {
        Pointer::Mouse => 7.0,
        Pointer::Touch(_) => 18.0,
    }
}

pub(super) fn annotation_contains(
    annotation: &ShapeAnnotation,
    point: Point,
    line_tolerance: f32,
) -> bool {
    let point = if annotation.supports_rotation() {
        annotation.unrotated_point(point)
    } else {
        point
    };
    match &annotation.geometry {
        AnnotationGeometry::Bounded { bounds, .. } => contains(*bounds, point),
        AnnotationGeometry::Linear { kind, start, end } => {
            let shaft = point_segment_distance(point, *start, *end);
            if shaft <= line_tolerance {
                return true;
            }
            *kind == LineKind::Arrow
                && arrowhead_points(*start, *end).is_some_and(
                    |(left, right)| {
                        point_segment_distance(point, *end, left)
                            <= line_tolerance
                            || point_segment_distance(point, *end, right)
                                <= line_tolerance
                    },
                )
        }
        AnnotationGeometry::Ink { points, .. } => {
            (annotation.filled && point_in_polygon(point, points))
                || points.windows(2).any(|segment| {
                    point_segment_distance(point, segment[0], segment[1])
                        <= line_tolerance + annotation.stroke_width / 2.0
                })
        }
    }
}

pub(super) fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    let Some(mut previous) = polygon.last().copied() else {
        return false;
    };
    let mut inside = false;
    for current in polygon {
        let crosses = (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y)
                    / (previous.y - current.y)
                    + current.x;
        if crosses {
            inside = !inside;
        }
        previous = *current;
    }
    inside
}

pub(super) fn valid_annotation_size(annotation: &ShapeAnnotation) -> bool {
    match &annotation.geometry {
        AnnotationGeometry::Bounded { bounds, .. } => {
            bounds.width >= 1.0 && bounds.height >= 1.0
        }
        AnnotationGeometry::Linear { start, end, .. } => {
            distance(*start, *end) >= 1.0
        }
        AnnotationGeometry::Ink { points, .. } => points
            .windows(2)
            .any(|segment| distance(segment[0], segment[1]) >= 1.0),
    }
}

pub(super) fn constrain_line(start: Point, end: Point, enabled: bool) -> Point {
    if !enabled {
        return end;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return end;
    }
    let step = std::f32::consts::FRAC_PI_4;
    let angle = (dy.atan2(dx) / step).round() * step;
    Point::new(
        start.x + length * angle.cos(),
        start.y + length * angle.sin(),
    )
}

pub(super) fn angle_from(center: Point, point: Point) -> f32 {
    (point.y - center.y).atan2(point.x - center.x)
}

pub(super) fn normalized_rotation(rotation: f32) -> f32 {
    let rotation = (rotation + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    if rotation.abs() <= f32::EPSILON {
        0.0
    } else {
        rotation
    }
}

pub(super) fn rotate_point(
    point: Point,
    center: Point,
    rotation: f32,
) -> Point {
    if rotation.abs() <= f32::EPSILON {
        return point;
    }
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    let sin = rotation.sin();
    let cos = rotation.cos();
    Point::new(
        center.x + dx * cos - dy * sin,
        center.y + dx * sin + dy * cos,
    )
}

#[must_use]
pub fn arrowhead_points(start: Point, end: Point) -> Option<(Point, Point)> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return None;
    }
    // Keep arrowhead geometry independent from line thickness.
    let head = 24.0_f32.min(length * 0.8);
    let ux = dx / length;
    let uy = dy / length;
    let base = Point::new(end.x - ux * head, end.y - uy * head);
    let half_width = head * 0.45;
    let offset = Point::new(-uy * half_width, ux * half_width);
    Some((
        Point::new(base.x + offset.x, base.y + offset.y),
        Point::new(base.x - offset.x, base.y - offset.y),
    ))
}

pub(super) fn linear_bounds(
    kind: LineKind,
    start: Point,
    end: Point,
    stroke_width: f32,
) -> Rect {
    let mut bounds = segment_stroke_bounds(start, end, stroke_width);
    if kind == LineKind::Arrow
        && let Some((first, second)) = arrowhead_points(start, end)
    {
        for wing in [first, second] {
            bounds = union_bounds(
                bounds,
                segment_stroke_bounds(end, wing, stroke_width),
            );
        }
        let half = stroke_width / 2.0;
        bounds = union_bounds(
            bounds,
            Rect::new(end.x - half, end.y - half, stroke_width, stroke_width),
        );
    }
    bounds
}

pub(super) fn segment_stroke_bounds(
    start: Point,
    end: Point,
    stroke_width: f32,
) -> Rect {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return Rect::new(start.x, start.y, 0.0, 0.0);
    }
    let half = stroke_width / 2.0;
    let x_padding = half * (dy / length).abs();
    let y_padding = half * (dx / length).abs();
    Rect::new(
        start.x.min(end.x) - x_padding,
        start.y.min(end.y) - y_padding,
        (end.x - start.x).abs() + x_padding * 2.0,
        (end.y - start.y).abs() + y_padding * 2.0,
    )
}

pub(super) fn point_bounds(points: &[Point]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x;
    let mut bottom = first.y;
    for point in &points[1..] {
        left = left.min(point.x);
        top = top.min(point.y);
        right = right.max(point.x);
        bottom = bottom.max(point.y);
    }
    Rect::new(left, top, right - left, bottom - top)
}

pub(super) fn ink_bounds(points: &[Point], stroke_width: f32) -> Rect {
    let bounds = point_bounds(points);
    let half = stroke_width / 2.0;
    Rect::new(
        bounds.x - half,
        bounds.y - half,
        bounds.width + stroke_width,
        bounds.height + stroke_width,
    )
}

pub(super) fn resize_ink_points(
    points: &[Point],
    target: Rect,
    stroke_width: f32,
) -> Vec<Point> {
    let source = point_bounds(points);
    let half = stroke_width / 2.0;
    let width = (target.width - stroke_width).max(1.0);
    let height = (target.height - stroke_width).max(1.0);
    let left = target.x + half;
    let top = target.y + half;
    points
        .iter()
        .map(|point| {
            let x = if source.width <= f32::EPSILON {
                left + width / 2.0
            } else {
                left + (point.x - source.x) / source.width * width
            };
            let y = if source.height <= f32::EPSILON {
                top + height / 2.0
            } else {
                top + (point.y - source.y) / source.height * height
            };
            Point::new(x, y)
        })
        .collect()
}

pub(super) fn union_bounds(first: Rect, second: Rect) -> Rect {
    let left = first.x.min(second.x);
    let top = first.y.min(second.y);
    let right = (first.x + first.width).max(second.x + second.width);
    let bottom = (first.y + first.height).max(second.y + second.height);
    Rect::new(left, top, right - left, bottom - top)
}

pub(super) fn drag_bounds(start: Point, end: Point, square: bool) -> Rect {
    let mut dx = end.x - start.x;
    let mut dy = end.y - start.y;
    if square {
        let side = dx.abs().max(dy.abs());
        dx = side.copysign(dx);
        dy = side.copysign(dy);
    }
    Rect::new(
        start.x.min(start.x + dx),
        start.y.min(start.y + dy),
        dx.abs(),
        dy.abs(),
    )
}

pub(super) fn mark_bounds(center: Point, end: Point, page: Size) -> Rect {
    let default_half = page.width / 64.0;
    let half = (end.x - center.x)
        .abs()
        .max((end.y - center.y).abs())
        .max(default_half);
    Rect::new(center.x - half, center.y - half, half * 2.0, half * 2.0)
}

pub(super) fn keep_reachable(mut bounds: Rect, page: Size) -> Rect {
    // Clamp continuously; correcting only after disappearance causes a jump.
    let visible_x = 1.0_f32.min(bounds.width).min(page.width);
    let visible_y = 1.0_f32.min(bounds.height).min(page.height);
    bounds.x = bounds
        .x
        .clamp(visible_x - bounds.width, page.width - visible_x);
    bounds.y = bounds
        .y
        .clamp(visible_y - bounds.height, page.height - visible_y);
    bounds
}

pub(super) fn keep_annotation_reachable(
    annotation: &ShapeAnnotation,
    page: Size,
) -> ShapeAnnotation {
    let bounds = annotation.visible_bounds();
    let kept = keep_reachable(bounds, page);
    annotation.translated(kept.x - bounds.x, kept.y - bounds.y)
}
