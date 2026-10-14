use iced::{Point, Rectangle};

pub(super) fn same_point(first: Point, second: Point) -> bool {
    (first.x - second.x).abs() <= f32::EPSILON
        && (first.y - second.y).abs() <= f32::EPSILON
}

pub(super) fn inset_clip(clip: Rectangle, inset: f32) -> Rectangle {
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

pub(super) fn clip_polygon_to_rectangle(
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

pub(super) fn clipped_polyline_runs(
    points: &[Point],
    clip: Rectangle,
) -> Vec<Vec<Point>> {
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

pub(super) fn clipped_closed_polyline_runs(
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

pub(super) fn clip_segment(
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
