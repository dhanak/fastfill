use super::components::{self, PixelRect};

pub(super) fn detect(
    binary: &[u8],
    width: usize,
    height: usize,
    parts: &[PixelRect],
) -> Vec<PixelRect> {
    // Round calibrated 2.5%, 1.2%, and 0.6% dimensions without float casts.
    let minimum_width = 20.max((width + 20) / 40);
    let maximum_height = 5.max((width.min(height) * 3 + 125) / 250);
    let maximum_gap = ((width * 3 + 250) / 500).clamp(3, 10);
    // Horizontal opening without a morphology dependency: keep only long
    // contiguous row runs, then join touching runs into rule components.
    let mut opened = vec![0; binary.len()];
    for (y, row) in binary.chunks_exact(width).enumerate() {
        let mut x = 0;
        while x < width {
            if row[x] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < width && row[x] != 0 {
                x += 1;
            }
            if x - start >= minimum_width {
                opened[y * width + start..y * width + x].fill(1);
            }
        }
    }
    let mut lines = components::bounds(&opened, width, height)
        .into_iter()
        .filter(|b| b.height <= maximum_height && b.width >= b.height * 5)
        .map(|b| PixelRect {
            y: b.y + b.height / 2,
            height: 1,
            ..b
        })
        .collect::<Vec<_>>();
    lines.extend(table_cells(binary, width, height, &lines, minimum_width));
    lines.extend(broken(parts, minimum_width, maximum_height, maximum_gap));
    lines.sort_by_key(|b| std::cmp::Reverse(b.width));
    lines
}

fn table_cells(
    binary: &[u8],
    width: usize,
    height: usize,
    rules: &[PixelRect],
    minimum: usize,
) -> Vec<PixelRect> {
    if rules.len() < 2 {
        return Vec::new();
    }
    let mut dividers = vec![Vec::new(); rules.len()];
    for x in 0..width {
        let mut y = 0;
        while y < height {
            if binary[y * width + x] == 0 {
                y += 1;
                continue;
            }
            let start = y;
            while y < height && binary[y * width + x] != 0 {
                y += 1;
            }
            if y - start < minimum {
                continue;
            }
            let crossings = rules
                .iter()
                .enumerate()
                .filter(|(_, rule)| {
                    x >= rule.x
                        && x < rule.right()
                        && rule.y >= start
                        && rule.y < y
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            // Two rule intersections distinguish dividers from text stems.
            if crossings.len() >= 2 {
                for index in crossings {
                    dividers[index].push(x);
                }
            }
        }
    }
    let mut cells = Vec::new();
    for (rule, columns) in rules.iter().zip(dividers) {
        let mut edges = vec![rule.x];
        let mut start = 0;
        while start < columns.len() {
            let mut end = start + 1;
            while end < columns.len() && columns[end] == columns[end - 1] + 1 {
                end += 1;
            }
            let center = columns[start].midpoint(columns[end - 1]);
            if center >= rule.x + minimum && center + minimum < rule.right() {
                edges.push(center);
            }
            start = end;
        }
        if edges.len() == 1 {
            continue;
        }
        edges.push(rule.right() - 1);
        cells.extend(edges.windows(2).filter_map(|pair| {
            let width = pair[1] - pair[0] + 1;
            (width >= minimum).then_some(PixelRect {
                x: pair[0],
                width,
                ..*rule
            })
        }));
    }
    cells
}

#[derive(Default)]
struct Row {
    marks: Vec<PixelRect>,
    bottom_sum: usize,
}

impl Row {
    #[allow(clippy::cast_precision_loss)]
    fn bottom(&self) -> f64 {
        self.bottom_sum as f64 / self.marks.len() as f64
    }
}

#[allow(clippy::cast_precision_loss)]
fn broken(
    parts: &[PixelRect],
    minimum: usize,
    max_height: usize,
    gap: usize,
) -> Vec<PixelRect> {
    let tolerance = 2.max(max_height / 3) as f64;
    let max_mark_height = 3.max(max_height / 2);
    let mut marks = parts
        .iter()
        .copied()
        .filter(|b| {
            b.height <= max_height
                && b.width <= minimum * 2
                && (b.height <= max_mark_height || b.width >= b.height * 2)
        })
        .collect::<Vec<_>>();
    marks.sort_by_key(|b| b.bottom());
    let mut rows: Vec<Row> = Vec::new();
    for mark in marks {
        let nearest = rows
            .iter_mut()
            .filter(|row| {
                (row.bottom() - mark.bottom() as f64).abs() <= tolerance
            })
            .min_by(|a, b| {
                (a.bottom() - mark.bottom() as f64)
                    .abs()
                    .total_cmp(&(b.bottom() - mark.bottom() as f64).abs())
            });
        if let Some(row) = nearest {
            row.bottom_sum += mark.bottom();
            row.marks.push(mark);
        } else {
            rows.push(Row {
                marks: vec![mark],
                bottom_sum: mark.bottom(),
            });
        }
    }
    let mut lines = Vec::new();
    for row in &mut rows {
        row.marks.sort_by_key(|b| b.x);
        let mut start = 0;
        for index in 1..=row.marks.len() {
            if index == row.marks.len()
                || row.marks[index]
                    .x
                    .saturating_sub(row.marks[index - 1].right())
                    > gap
            {
                if let Some(line) = create(&row.marks[start..index], minimum) {
                    lines.push(line);
                }
                start = index;
            }
        }
    }
    lines
}

fn create(marks: &[PixelRect], minimum: usize) -> Option<PixelRect> {
    const MINIMUM_MARKS: usize = 6;
    if marks.len() < MINIMUM_MARKS {
        return None;
    }
    let mut gaps = marks
        .windows(2)
        .map(|pair| pair[1].x.saturating_sub(pair[0].right()))
        .collect::<Vec<_>>();
    gaps.sort_unstable();
    let edge_gap = gaps[gaps.len() / 2] + 2;
    let (mut first, mut last) = (0, marks.len() - 1);
    while last - first + 1 > MINIMUM_MARKS
        && marks[first + 1].x.saturating_sub(marks[first].right()) > edge_gap
    {
        first += 1;
    }
    while last - first + 1 > MINIMUM_MARKS
        && marks[last].x.saturating_sub(marks[last - 1].right()) > edge_gap
    {
        last -= 1;
    }
    let marks = &marks[first..=last];
    let width = marks.last()?.right() - marks[0].x;
    let ink: usize = marks.iter().map(|b| b.width).sum();
    let dot_size = 4.max(minimum / 4);
    let dotted = marks
        .iter()
        .all(|b| b.width <= dot_size && b.height <= dot_size);
    if width < minimum * if dotted { 1 } else { 2 }
        || ink * 5 < width
        || ink * 10 > width * 9
    {
        return None;
    }
    let center_sum: usize = marks.iter().map(|b| 2 * b.y + b.height).sum();
    let y = (center_sum + marks.len()) / (2 * marks.len());
    Some(PixelRect {
        x: marks[0].x,
        y,
        width,
        height: 1,
    })
}
