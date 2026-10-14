use super::clipping::{
    clip_polygon_to_rectangle, clipped_closed_polyline_runs,
    clipped_polyline_runs, inset_clip, same_point,
};
use crate::appearance::{annotation_color, text_font};
use fastfill_rust::annotations::{
    AnnotationRef, ShapeAnnotation, TEXT_LINE_HEIGHT, TextAnnotation,
};
use fastfill_rust::geometry::PageView;
use fastfill_rust::geometry::Point as PagePoint;
use fastfill_rust::rendering::{
    RenderAnnotation, SceneLineCap, SceneLineJoin, ScenePaint, ScenePath,
    layout_annotation, layout_shape,
};
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
    for path in layout_shape(annotation) {
        draw_path(frame, view, &path, scale);
    }
}

pub(super) fn draw_annotation(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: AnnotationRef<'_>,
    scale: f32,
) {
    match layout_annotation(annotation) {
        RenderAnnotation::Paths(paths) => {
            for path in paths {
                draw_path(frame, view, &path, scale);
            }
        }
        RenderAnnotation::Text(annotation) => {
            draw_text(frame, view, annotation, scale);
        }
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

fn draw_path(
    frame: &mut canvas::Frame,
    view: PageView,
    path: &ScenePath,
    scale: f32,
) {
    let page = view.page_rect();
    let page_clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    if let Some(fill) = path.fill {
        for subpath in &path.subpaths {
            if !subpath.closed || subpath.points.len() < 3 {
                continue;
            }
            let polygon = clip_polygon_to_rectangle(
                view_points(view, &subpath.points),
                page_clip,
            );
            if polygon.len() >= 3 {
                frame.fill(&closed_path(&polygon), scene_color(fill));
            }
        }
    }
    let Some(stroke) = path.stroke else {
        return;
    };
    let width = stroke.width * scale;
    let clip = inset_clip(page_clip, width / 2.0);
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let mut runs = Vec::new();
    for subpath in &path.subpaths {
        let points = view_points(view, &subpath.points);
        if subpath.closed {
            runs.extend(clipped_closed_polyline_runs(&points, clip));
        } else {
            runs.extend(clipped_polyline_runs(&points, clip));
        }
    }
    let path = path_from_runs(&runs);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(scene_color(stroke.paint))
            .with_width(width)
            .with_line_cap(match stroke.cap {
                SceneLineCap::Butt => canvas::LineCap::Butt,
                SceneLineCap::Round => canvas::LineCap::Round,
            })
            .with_line_join(match stroke.join {
                SceneLineJoin::Miter => canvas::LineJoin::Miter,
                SceneLineJoin::Round => canvas::LineJoin::Round,
            }),
    );
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

fn scene_color(paint: ScenePaint) -> Color {
    let mut color = annotation_color(paint.color);
    color.a *= paint.opacity;
    color
}

fn view_points(view: PageView, points: &[PagePoint]) -> Vec<Point> {
    points
        .iter()
        .map(|point| {
            let point = view.page_to_view(*point);
            Point::new(point.x, point.y)
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
