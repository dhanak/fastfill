use super::render::closed_path;
use crate::appearance::{SELECTION_HANDLE_SIZE, SELECTION_STROKE, accent};
use fastfill_rust::annotations::TextAnnotation;
use fastfill_rust::annotations::{AnnotationGeometry, ShapeAnnotation};
use fastfill_rust::geometry::{PageView, Point as PagePoint, Rect as PageRect};
use iced::widget::canvas;
use iced::{Color, Point};

pub(super) fn draw_selection(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let blue = accent();
    let frame_points = annotation_frame_points(view, annotation);
    let path = annotation_frame_path(view, annotation, scale, frame_points);
    frame.stroke(&path, canvas::Stroke::default().with_color(blue));
    let handles: Vec<Point> = match &annotation.geometry {
        AnnotationGeometry::Bounded { .. } | AnnotationGeometry::Ink { .. } => {
            frame_points
                .expect("rotatable annotation has frame corners")
                .into_iter()
                .collect()
        }
        AnnotationGeometry::Linear { start, end, .. } => [start, end]
            .into_iter()
            .map(|point| {
                let point = view.page_to_view(*point);
                Point::new(point.x, point.y)
            })
            .collect(),
    };
    if let (Some(corners), Some(handle)) = (
        annotation.frame_corners(),
        annotation.rotation_handle(scale),
    ) {
        draw_rotation_handle(frame, view, corners, handle);
    }
    for center in handles {
        let half = SELECTION_HANDLE_SIZE / 2.0;
        let path = canvas::Path::rectangle(
            Point::new(center.x - half, center.y - half),
            iced::Size::new(SELECTION_HANDLE_SIZE, SELECTION_HANDLE_SIZE),
        );
        frame.fill(&path, Color::WHITE);
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(blue)
                .with_width(SELECTION_STROKE),
        );
    }
}

pub(super) fn draw_text_selection(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: &TextAnnotation,
    scale: f32,
) {
    let corners = annotation.frame_corners();
    let points = corners.map(|point| {
        let point = view.page_to_view(point);
        Point::new(point.x, point.y)
    });
    frame.stroke(
        &closed_path(&points),
        canvas::Stroke::default()
            .with_color(accent())
            .with_width(SELECTION_STROKE),
    );
    if let Some(handle) = annotation.rotation_handle(scale) {
        draw_rotation_handle(frame, view, corners, handle);
    }
}

fn draw_rotation_handle(
    frame: &mut canvas::Frame,
    view: PageView,
    corners: [PagePoint; 4],
    handle: PagePoint,
) {
    let top = PagePoint::new(
        corners[0].x.midpoint(corners[1].x),
        corners[0].y.midpoint(corners[1].y),
    );
    let top = view.page_to_view(top);
    let handle = view.page_to_view(handle);
    let top = Point::new(top.x, top.y);
    let handle = Point::new(handle.x, handle.y);
    let stem = canvas::Path::line(top, handle);
    frame.stroke(
        &stem,
        canvas::Stroke::default()
            .with_color(accent())
            .with_width(SELECTION_STROKE),
    );
    let knob = canvas::Path::circle(handle, SELECTION_HANDLE_SIZE / 2.0);
    frame.fill(&knob, Color::WHITE);
    frame.stroke(
        &knob,
        canvas::Stroke::default()
            .with_color(accent())
            .with_width(SELECTION_STROKE),
    );
}

pub(super) fn draw_group_member(
    frame: &mut canvas::Frame,
    view: PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let mut color = accent();
    color.a = 0.5;
    let frame_points = annotation_frame_points(view, annotation);
    let path = annotation_frame_path(view, annotation, scale, frame_points);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(SELECTION_STROKE),
    );
}

pub(super) fn draw_group_selection(
    frame: &mut canvas::Frame,
    view: PageView,
    bounds: PageRect,
) {
    let path = bounds_path(view, bounds);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(accent())
            .with_width(SELECTION_STROKE),
    );
}

pub(super) fn draw_marquee(
    frame: &mut canvas::Frame,
    view: PageView,
    bounds: PageRect,
) {
    let path = bounds_path(view, bounds);
    let mut fill = accent();
    fill.a = 0.12;
    frame.fill(&path, fill);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(accent())
            .with_width(SELECTION_STROKE),
    );
}

fn bounds_path(view: PageView, bounds: PageRect) -> canvas::Path {
    let origin = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let opposite = view.page_to_view(PagePoint::new(
        bounds.x + bounds.width,
        bounds.y + bounds.height,
    ));
    canvas::Path::rectangle(
        Point::new(origin.x, origin.y),
        iced::Size::new(opposite.x - origin.x, opposite.y - origin.y),
    )
}

fn annotation_frame_points(
    view: PageView,
    annotation: &ShapeAnnotation,
) -> Option<[Point; 4]> {
    annotation.frame_corners().map(|corners| {
        corners.map(|point| {
            let point = view.page_to_view(point);
            Point::new(point.x, point.y)
        })
    })
}

fn annotation_frame_path(
    view: PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
    frame_points: Option<[Point; 4]>,
) -> canvas::Path {
    frame_points.map_or_else(
        || {
            let controls = annotation.bounds();
            let origin =
                view.page_to_view(PagePoint::new(controls.x, controls.y));
            canvas::Path::rectangle(
                Point::new(origin.x, origin.y),
                iced::Size::new(
                    controls.width * scale,
                    controls.height * scale,
                ),
            )
        },
        |points| closed_path(&points),
    )
}
