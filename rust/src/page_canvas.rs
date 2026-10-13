use crate::appearance::{PAGE_MARGIN, annotation_color};
use crate::picker::ImportedImage;
use fastfill_rust::annotations::{
    AnnotationGeometry, Annotations, DragModifiers, InkKind, LineKind,
    ShapeAnnotation, ShapeKind, arrowhead_points,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::TouchNavigation;
use iced::widget::canvas;
use iced::{Point, Rectangle, mouse};

mod program;
mod selection;

use selection::{
    draw_group_member, draw_group_selection, draw_marquee, draw_selection,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PageCanvas<'a> {
    pub(crate) image: Option<&'a ImportedImage>,
    pub(crate) navigation: Navigation,
    pub(crate) page_size: PageSize,
    pub(crate) annotations: &'a Annotations,
    pub(crate) input_epoch: u64,
    pub(crate) input_enabled: bool,
    pub(crate) modifiers: DragModifiers,
    pub(crate) background: bool,
}

#[derive(Default)]
pub(crate) struct CanvasState {
    dragging: Option<mouse::Button>,
    last_position: Option<Point>,
    touch: TouchNavigation,
    input_epoch: u64,
}

fn draw_shape(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    match &annotation.geometry {
        AnnotationGeometry::Bounded {
            kind: ShapeKind::Rectangle,
            ..
        } => {
            draw_rectangle(frame, view, annotation, scale);
        }
        AnnotationGeometry::Bounded {
            kind: ShapeKind::Oval,
            ..
        } => draw_oval(frame, view, annotation, scale),
        AnnotationGeometry::Bounded { kind, .. }
            if matches!(kind, ShapeKind::Checkmark | ShapeKind::Cross) =>
        {
            draw_mark(frame, view, annotation, *kind, scale);
        }
        AnnotationGeometry::Linear { .. } => {
            draw_linear(frame, view, annotation, scale);
        }
        AnnotationGeometry::Ink { .. } => {
            draw_ink(frame, view, annotation, scale);
        }
        AnnotationGeometry::Bounded { .. } => {}
    }
}

fn draw_rectangle(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let bounds = annotation.bounds();
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    let stroke_page =
        annotation.stroke_width.min(bounds.width).min(bounds.height);
    let stroke = stroke_page * scale;
    let half = stroke / 2.0;
    let color = annotation_color(annotation.color);
    let page = view.page_rect();
    let page_clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    if annotation.filled {
        let polygon = clip_polygon_to_rectangle(
            transformed_points(
                view,
                annotation,
                &rectangle_points(bounds, 0.0),
            ),
            page_clip,
        );
        if polygon.len() >= 3 {
            frame.fill(&closed_path(&polygon), color);
        }
        return;
    }
    let clip = inset_clip(page_clip, half);
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let points = transformed_points(
        view,
        annotation,
        &rectangle_points(bounds, stroke_page / 2.0),
    );
    let path = path_from_runs(&clipped_closed_polyline_runs(&points, clip));
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke),
    );
}

fn draw_oval(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let bounds = annotation.bounds();
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    let stroke_page =
        annotation.stroke_width.min(bounds.width).min(bounds.height);
    let stroke = stroke_page * scale;
    let half = stroke / 2.0;
    let page = view.page_rect();
    let page_clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    let color = annotation_color(annotation.color);
    if annotation.filled {
        let polygon = clip_polygon_to_rectangle(
            transformed_points(view, annotation, &ellipse_points(bounds, 0.0)),
            page_clip,
        );
        if polygon.len() >= 3 {
            frame.fill(&closed_path(&polygon), color);
        }
        return;
    }
    let clip = inset_clip(page_clip, half);
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let points = transformed_points(
        view,
        annotation,
        &ellipse_points(bounds, stroke_page / 2.0),
    );
    let path = path_from_runs(&clipped_closed_polyline_runs(&points, clip));
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke)
            .with_line_join(canvas::LineJoin::Round),
    );
}

fn draw_linear(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let AnnotationGeometry::Linear { kind, start, end } = &annotation.geometry
    else {
        return;
    };
    let arrowhead = (*kind == LineKind::Arrow)
        .then(|| arrowhead_points(*start, *end))
        .flatten()
        .map(|(left, right)| {
            let left = view.page_to_view(left);
            let right = view.page_to_view(right);
            (Point::new(left.x, left.y), Point::new(right.x, right.y))
        });
    let start = view.page_to_view(*start);
    let end = view.page_to_view(*end);
    let start = Point::new(start.x, start.y);
    let end = Point::new(end.x, end.y);
    let stroke = annotation.stroke_width * scale;
    let half = stroke / 2.0;
    let page = view.page_rect();
    let clip = Rectangle::new(
        Point::new(page.x + half, page.y + half),
        iced::Size::new(page.width - stroke, page.height - stroke),
    );
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let path = canvas::Path::new(|builder| {
        if let Some((start, end)) = clip_segment(start, end, clip) {
            builder.move_to(start);
            builder.line_to(end);
        }
        if let Some((left, right)) = arrowhead {
            let first = clip_segment(end, left, clip);
            let second = clip_segment(end, right, clip);
            match (first, second) {
                (Some((join, left)), Some((next, right)))
                    if same_point(join, next) =>
                {
                    builder.move_to(left);
                    builder.line_to(join);
                    builder.line_to(right);
                }
                (first, second) => {
                    for (start, end) in [first, second].into_iter().flatten() {
                        builder.move_to(start);
                        builder.line_to(end);
                    }
                }
            }
        }
    });
    let color = annotation_color(annotation.color);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke)
            .with_line_join(canvas::LineJoin::Round),
    );
}

fn draw_mark(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    kind: ShapeKind,
    scale: f32,
) {
    let bounds = annotation.bounds();
    let stroke_page =
        annotation.stroke_width.min(bounds.width).min(bounds.height);
    let stroke = stroke_page * scale;
    if bounds.width <= stroke_page || bounds.height <= stroke_page {
        return;
    }
    let inset_page = stroke_page / 2.0;
    let mark_bounds = Rectangle::new(
        Point::new(bounds.x + inset_page, bounds.y + inset_page),
        iced::Size::new(
            bounds.width - stroke_page,
            bounds.height - stroke_page,
        ),
    );
    let points = mark_points(kind, mark_bounds).map(|point| {
        let point = annotation.rotated_point(PagePoint::new(point.x, point.y));
        let point = view.page_to_view(point);
        Point::new(point.x, point.y)
    });
    let page = view.page_rect();
    let clip = inset_clip(
        Rectangle::new(
            Point::new(page.x, page.y),
            iced::Size::new(page.width, page.height),
        ),
        stroke / 2.0,
    );
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let path = canvas::Path::new(|builder| {
        if kind == ShapeKind::Checkmark {
            let first = clip_segment(points[0], points[1], clip);
            let second = clip_segment(points[1], points[2], clip);
            match (first, second) {
                (Some((start, join)), Some((next, end)))
                    if same_point(join, next) =>
                {
                    builder.move_to(start);
                    builder.line_to(join);
                    builder.line_to(end);
                }
                (first, second) => {
                    for (start, end) in [first, second].into_iter().flatten() {
                        builder.move_to(start);
                        builder.line_to(end);
                    }
                }
            }
        } else {
            for (start, end) in [(points[0], points[1]), (points[2], points[3])]
            {
                if let Some((start, end)) = clip_segment(start, end, clip) {
                    builder.move_to(start);
                    builder.line_to(end);
                }
            }
        }
    });
    let color = annotation_color(annotation.color);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke)
            .with_line_join(canvas::LineJoin::Round),
    );
}

fn draw_ink(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let AnnotationGeometry::Ink { kind, points } = &annotation.geometry else {
        return;
    };
    if points.len() < 2 {
        return;
    }
    let points = transformed_points(view, annotation, points);
    let page = view.page_rect();
    let page_clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    let mut color = annotation_color(annotation.color);
    if *kind == InkKind::Highlighter {
        color.a = 0.35;
    }
    if annotation.filled && points.len() >= 3 {
        let polygon = clip_polygon_to_rectangle(points.clone(), page_clip);
        if polygon.len() >= 3 {
            let path = canvas::Path::new(|builder| {
                builder.move_to(polygon[0]);
                for point in &polygon[1..] {
                    builder.line_to(*point);
                }
                builder.close();
            });
            frame.fill(&path, color);
        }
    }

    let stroke = annotation.stroke_width * scale;
    let half = stroke / 2.0;
    let clip = Rectangle::new(
        Point::new(page.x + half, page.y + half),
        iced::Size::new(page.width - stroke, page.height - stroke),
    );
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let runs = if annotation.filled {
        clipped_closed_polyline_runs(&points, clip)
    } else {
        clipped_polyline_runs(&points, clip)
    };
    let path = canvas::Path::new(|builder| {
        for run in &runs {
            let Some(first) = run.first() else {
                continue;
            };
            let closed =
                run.last().is_some_and(|last| same_point(*first, *last));
            builder.move_to(*first);
            let end = run.len() - usize::from(closed);
            for point in &run[1..end] {
                builder.line_to(*point);
            }
            if closed {
                builder.close();
            }
        }
    });
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round),
    );
}

fn mark_points(kind: ShapeKind, bounds: Rectangle) -> [Point; 4] {
    let point = |x: f32, y: f32| {
        Point::new(bounds.x + x * bounds.width, bounds.y + y * bounds.height)
    };
    match kind {
        ShapeKind::Checkmark => [
            point(0.0, 0.45),
            point(0.36, 1.0),
            point(1.0, 0.0),
            point(1.0, 0.0),
        ],
        ShapeKind::Cross => [
            point(0.0, 0.0),
            point(1.0, 1.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
        ],
        ShapeKind::Rectangle | ShapeKind::Oval => {
            unreachable!("mark renderer requires a mark kind")
        }
    }
}

fn same_point(first: Point, second: Point) -> bool {
    (first.x - second.x).abs() <= f32::EPSILON
        && (first.y - second.y).abs() <= f32::EPSILON
}

fn transformed_points(
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    points: &[PagePoint],
) -> Vec<Point> {
    points
        .iter()
        .map(|point| {
            let point = view.page_to_view(annotation.rotated_point(*point));
            Point::new(point.x, point.y)
        })
        .collect()
}

fn rectangle_points(bounds: PageRect, inset: f32) -> [PagePoint; 4] {
    let left = bounds.x + inset;
    let top = bounds.y + inset;
    let right = bounds.x + bounds.width - inset;
    let bottom = bounds.y + bounds.height - inset;
    [
        PagePoint::new(left, top),
        PagePoint::new(right, top),
        PagePoint::new(right, bottom),
        PagePoint::new(left, bottom),
    ]
}

fn ellipse_points(bounds: PageRect, inset: f32) -> Vec<PagePoint> {
    const SEGMENTS: u16 = 128;

    let center = bounds.center();
    let width = (bounds.width - inset * 2.0).max(0.0);
    let height = (bounds.height - inset * 2.0).max(0.0);
    let step = std::f32::consts::TAU / f32::from(SEGMENTS);
    (0..SEGMENTS)
        .map(|index| {
            let angle = step * f32::from(index);
            PagePoint::new(
                center.x + width / 2.0 * angle.cos(),
                center.y + height / 2.0 * angle.sin(),
            )
        })
        .collect()
}

fn closed_path(points: &[Point]) -> canvas::Path {
    canvas::Path::new(|builder| {
        let Some(first) = points.first() else {
            return;
        };
        builder.move_to(*first);
        for point in &points[1..] {
            builder.line_to(*point);
        }
        builder.close();
    })
}

fn path_from_runs(runs: &[Vec<Point>]) -> canvas::Path {
    canvas::Path::new(|builder| {
        for run in runs {
            let Some(first) = run.first() else {
                continue;
            };
            let closed =
                run.last().is_some_and(|last| same_point(*first, *last));
            builder.move_to(*first);
            let end = run.len() - usize::from(closed);
            for point in &run[1..end] {
                builder.line_to(*point);
            }
            if closed {
                builder.close();
            }
        }
    })
}

fn inset_clip(clip: Rectangle, inset: f32) -> Rectangle {
    Rectangle::new(
        Point::new(clip.x + inset, clip.y + inset),
        iced::Size::new(
            (clip.width - inset * 2.0).max(0.0),
            (clip.height - inset * 2.0).max(0.0),
        ),
    )
}

#[derive(Clone, Copy)]
enum ClipEdge {
    Left,
    Right,
    Top,
    Bottom,
}

fn clip_polygon_to_rectangle(
    mut polygon: Vec<Point>,
    clip: Rectangle,
) -> Vec<Point> {
    for edge in [
        ClipEdge::Left,
        ClipEdge::Right,
        ClipEdge::Top,
        ClipEdge::Bottom,
    ] {
        let input = std::mem::take(&mut polygon);
        let Some(mut previous) = input.last().copied() else {
            break;
        };
        let mut previous_inside = point_inside_edge(previous, edge, clip);
        for current in input {
            let current_inside = point_inside_edge(current, edge, clip);
            if current_inside != previous_inside {
                polygon.push(edge_intersection(previous, current, edge, clip));
            }
            if current_inside {
                polygon.push(current);
            }
            previous = current;
            previous_inside = current_inside;
        }
    }
    polygon
}

fn point_inside_edge(point: Point, edge: ClipEdge, clip: Rectangle) -> bool {
    match edge {
        ClipEdge::Left => point.x >= clip.x,
        ClipEdge::Right => point.x <= clip.x + clip.width,
        ClipEdge::Top => point.y >= clip.y,
        ClipEdge::Bottom => point.y <= clip.y + clip.height,
    }
}

fn edge_intersection(
    start: Point,
    end: Point,
    edge: ClipEdge,
    clip: Rectangle,
) -> Point {
    let vertical = matches!(edge, ClipEdge::Left | ClipEdge::Right);
    let boundary = match edge {
        ClipEdge::Left => clip.x,
        ClipEdge::Right => clip.x + clip.width,
        ClipEdge::Top => clip.y,
        ClipEdge::Bottom => clip.y + clip.height,
    };
    if vertical {
        let ratio = (boundary - start.x) / (end.x - start.x);
        Point::new(boundary, start.y + ratio * (end.y - start.y))
    } else {
        let ratio = (boundary - start.y) / (end.y - start.y);
        Point::new(start.x + ratio * (end.x - start.x), boundary)
    }
}

fn clipped_polyline_runs(points: &[Point], clip: Rectangle) -> Vec<Vec<Point>> {
    let mut runs = Vec::new();
    let mut run = Vec::new();
    for segment in points.windows(2) {
        let clipped = clip_segment(segment[0], segment[1], clip)
            .filter(|(start, end)| !same_point(*start, *end));
        match clipped {
            Some((start, end)) => {
                if run.last().is_none_or(|last| !same_point(*last, start)) {
                    if !run.is_empty() {
                        runs.push(std::mem::take(&mut run));
                    }
                    run.push(start);
                }
                run.push(end);
            }
            None if !run.is_empty() => runs.push(std::mem::take(&mut run)),
            None => {}
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

fn clipped_closed_polyline_runs(
    points: &[Point],
    clip: Rectangle,
) -> Vec<Vec<Point>> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut closed = Vec::with_capacity(points.len() + 1);
    closed.extend_from_slice(points);
    closed.push(*first);
    let mut runs = clipped_polyline_runs(&closed, clip);

    if runs.len() > 1 {
        let joins_at_start = runs
            .last()
            .and_then(|run| run.last())
            .zip(runs.first().and_then(|run| run.first()))
            .is_some_and(|(last, first)| same_point(*last, *first));
        if joins_at_start {
            let mut last = runs.pop().expect("run exists");
            let first = runs.remove(0);
            last.extend_from_slice(&first[1..]);
            runs.insert(0, last);
        }
    }
    runs
}

fn clip_segment(
    start: Point,
    end: Point,
    clip: Rectangle,
) -> Option<(Point, Point)> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let mut first: f32 = 0.0;
    let mut last: f32 = 1.0;
    for (direction, distance) in [
        (-dx, start.x - clip.x),
        (dx, clip.x + clip.width - start.x),
        (-dy, start.y - clip.y),
        (dy, clip.y + clip.height - start.y),
    ] {
        if direction.abs() <= f32::EPSILON {
            if distance < 0.0 {
                return None;
            }
            continue;
        }
        let ratio = distance / direction;
        if direction < 0.0 {
            if ratio > last {
                return None;
            }
            first = first.max(ratio);
        } else {
            if ratio < first {
                return None;
            }
            last = last.min(ratio);
        }
    }
    Some((
        Point::new(start.x + first * dx, start.y + first * dy),
        Point::new(start.x + last * dx, start.y + last * dy),
    ))
}

pub(crate) fn page_viewport(size: iced::Size) -> PageRect {
    PageRect::new(
        PAGE_MARGIN,
        PAGE_MARGIN,
        (size.width - PAGE_MARGIN * 2.0).max(0.0),
        (size.height - PAGE_MARGIN * 2.0).max(0.0),
    )
}

#[cfg(test)]
#[path = "page_canvas_tests.rs"]
mod tests;
