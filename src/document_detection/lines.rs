//! Seeded progressive Hough recovery and intersections of related sides.

use super::polygons::Point;
use image::GrayImage;
use std::f64::consts::PI;

#[derive(Clone, Copy)]
struct Line {
    a: Point,
    b: Point,
}

impl Line {
    fn length(self) -> f64 {
        (self.b[0] - self.a[0]).hypot(self.b[1] - self.a[1])
    }

    fn angle(self) -> f64 {
        (self.b[1] - self.a[1])
            .atan2(self.b[0] - self.a[0])
            .rem_euclid(PI)
    }

    fn position(self, horizontal: bool, width: f64, height: f64) -> f64 {
        if horizontal {
            self.a[1]
                + (width / 2.0 - self.a[0]) * (self.b[1] - self.a[1])
                    / (self.b[0] - self.a[0])
        } else {
            self.a[0]
                + (height / 2.0 - self.a[1]) * (self.b[0] - self.a[0])
                    / (self.b[1] - self.a[1])
        }
    }
}

pub(super) fn candidates(
    edges: &GrayImage,
    length_ratio: f64,
    threshold: i32,
    gap: f64,
) -> Vec<[Point; 4]> {
    let width = f64::from(edges.width());
    let height = f64::from(edges.height());
    let mut lines =
        segments(edges, threshold, width.min(height) * length_ratio, gap);
    lines.sort_unstable_by(|a, b| b.length().total_cmp(&a.length()));
    let horizontal = distinct(&lines, true, width, height);
    let vertical = distinct(&lines, false, width, height);
    let mut quads = Vec::new();
    for (i, first) in horizontal.iter().enumerate() {
        for second in &horizontal[i + 1..] {
            let (top, bottom) =
                if first.a[1] + first.b[1] < second.a[1] + second.b[1] {
                    (*first, *second)
                } else {
                    (*second, *first)
                };
            if (bottom.a[1] + bottom.b[1] - top.a[1] - top.b[1]) / 2.0
                < height * 0.12
            {
                continue;
            }
            for (j, first) in vertical.iter().enumerate() {
                for second in &vertical[j + 1..] {
                    let (left, right) = if first.a[0] + first.b[0]
                        < second.a[0] + second.b[0]
                    {
                        (*first, *second)
                    } else {
                        (*second, *first)
                    };
                    if (right.a[0] + right.b[0] - left.a[0] - left.b[0]) / 2.0
                        < width * 0.12
                    {
                        continue;
                    }
                    let intersections = [
                        intersect(top, left),
                        intersect(top, right),
                        intersect(bottom, right),
                        intersect(bottom, left),
                    ];
                    let [Some(a), Some(b), Some(c), Some(d)] = intersections
                    else {
                        continue;
                    };
                    let mut polygon = [a, b, c, d];
                    let margin = width.min(height) * 0.05;
                    if polygon.iter().any(|[x, y]| {
                        *x < -margin
                            || *y < -margin
                            || *x > width + margin
                            || *y > height + margin
                    }) {
                        continue;
                    }
                    for point in &mut polygon {
                        point[0] = point[0].round().clamp(0.0, width - 1.0);
                        point[1] = point[1].round().clamp(0.0, height - 1.0);
                    }
                    quads.push(polygon);
                }
            }
        }
    }
    quads
}

fn distinct(
    lines: &[Line],
    horizontal: bool,
    width: f64,
    height: f64,
) -> Vec<Line> {
    let spacing = 10.0_f64.max(width.min(height) * 0.015);
    let mut selected: Vec<Line> = Vec::new();
    for &line in lines {
        let (dx, dy) =
            ((line.b[0] - line.a[0]).abs(), (line.b[1] - line.a[1]).abs());
        if if horizontal {
            dx < dy * 1.5
        } else {
            dy < dx * 1.5
        } {
            continue;
        }
        let angle = line.angle();
        let position = line.position(horizontal, width, height);
        if selected.iter().any(|other| {
            let difference = (angle - other.angle()).abs();
            difference.min(PI - difference) < PI / 60.0
                && (position - other.position(horizontal, width, height)).abs()
                    < spacing
        }) {
            continue;
        }
        selected.push(line);
        if selected.len() == 14 {
            break;
        }
    }
    selected
}

fn intersect(first: Line, second: Line) -> Option<Point> {
    let equation = |line: Line| {
        let (a, b) = (line.b[1] - line.a[1], line.a[0] - line.b[0]);
        (a, b, a * line.a[0] + b * line.a[1])
    };
    let (first_a, first_b, first_c) = equation(first);
    let (second_a, second_b, second_c) = equation(second);
    let determinant = first_a * second_b - second_a * first_b;
    (determinant.abs() >= 0.001).then(|| {
        [
            (second_b * first_c - first_b * second_c) / determinant,
            (first_a * second_c - second_a * first_c) / determinant,
        ]
    })
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn segments(
    edges: &GrayImage,
    threshold: i32,
    minimum: f64,
    gap: f64,
) -> Vec<Line> {
    let mut points = edges
        .enumerate_pixels()
        .filter(|(_, _, p)| p[0] != 0)
        .map(|(x, y, _)| [x, y])
        .collect::<Vec<_>>();
    let offset = edges.width() as usize + edges.height() as usize;
    let stride = offset * 2 + 1;
    let angles = (0..180)
        .map(|angle| {
            let theta = f64::from(angle) * f64::from(PI as f32 / 180.0);
            [theta.cos() as f32, theta.sin() as f32]
        })
        .collect::<Vec<_>>();
    let index = |point: [u32; 2], angle: usize| {
        let [cos, sin] = angles[angle];
        let rho =
            (point[0] as f32 * cos + point[1] as f32 * sin).round_ties_even();
        angle * stride + (rho + offset as f32) as usize
    };
    let mut votes = vec![0_i32; stride * 180];
    let mut mask = edges.clone();
    let mut lines = Vec::new();
    // Match the fixed seed/multiply-with-carry sampling used by OpenCV.
    // Consume traced pixels so the same strong edge cannot invent many rays.
    let mut random = u64::MAX;
    while !points.is_empty() {
        random = u64::from(random as u32) * 4_164_903_690 + (random >> 32);
        let point = points.swap_remove(random as u32 as usize % points.len());
        if mask[(point[0], point[1])][0] == 0 {
            continue;
        }
        let (mut maximum, mut direction) = (threshold - 1, 0);
        for angle in 0..180 {
            let bin = index(point, angle);
            votes[bin] += 1;
            if votes[bin] > maximum {
                maximum = votes[bin];
                direction = angle;
            }
        }
        if maximum < threshold {
            continue;
        }
        let [cos, sin] = angles[direction];
        let tangent = [-f64::from(sin), f64::from(cos)];
        let (a, first) = trace(&mask, point, tangent, gap);
        let (b, second) = trace(&mask, point, tangent.map(|v| -v), gap);
        let good =
            (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) >= minimum.round();
        for [x, y] in first.into_iter().chain(second) {
            if mask[(x, y)][0] == 0 {
                continue;
            }
            if good {
                for angle in 0..180 {
                    votes[index([x, y], angle)] -= 1;
                }
            }
            mask[(x, y)][0] = 0;
        }
        if good {
            lines.push(Line { a, b });
        }
    }
    lines
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn trace(
    mask: &GrayImage,
    start: [u32; 2],
    direction: Point,
    gap: f64,
) -> (Point, Vec<[u32; 2]>) {
    let divisor = direction[0].abs().max(direction[1].abs());
    let along_x = direction[0].abs() > direction[1].abs();
    let step =
        direction.map(|v| (v / divisor * 65536.0).round_ties_even() as i64);
    let mut position = start.map(|v| i64::from(v) * 65536 + 32768);
    let mut end = start.map(f64::from);
    let mut active = Vec::new();
    let mut missed = 0.0;
    // One axis advances exactly one pixel: border/gap always bounds the walk.
    for _ in 0..=mask.width().max(mask.height()) {
        let [x, y] = position.map(|v| v >> 16);
        if x < 0
            || y < 0
            || x >= i64::from(mask.width())
            || y >= i64::from(mask.height())
        {
            break;
        }
        if mask[(x as u32, y as u32)][0] != 0 {
            end = [f64::from(x as u32), f64::from(y as u32)];
            active.push([x as u32, y as u32]);
            missed = 0.0;
        } else {
            missed += 1.0;
            if missed > gap {
                break;
            }
        }
        position[0] += if along_x {
            step[0].signum() * 65536
        } else {
            step[0]
        };
        position[1] += if along_x {
            step[1]
        } else {
            step[1].signum() * 65536
        };
    }
    (end, active)
}
