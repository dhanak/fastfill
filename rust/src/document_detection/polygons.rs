//! Connected-outline hulls and closed Douglas–Peucker simplification.

use image::GrayImage;

pub(super) type Point = [f64; 2];

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

pub(super) fn convex(points: [Point; 4]) -> bool {
    let turns = std::array::from_fn::<_, 4, _>(|i| {
        cross(points[i], points[(i + 1) % 4], points[(i + 2) % 4])
    });
    turns.iter().all(|turn| *turn > 0.0) || turns.iter().all(|turn| *turn < 0.0)
}

pub(super) fn area(polygon: &[Point]) -> f64 {
    polygon
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let b = polygon[(i + 1) % polygon.len()];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

pub(super) fn ordered(mut polygon: [Point; 4]) -> [Point; 4] {
    let center = [
        polygon.iter().map(|p| p[0]).sum::<f64>() / 4.0,
        polygon.iter().map(|p| p[1]).sum::<f64>() / 4.0,
    ];
    polygon.sort_by(|a, b| {
        (a[1] - center[1])
            .atan2(a[0] - center[0])
            .total_cmp(&(b[1] - center[1]).atan2(b[0] - center[0]))
    });
    let first = (0..4)
        .min_by(|a, b| {
            (polygon[*a][0] + polygon[*a][1])
                .total_cmp(&(polygon[*b][0] + polygon[*b][1]))
        })
        .unwrap_or(0);
    polygon.rotate_left(first);
    polygon
}

fn hull(points: &mut [Point]) -> Vec<Point> {
    points.sort_unstable_by(|a, b| {
        a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]))
    });
    let mut output = Vec::new();
    for point in points.iter() {
        while output.len() >= 2
            && cross(output[output.len() - 2], output[output.len() - 1], *point)
                <= 0.0
        {
            output.pop();
        }
        output.push(*point);
    }
    let lower = output.len();
    for point in points.iter().rev().skip(1) {
        while output.len() > lower
            && cross(output[output.len() - 2], output[output.len() - 1], *point)
                <= 0.0
        {
            output.pop();
        }
        output.push(*point);
    }
    output.pop();
    output
}

fn approximate(hull: &[Point], epsilon: f64) -> Vec<Point> {
    let distance = |a: Point, b: Point| (a[0] - b[0]).hypot(a[1] - b[1]);
    let farthest = |origin| {
        (0..hull.len())
            .max_by(|a, b| {
                distance(origin, hull[*a])
                    .total_cmp(&distance(origin, hull[*b]))
            })
            .unwrap_or(0)
    };
    let first = farthest(hull[0]);
    let second = farthest(hull[first]);
    let mut ring = hull[first..]
        .iter()
        .chain(&hull[..first])
        .copied()
        .collect::<Vec<_>>();
    ring.push(hull[first]);
    let opposite = (second + hull.len() - first) % hull.len();
    let mut keep = vec![false; ring.len()];
    keep[0] = true;
    keep[opposite] = true;
    let mut stack = vec![(0, opposite), (opposite, ring.len() - 1)];
    while let Some((start, end)) = stack.pop() {
        let length = distance(ring[start], ring[end]);
        let candidate = (start + 1..end)
            .map(|i| {
                (
                    i,
                    cross(ring[start], ring[end], ring[i]).abs()
                        / length.max(1.0),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((index, deviation)) = candidate
            && deviation > epsilon
        {
            keep[index] = true;
            stack.push((start, index));
            stack.push((index, end));
        }
    }
    ring.into_iter()
        .zip(keep)
        .filter_map(|(p, keep)| keep.then_some(p))
        .collect()
}

pub(super) fn candidates(
    mask: &GrayImage,
    include_holes: bool,
) -> Vec<[Point; 4]> {
    let mut output = Vec::new();
    let (width, height) = (mask.width() as usize, mask.height() as usize);
    let mut remaining = mask.as_raw().clone();
    for inverted in [false, true] {
        if inverted {
            if !include_holes {
                break;
            }
            remaining =
                mask.as_raw().iter().map(|v| u8::from(*v == 0)).collect();
        }
        let mut queue = Vec::new();
        let mut boundary = Vec::new();
        for index in 0..remaining.len() {
            if remaining[index] == 0 {
                continue;
            }
            queue.clear();
            boundary.clear();
            queue.push(index);
            remaining[index] = 0;
            let mut next = 0;
            while next < queue.len() {
                let index = queue[next];
                next += 1;
                let (x, y) = (index % width, index / width);
                let mut edge =
                    x == 0 || y == 0 || x + 1 == width || y + 1 == height;
                for row in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                    for col in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                        let neighbor = row * width + col;
                        if (mask.as_raw()[neighbor] == 0) != inverted {
                            edge = true;
                        }
                        if remaining[neighbor] != 0 {
                            remaining[neighbor] = 0;
                            queue.push(neighbor);
                        }
                    }
                }
                if edge {
                    #[allow(clippy::cast_precision_loss)]
                    boundary.push([x as f64, y as f64]);
                }
            }
            if boundary.len() < 4 {
                continue;
            }
            let hull = hull(&mut boundary);
            #[allow(clippy::cast_precision_loss)]
            if hull.len() < 4 || area(&hull) < (width * height) as f64 * 0.05 {
                continue;
            }
            let perimeter = hull
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let b = hull[(i + 1) % hull.len()];
                    (a[0] - b[0]).hypot(a[1] - b[1])
                })
                .sum::<f64>();
            for factor in [0.01, 0.015, 0.02, 0.03, 0.04] {
                if let Ok(polygon) = <[Point; 4]>::try_from(approximate(
                    &hull,
                    perimeter * factor,
                )) {
                    output.push(ordered(polygon));
                }
            }
        }
    }
    output
}
