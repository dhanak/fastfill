//! Convert normalized C# transforms to rigid page-point geometry.

use super::{AffineTransform, NormalizedPoint, NormalizedRect};
use crate::geometry::{Point, Rect, Size};

pub(super) fn point(point: NormalizedPoint, page: Size) -> Point {
    Point::new(point.x * page.width, point.y * page.height)
}

pub(super) fn normalized(point: Point, page: Size) -> NormalizedPoint {
    NormalizedPoint {
        x: point.x / page.width,
        y: point.y / page.height,
    }
}

pub(super) fn rect(bounds: NormalizedRect, page: Size) -> Rect {
    Rect::new(
        bounds.x * page.width,
        bounds.y * page.height,
        bounds.width * page.width,
        bounds.height * page.height,
    )
}

pub(super) fn normalized_rect(bounds: Rect, page: Size) -> NormalizedRect {
    NormalizedRect {
        x: bounds.x / page.width,
        y: bounds.y / page.height,
        width: bounds.width / page.width,
        height: bounds.height / page.height,
    }
}

pub(super) fn rotation(transform: AffineTransform, page: Size) -> Option<f32> {
    let cosine = transform.m11;
    let sine = transform.m12 * page.height / page.width;
    let cross = transform.m21 * page.width / page.height;
    let tolerance = 0.0001;
    ((cosine - transform.m22).abs() < tolerance
        && (sine + cross).abs() < tolerance
        && (cosine * cosine + sine * sine - 1.0).abs() < tolerance)
        .then(|| sine.atan2(cosine))
}

pub(super) fn translation(
    center: Point,
    transform: AffineTransform,
    page: Size,
) -> Point {
    let moved = point(transform.apply(normalized(center, page)), page);
    Point::new(moved.x - center.x, moved.y - center.y)
}

pub(super) fn around(
    center: Point,
    rotation: f32,
    page: Size,
) -> AffineTransform {
    let center = normalized(center, page);
    let (sine, cosine) = rotation.sin_cos();
    let m12 = sine * page.width / page.height;
    let m21 = -sine * page.height / page.width;
    AffineTransform {
        m11: cosine,
        m12,
        m21,
        m22: cosine,
        m31: center.x - cosine * center.x - m21 * center.y,
        m32: center.y - m12 * center.x - cosine * center.y,
    }
}
