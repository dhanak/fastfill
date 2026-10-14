use super::clipping::{
    clip_polygon_to_rectangle, clip_segment, clipped_closed_polyline_runs,
    clipped_polyline_runs, inset_clip, same_point,
};
use crate::appearance::{annotation_color, text_font};
use fastfill_rust::annotations::{
    AnnotationGeometry, InkKind, LineKind, ShapeAnnotation, ShapeKind,
    TEXT_LINE_HEIGHT, TextAnnotation, arrowhead_points,
};
use fastfill_rust::geometry::Point as PagePoint;
use fastfill_rust::geometry::{PageView, Rect as PageRect};
use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
use iced::advanced::text::{self, Paragraph as _, Text as ParagraphText};
use iced::widget::canvas;
use iced::{Color, Pixels, Point, Rectangle, Size, Vector, alignment};

pub(super) fn draw_shape(
    frame: &mut canvas::Frame,
    view: PageView,
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

pub(super) fn draw_text(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: &TextAnnotation,
    scale: f32,
) {
    draw_text_content(frame, view, annotation, scale, true);
}

pub(super) fn draw_text_underline(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: &TextAnnotation,
    scale: f32,
) {
    draw_text_content(frame, view, annotation, scale, false);
}

fn draw_text_content(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: &TextAnnotation,
    scale: f32,
    glyphs: bool,
) {
    let position = view
        .page_to_view(PagePoint::new(annotation.bounds.x, annotation.bounds.y));
    let page = view.page_rect();
    let clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    let Some(clip) = clip.intersection(&Rectangle::with_size(frame.size()))
    else {
        return;
    };
    frame.with_clip(clip, |frame| {
        let center = view.page_to_view(annotation.bounds.center());
        frame.push_transform();
        frame.translate(Vector::new(center.x, center.y));
        frame.rotate(annotation.rotation);
        frame.translate(Vector::new(-center.x, -center.y));
        let color = annotation_color(annotation.format.color);
        let size = Pixels(annotation.format.font_size * scale);
        let font = text_font(annotation.format);
        if glyphs {
            frame.fill_text(canvas::Text {
                content: annotation.text.clone(),
                position: Point::new(position.x, position.y),
                max_width: f32::INFINITY,
                color,
                size,
                line_height: text::LineHeight::Relative(TEXT_LINE_HEIGHT),
                font,
                align_x: text::Alignment::Left,
                align_y: alignment::Vertical::Top,
                shaping: text::Shaping::Advanced,
            });
        }
        if annotation.format.underline {
            draw_underlines(frame, annotation, position, size, font, color);
        }
        frame.pop_transform();
    });
}

fn draw_underlines(
    frame: &mut canvas::Frame,
    annotation: &TextAnnotation,
    position: PagePoint,
    size: Pixels,
    font: iced::Font,
    color: Color,
) {
    let line_height = size.0 * TEXT_LINE_HEIGHT;
    let width = |line: &str| {
        GraphicsParagraph::with_text(ParagraphText {
            content: line,
            bounds: Size::INFINITE,
            size,
            line_height: text::LineHeight::Relative(TEXT_LINE_HEIGHT),
            font,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
        })
        .min_bounds()
        .width
    };
    let thickness = (size.0 / 16.0).clamp(1.0, 3.0);
    let mut y = position.y + line_height - thickness;
    for line in annotation.text.lines() {
        let width = width(line);
        if width <= 0.0 {
            y += line_height;
            continue;
        }
        let path = canvas::Path::line(
            Point::new(position.x, y),
            Point::new(position.x + width, y),
        );
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(color)
                .with_width(thickness),
        );
        y += line_height;
    }
}

fn draw_rectangle(
    frame: &mut canvas::Frame,
    view: PageView,
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
    view: PageView,
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
    view: PageView,
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
    view: PageView,
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
    view: PageView,
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

pub(super) fn mark_points(kind: ShapeKind, bounds: Rectangle) -> [Point; 4] {
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

fn transformed_points(
    view: PageView,
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

pub(super) fn closed_path(points: &[Point]) -> canvas::Path {
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
