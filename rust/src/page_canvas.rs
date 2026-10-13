use crate::app::Message;
use crate::appearance::{
    PAGE_MARGIN, PAGE_SHADOW_X, PAGE_SHADOW_Y, SELECTION_HANDLE_SIZE,
    SELECTION_STROKE, accent, annotation_color, canvas_background, page_shadow,
};
use crate::picker::ImportedImage;
use fastfill_rust::annotations::{
    AnnotationGeometry, Annotations, DragModifiers, EditHit, InkKind,
    LineEndpoint, LineKind, Pointer, RESIZE_CORNERS, ResizeCorner,
    ShapeAnnotation, ShapeKind, Tool, arrowhead_points,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::{TouchEvent, TouchNavigation};
use iced::touch;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};

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

impl canvas::Program<Message> for PageCanvas<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if state.input_epoch != self.input_epoch {
            *state = CanvasState {
                input_epoch: self.input_epoch,
                ..CanvasState::default()
            };
        }
        if !self.input_enabled {
            return None;
        }
        if let canvas::Event::Touch(event) = event {
            return self.touch_input(state, *event, bounds);
        }
        if self.annotations.tool() != Tool::Navigate
            && state.dragging.is_none()
            && let Some(message) = self.annotation_mouse_input(
                event,
                bounds,
                cursor,
                self.modifiers.square,
            )
        {
            return Some(canvas::Action::publish(message).and_capture());
        }

        let position = cursor.position_in(bounds);
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(button))
                if matches!(
                    button,
                    mouse::Button::Left | mouse::Button::Right
                ) && (*button == mouse::Button::Right
                    || self.annotations.tool() == Tool::Navigate)
                    && position.is_some()
                    && !state.touch.is_active()
                    && self.annotations.active_pointer().is_none() =>
            {
                state.dragging = Some(*button);
                state.last_position = position;
                Some(canvas::Action::capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(button))
                if state.dragging == Some(*button) =>
            {
                state.dragging = None;
                state.last_position = None;
                Some(canvas::Action::capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. })
                if state.dragging.is_some() =>
            {
                let current = cursor.position().map(|point| {
                    Point::new(point.x - bounds.x, point.y - bounds.y)
                })?;
                let previous = state.last_position.replace(current)?;
                Some(
                    canvas::Action::publish(Message::Pan(PagePoint::new(
                        current.x - previous.x,
                        current.y - previous.y,
                    )))
                    .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta })
                if !state.touch.is_active()
                    && state.dragging.is_none()
                    && self.annotations.active_pointer().is_none() =>
            {
                let anchor = position?;
                let factor = wheel_zoom_factor(*delta);
                Some(
                    canvas::Action::publish(Message::ZoomAt {
                        factor,
                        anchor: PagePoint::new(anchor.x, anchor.y),
                        viewport: page_viewport(bounds.size()),
                    })
                    .and_capture(),
                )
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if self.background {
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                canvas_background(),
            );
        }

        if let Some(view) = self
            .navigation
            .view(self.page_size, page_viewport(bounds.size()))
        {
            let page = view.page_rect();
            let size = iced::Size::new(page.width, page.height);
            if self.background {
                frame.fill_rectangle(
                    Point::new(page.x + PAGE_SHADOW_X, page.y + PAGE_SHADOW_Y),
                    size,
                    page_shadow(),
                );
                frame.fill_rectangle(
                    Point::new(page.x, page.y),
                    size,
                    Color::WHITE,
                );
                if let Some(image) = self.image {
                    frame.draw_image(
                        Rectangle::new(Point::new(page.x, page.y), size),
                        &image.handle,
                    );
                }
            } else {
                let scale = page.width / self.page_size.width;
                for (index, annotation) in
                    self.annotations.shapes().iter().enumerate()
                {
                    if self.annotations.edited_index() == Some(index) {
                        continue;
                    }
                    draw_shape(&mut frame, view, annotation, scale);
                }
                if let Some(preview) = self.annotations.preview() {
                    draw_shape(&mut frame, view, &preview, scale);
                }
                if self.annotations.tool() != Tool::Navigate
                    && let Some(annotation) =
                        self.annotations.selection_annotation(scale)
                {
                    draw_selection(&mut frame, view, &annotation, scale);
                }
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if !self.input_enabled || cursor.position_in(bounds).is_none() {
            mouse::Interaction::None
        } else if state.dragging.is_some() || state.touch.is_active() {
            mouse::Interaction::Grabbing
        } else if self.annotations.tool() == Tool::Navigate {
            mouse::Interaction::Grab
        } else if let Some(point) = cursor.position_in(bounds)
            && let Some(point) =
                self.to_page(PagePoint::new(point.x, point.y), bounds)
            && let Some(hit) = self.annotations.hit_selected(
                point,
                Pointer::Mouse,
                self.scale(bounds).unwrap_or(1.0),
                self.modifiers,
            )
        {
            edit_cursor(hit)
        } else if matches!(
            self.annotations.tool(),
            Tool::Rectangle
                | Tool::Oval
                | Tool::Line
                | Tool::Arrow
                | Tool::Checkmark
                | Tool::Cross
                | Tool::Pen
                | Tool::Highlighter
        ) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::None
        }
    }
}

impl PageCanvas<'_> {
    fn to_page(self, local: PagePoint, bounds: Rectangle) -> Option<PagePoint> {
        self.navigation
            .view(self.page_size, page_viewport(bounds.size()))
            .map(|view| view.view_to_page(local))
    }

    fn scale(self, bounds: Rectangle) -> Option<f32> {
        self.navigation
            .view(self.page_size, page_viewport(bounds.size()))
            .map(|view| view.page_rect().width / self.page_size.width)
    }

    fn annotation_mouse_input(
        &self,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        square: bool,
    ) -> Option<Message> {
        let active = self.annotations.active_pointer();
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )) if active.is_none() => {
                let point = cursor.position_in(bounds)?;
                Some(Message::DrawPressed(
                    Pointer::Mouse,
                    self.to_page(PagePoint::new(point.x, point.y), bounds)?,
                    self.scale(bounds)?,
                ))
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { position })
                if active == Some(Pointer::Mouse) =>
            {
                Some(Message::DrawMoved(
                    Pointer::Mouse,
                    self.to_page(local_touch_point(*position, bounds), bounds)?,
                    square,
                ))
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )) if active == Some(Pointer::Mouse) => Some(
                cursor.position().map_or(Message::CancelInteraction, |p| {
                    self.to_page(local_touch_point(p, bounds), bounds).map_or(
                        Message::CancelInteraction,
                        |point| {
                            Message::DrawReleased(Pointer::Mouse, point, square)
                        },
                    )
                }),
            ),
            _ => None,
        }
    }

    fn touch_input(
        &self,
        state: &mut CanvasState,
        event: touch::Event,
        bounds: Rectangle,
    ) -> Option<canvas::Action<Message>> {
        let event = map_touch_event(event, bounds)?;
        if self.annotations.tool() != Tool::Navigate {
            if state.dragging.is_some() {
                return Some(canvas::Action::capture());
            }
            return self
                .annotation_touch_input(event, bounds, self.modifiers.square)
                .map_or(Some(canvas::Action::capture()), |message| {
                    Some(canvas::Action::publish(message).and_capture())
                });
        }
        let was_active = state.touch.is_active();
        if matches!(event, TouchEvent::Pressed(_, _)) {
            state.dragging = None;
            state.last_position = None;
        }
        let delta = state.touch.handle(event, true);
        if let Some(delta) = delta {
            Some(
                canvas::Action::publish(Message::TouchGesture {
                    delta,
                    viewport: page_viewport(bounds.size()),
                })
                .and_capture(),
            )
        } else if was_active || state.touch.is_active() {
            Some(canvas::Action::capture())
        } else {
            None
        }
    }

    fn annotation_touch_input(
        &self,
        event: TouchEvent,
        bounds: Rectangle,
        square: bool,
    ) -> Option<Message> {
        let active = self.annotations.active_pointer();
        match event {
            TouchEvent::Pressed(id, position) if active.is_none() => {
                Some(Message::DrawPressed(
                    Pointer::Touch(id),
                    self.to_page(position, bounds)?,
                    self.scale(bounds)?,
                ))
            }
            TouchEvent::Moved(id, position)
                if active == Some(Pointer::Touch(id)) =>
            {
                Some(Message::DrawMoved(
                    Pointer::Touch(id),
                    self.to_page(position, bounds)?,
                    square,
                ))
            }
            TouchEvent::Released(id, position)
                if active == Some(Pointer::Touch(id)) =>
            {
                Some(Message::DrawReleased(
                    Pointer::Touch(id),
                    self.to_page(position, bounds)?,
                    square,
                ))
            }
            TouchEvent::Lost(id, _) if active == Some(Pointer::Touch(id)) => {
                Some(Message::CancelInteraction)
            }
            _ => None,
        }
    }
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
    let origin = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let width = bounds.width * scale;
    let height = bounds.height * scale;
    let Some((outline, stroke)) = inset_outline(
        Point::new(origin.x, origin.y),
        width,
        height,
        annotation.stroke_width * scale,
    ) else {
        return;
    };
    let half = stroke / 2.0;
    let color = annotation_color(annotation.color);
    let page = view.page_rect();
    let clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    if annotation.filled {
        let shape = Rectangle::new(
            Point::new(origin.x, origin.y),
            iced::Size::new(width, height),
        );
        if let Some(shape) = shape.intersection(&clip) {
            frame.fill(
                &canvas::Path::rectangle(shape.position(), shape.size()),
                color,
            );
        }
        return;
    }
    // Clip stroke bands explicitly: tiny-skia loses nested frame clips.
    for edge in [
        Rectangle::new(
            Point::new(outline.x - half, outline.y - half),
            iced::Size::new(outline.width + stroke, stroke),
        ),
        Rectangle::new(
            Point::new(outline.x - half, outline.y + outline.height - half),
            iced::Size::new(outline.width + stroke, stroke),
        ),
        Rectangle::new(
            Point::new(outline.x - half, outline.y - half),
            iced::Size::new(stroke, outline.height + stroke),
        ),
        Rectangle::new(
            Point::new(outline.x + outline.width - half, outline.y - half),
            iced::Size::new(stroke, outline.height + stroke),
        ),
    ] {
        if let Some(edge) = edge.intersection(&clip) {
            frame.fill(
                &canvas::Path::rectangle(edge.position(), edge.size()),
                color,
            );
        }
    }
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
    let origin = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let width = bounds.width * scale;
    let height = bounds.height * scale;
    let Some((outline, stroke)) = inset_outline(
        Point::new(origin.x, origin.y),
        width,
        height,
        annotation.stroke_width * scale,
    ) else {
        return;
    };
    let half = stroke / 2.0;
    let center = outline.center();
    let page = view.page_rect();
    let page_clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    let color = annotation_color(annotation.color);
    if annotation.filled {
        let center =
            Point::new(origin.x + width / 2.0, origin.y + height / 2.0);
        let polygon = clip_polygon_to_rectangle(
            ellipse_polygon(center, width, height),
            page_clip,
        );
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
        return;
    }
    let clip = Rectangle::new(
        Point::new(page.x + half, page.y + half),
        iced::Size::new(page.width - stroke, page.height - stroke),
    );
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let runs =
        clipped_ellipse_runs(center, outline.width, outline.height, clip);
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
    let origin = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let width = bounds.width * scale;
    let height = bounds.height * scale;
    let stroke = annotation.stroke_width * scale;
    if width <= stroke || height <= stroke {
        return;
    }
    let inset = stroke / 2.0;
    let mark_bounds = Rectangle::new(
        Point::new(origin.x + inset, origin.y + inset),
        iced::Size::new(width - stroke, height - stroke),
    );
    let points = mark_points(kind, mark_bounds);
    let page = view.page_rect();
    let clip = Rectangle::new(
        Point::new(page.x + inset, page.y + inset),
        iced::Size::new(page.width - stroke, page.height - stroke),
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
    let points: Vec<Point> = points
        .iter()
        .map(|point| {
            let point = view.page_to_view(*point);
            Point::new(point.x, point.y)
        })
        .collect();
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

fn inset_outline(
    origin: Point,
    width: f32,
    height: f32,
    requested_stroke: f32,
) -> Option<(Rectangle, f32)> {
    let stroke = requested_stroke.min(width).min(height);
    if stroke <= 0.0 {
        return None;
    }
    let inset = stroke / 2.0;
    Some((
        Rectangle::new(
            Point::new(origin.x + inset, origin.y + inset),
            iced::Size::new(width - stroke, height - stroke),
        ),
        stroke,
    ))
}

fn ellipse_point(center: Point, width: f32, height: f32, angle: f32) -> Point {
    Point::new(
        center.x + width / 2.0 * angle.cos(),
        center.y + height / 2.0 * angle.sin(),
    )
}

fn ellipse_polygon(center: Point, width: f32, height: f32) -> Vec<Point> {
    const SEGMENTS: u16 = 128;

    let step = std::f32::consts::TAU / f32::from(SEGMENTS);
    (0..SEGMENTS)
        .map(|index| {
            ellipse_point(center, width, height, step * f32::from(index))
        })
        .collect()
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

fn clipped_ellipse_runs(
    center: Point,
    width: f32,
    height: f32,
    clip: Rectangle,
) -> Vec<Vec<Point>> {
    const SEGMENTS: u16 = 128;

    let segment_count = usize::from(SEGMENTS);
    let step = std::f32::consts::TAU / f32::from(SEGMENTS);
    let first = ellipse_point(center, width, height, 0.0);
    let mut previous = first;
    let mut segments = Vec::with_capacity(segment_count);
    for index in 1..=SEGMENTS {
        let current = if index == SEGMENTS {
            first
        } else {
            ellipse_point(center, width, height, step * f32::from(index))
        };
        segments.push(
            clip_segment(previous, current, clip)
                .filter(|(start, end)| !same_point(*start, *end)),
        );
        previous = current;
    }

    let Some(gap) = segments.iter().position(Option::is_none) else {
        let mut run = Vec::with_capacity(segment_count + 1);
        run.push(segments[0].unwrap().0);
        run.extend(segments.into_iter().map(|segment| segment.unwrap().1));
        return vec![run];
    };

    let mut runs = Vec::new();
    let mut run = Vec::new();
    for offset in 1..=segment_count {
        match segments[(gap + offset) % segment_count] {
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

fn edit_cursor(hit: EditHit) -> mouse::Interaction {
    match hit {
        EditHit::Move => mouse::Interaction::Move,
        EditHit::Resize(ResizeCorner::TopLeft | ResizeCorner::BottomRight) => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        EditHit::Resize(ResizeCorner::TopRight | ResizeCorner::BottomLeft) => {
            mouse::Interaction::ResizingDiagonallyUp
        }
        EditHit::Endpoint(LineEndpoint::Start | LineEndpoint::End) => {
            mouse::Interaction::Crosshair
        }
    }
}

fn draw_selection(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: &ShapeAnnotation,
    scale: f32,
) {
    let blue = accent();
    let controls = annotation.bounds();
    let origin = view.page_to_view(PagePoint::new(controls.x, controls.y));
    let path = canvas::Path::rectangle(
        Point::new(origin.x, origin.y),
        iced::Size::new(controls.width * scale, controls.height * scale),
    );
    frame.stroke(&path, canvas::Stroke::default().with_color(blue));
    let handles: Vec<Point> = match &annotation.geometry {
        AnnotationGeometry::Bounded { .. } | AnnotationGeometry::Ink { .. } => {
            RESIZE_CORNERS
                .into_iter()
                .map(|corner| {
                    let point = view.page_to_view(corner.position(controls));
                    Point::new(point.x, point.y)
                })
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

pub(crate) fn page_viewport(size: iced::Size) -> PageRect {
    PageRect::new(
        PAGE_MARGIN,
        PAGE_MARGIN,
        (size.width - PAGE_MARGIN * 2.0).max(0.0),
        (size.height - PAGE_MARGIN * 2.0).max(0.0),
    )
}

fn wheel_zoom_factor(delta: mouse::ScrollDelta) -> f32 {
    let steps = match delta {
        mouse::ScrollDelta::Lines { y, .. } => y,
        mouse::ScrollDelta::Pixels { y, .. } => y / 120.0,
    };
    1.2_f32.powf(steps)
}

fn map_touch_event(
    event: touch::Event,
    bounds: Rectangle,
) -> Option<TouchEvent> {
    match event {
        touch::Event::FingerPressed { id, position } => {
            bounds.contains(position).then(|| {
                TouchEvent::Pressed(id.0, local_touch_point(position, bounds))
            })
        }
        touch::Event::FingerMoved { id, position } => {
            Some(TouchEvent::Moved(id.0, local_touch_point(position, bounds)))
        }
        touch::Event::FingerLifted { id, position } => Some(
            TouchEvent::Released(id.0, local_touch_point(position, bounds)),
        ),
        touch::Event::FingerLost { id, position } => {
            Some(TouchEvent::Lost(id.0, local_touch_point(position, bounds)))
        }
    }
}

fn local_touch_point(position: Point, bounds: Rectangle) -> PagePoint {
    PagePoint::new(position.x - bounds.x, position.y - bounds.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_clipping_handles_crossing_inside_and_outside_lines() {
        let clip =
            Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(30.0, 40.0));
        assert_eq!(
            clip_segment(Point::new(0.0, 30.0), Point::new(50.0, 30.0), clip),
            Some((Point::new(10.0, 30.0), Point::new(40.0, 30.0))),
        );
        assert_eq!(
            clip_segment(Point::new(20.0, 30.0), Point::new(30.0, 40.0), clip),
            Some((Point::new(20.0, 30.0), Point::new(30.0, 40.0))),
        );
        assert_eq!(
            clip_segment(Point::new(0.0, 10.0), Point::new(50.0, 10.0), clip),
            None,
        );
    }

    #[test]
    fn polyline_clipping_keeps_every_run_inside_the_page() {
        let clip =
            Rectangle::new(Point::new(10.0, 20.0), iced::Size::new(30.0, 40.0));
        let points = [
            Point::new(0.0, 30.0),
            Point::new(20.0, 30.0),
            Point::new(50.0, 50.0),
            Point::new(20.0, 70.0),
        ];
        let runs = clipped_polyline_runs(&points, clip);

        assert_eq!(runs.len(), 2);
        assert!(runs.iter().flatten().all(|point| {
            point.x >= clip.x
                && point.x <= clip.x + clip.width
                && point.y >= clip.y
                && point.y <= clip.y + clip.height
        }));
    }

    #[test]
    fn closed_polyline_keeps_its_start_as_a_join_not_two_caps() {
        let clip =
            Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(100.0, 100.0));
        let points = [
            Point::new(10.0, 10.0),
            Point::new(90.0, 10.0),
            Point::new(50.0, 90.0),
        ];
        let runs = clipped_closed_polyline_runs(&points, clip);

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].first(), runs[0].last());
    }

    #[test]
    fn arrowhead_dimensions_do_not_depend_on_direction_or_box_aspect() {
        let end = PagePoint::new(200.0, 200.0);
        let horizontal =
            arrowhead_points(PagePoint::new(0.0, 200.0), end).unwrap();
        let diagonal_offset = 200.0 / 2.0_f32.sqrt();
        let diagonal = arrowhead_points(
            PagePoint::new(end.x - diagonal_offset, end.y - diagonal_offset),
            end,
        )
        .unwrap();
        for (horizontal, diagonal) in [horizontal.0, horizontal.1]
            .into_iter()
            .zip([diagonal.0, diagonal.1])
        {
            let horizontal = (horizontal.x - end.x).hypot(horizontal.y - end.y);
            let diagonal = (diagonal.x - end.x).hypot(diagonal.y - end.y);
            assert!((horizontal - diagonal).abs() < 0.001);
        }
    }

    #[test]
    fn mark_points_fill_their_selection_bounds() {
        let bounds = Rectangle::new(
            Point::new(10.0, 20.0),
            iced::Size::new(100.0, 80.0),
        );
        let check = mark_points(ShapeKind::Checkmark, bounds);
        assert_eq!(check[0], Point::new(10.0, 56.0));
        assert_eq!(check[1], Point::new(46.0, 100.0));
        assert_eq!(check[2], Point::new(110.0, 20.0));

        let cross = mark_points(ShapeKind::Cross, bounds);
        assert_eq!(cross[0], Point::new(10.0, 20.0));
        assert_eq!(cross[1], Point::new(110.0, 100.0));
        assert_eq!(cross[2], Point::new(110.0, 20.0));
        assert_eq!(cross[3], Point::new(10.0, 100.0));
    }

    #[test]
    fn bounded_outlines_fit_inside_their_selection_frame() {
        let origin = Point::new(10.0, 20.0);
        let (outline, stroke) =
            inset_outline(origin, 100.0, 80.0, 6.0).expect("ordinary outline");
        assert_eq!(stroke, 6.0);
        assert_eq!(outline.x - stroke / 2.0, origin.x);
        assert_eq!(outline.y - stroke / 2.0, origin.y);
        assert_eq!(outline.x + outline.width + stroke / 2.0, 110.0);
        assert_eq!(outline.y + outline.height + stroke / 2.0, 100.0);

        let (tiny, stroke) =
            inset_outline(origin, 2.0, 8.0, 6.0).expect("tiny outline");
        assert_eq!(stroke, 2.0);
        assert_eq!(
            tiny,
            Rectangle::new(Point::new(11.0, 21.0), iced::Size::new(0.0, 6.0),)
        );
    }

    #[test]
    fn oval_segments_form_continuous_visible_runs() {
        let center = Point::new(50.0, 50.0);
        let full = clipped_ellipse_runs(
            center,
            80.0,
            40.0,
            Rectangle::new(Point::ORIGIN, iced::Size::new(100.0, 100.0)),
        );
        assert_eq!(full.len(), 1);
        assert!(same_point(full[0][0], *full[0].last().unwrap()));

        let clipped = clipped_ellipse_runs(
            center,
            80.0,
            40.0,
            Rectangle::new(Point::new(50.0, 0.0), iced::Size::new(50.0, 100.0)),
        );
        assert_eq!(clipped.len(), 1);
        assert!(!same_point(clipped[0][0], *clipped[0].last().unwrap()));
        assert!(
            clipped[0]
                .windows(2)
                .all(|pair| !same_point(pair[0], pair[1]))
        );
    }

    #[test]
    fn filled_polygon_clipping_keeps_every_point_inside_the_page() {
        let clip =
            Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(10.0, 10.0));
        let polygon = clip_polygon_to_rectangle(
            vec![
                Point::new(-5.0, -5.0),
                Point::new(15.0, -5.0),
                Point::new(15.0, 15.0),
                Point::new(-5.0, 15.0),
            ],
            clip,
        );

        assert_eq!(polygon.len(), 4);
        assert!(polygon.into_iter().all(|point| {
            point.x >= clip.x
                && point.x <= clip.x + clip.width
                && point.y >= clip.y
                && point.y <= clip.y + clip.height
        }));
    }
}
