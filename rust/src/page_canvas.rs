use crate::app::Message;
use crate::picker::ImportedImage;
use fastfill_rust::annotations::{
    AnnotationGeometry, Annotations, DragModifiers, EditHit, LineEndpoint,
    LineKind, Pointer, RESIZE_CORNERS, ResizeCorner, ShapeAnnotation,
    ShapeKind, Tool, arrowhead_points,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::{TouchEvent, TouchNavigation};
use iced::touch;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};

const PAGE_MARGIN: f32 = 36.0;

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
                Color::from_rgb8(67, 72, 78),
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
                    Point::new(page.x + 5.0, page.y + 7.0),
                    size,
                    Color::from_rgba8(0, 0, 0, 0.22),
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
                    draw_shape(&mut frame, view, *annotation, scale, false);
                }
                if let Some(preview) = self.annotations.preview() {
                    draw_shape(&mut frame, view, preview, scale, true);
                }
                if self.annotations.tool() != Tool::Navigate
                    && let Some(annotation) =
                        self.annotations.selection_annotation(scale)
                {
                    draw_selection(&mut frame, view, annotation, scale);
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
            Tool::Rectangle | Tool::Oval | Tool::Line | Tool::Arrow
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
    annotation: ShapeAnnotation,
    scale: f32,
    preview: bool,
) {
    match annotation.geometry {
        AnnotationGeometry::Bounded {
            kind: ShapeKind::Rectangle,
            ..
        } => {
            draw_rectangle(frame, view, annotation, scale, preview);
        }
        AnnotationGeometry::Bounded {
            kind: ShapeKind::Oval,
            ..
        } => draw_oval(frame, view, annotation, scale, preview),
        AnnotationGeometry::Linear { .. } => {
            draw_linear(frame, view, annotation, scale, preview);
        }
    }
}

fn draw_rectangle(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: ShapeAnnotation,
    scale: f32,
    preview: bool,
) {
    let bounds = annotation.bounds();
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    let origin = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let width = bounds.width * scale;
    let height = bounds.height * scale;
    let stroke = annotation.stroke_width * scale;
    let half = stroke / 2.0;
    let color = if preview {
        Color::from_rgb8(0, 102, 204)
    } else {
        Color::BLACK
    };
    let page = view.page_rect();
    let clip = Rectangle::new(
        Point::new(page.x, page.y),
        iced::Size::new(page.width, page.height),
    );
    // Clip stroke bands explicitly: tiny-skia loses nested frame clips.
    for edge in [
        Rectangle::new(
            Point::new(origin.x - half, origin.y - half),
            iced::Size::new(width + stroke, stroke),
        ),
        Rectangle::new(
            Point::new(origin.x - half, origin.y + height - half),
            iced::Size::new(width + stroke, stroke),
        ),
        Rectangle::new(
            Point::new(origin.x - half, origin.y - half),
            iced::Size::new(stroke, height + stroke),
        ),
        Rectangle::new(
            Point::new(origin.x + width - half, origin.y - half),
            iced::Size::new(stroke, height + stroke),
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
    annotation: ShapeAnnotation,
    scale: f32,
    preview: bool,
) {
    const SEGMENTS: u16 = 128;

    let bounds = annotation.bounds();
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return;
    }
    let origin = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let width = bounds.width * scale;
    let height = bounds.height * scale;
    let stroke = annotation.stroke_width * scale;
    let half = stroke / 2.0;
    let center = Point::new(origin.x + width / 2.0, origin.y + height / 2.0);
    let page = view.page_rect();
    let clip = Rectangle::new(
        Point::new(page.x + half, page.y + half),
        iced::Size::new(page.width - stroke, page.height - stroke),
    );
    if clip.width <= 0.0 || clip.height <= 0.0 {
        return;
    }
    let step = std::f32::consts::TAU / f32::from(SEGMENTS);
    let path = canvas::Path::new(|builder| {
        let mut angle: f32 = 0.0;
        let mut previous = ellipse_point(center, width, height, angle);
        for _ in 0..SEGMENTS {
            angle += step;
            let current = ellipse_point(center, width, height, angle);
            if let Some((start, end)) = clip_segment(previous, current, clip) {
                builder.move_to(start);
                builder.line_to(end);
            }
            previous = current;
        }
    });
    let color = if preview {
        Color::from_rgb8(0, 102, 204)
    } else {
        Color::BLACK
    };
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke),
    );
}

fn draw_linear(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: ShapeAnnotation,
    scale: f32,
    preview: bool,
) {
    let AnnotationGeometry::Linear { kind, start, end } = annotation.geometry
    else {
        return;
    };
    let arrowhead = (kind == LineKind::Arrow)
        .then(|| arrowhead_points(start, end, annotation.stroke_width))
        .flatten()
        .map(|(left, right)| {
            let left = view.page_to_view(left);
            let right = view.page_to_view(right);
            (Point::new(left.x, left.y), Point::new(right.x, right.y))
        });
    let start = view.page_to_view(start);
    let end = view.page_to_view(end);
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
        let mut add_segment = |start, end| {
            if let Some((start, end)) = clip_segment(start, end, clip) {
                builder.move_to(start);
                builder.line_to(end);
            }
        };
        add_segment(start, end);
        if let Some((left, right)) = arrowhead {
            add_segment(end, left);
            add_segment(end, right);
        }
    });
    let color = if preview {
        Color::from_rgb8(0, 102, 204)
    } else {
        Color::BLACK
    };
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke),
    );
}

fn ellipse_point(center: Point, width: f32, height: f32, angle: f32) -> Point {
    Point::new(
        center.x + width / 2.0 * angle.cos(),
        center.y + height / 2.0 * angle.sin(),
    )
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
    annotation: ShapeAnnotation,
    scale: f32,
) {
    let blue = Color::from_rgb8(0, 102, 204);
    let controls = annotation.bounds();
    let origin = view.page_to_view(PagePoint::new(controls.x, controls.y));
    let path = canvas::Path::rectangle(
        Point::new(origin.x, origin.y),
        iced::Size::new(controls.width * scale, controls.height * scale),
    );
    frame.stroke(&path, canvas::Stroke::default().with_color(blue));
    let handles: Vec<Point> = match annotation.geometry {
        AnnotationGeometry::Bounded { .. } => RESIZE_CORNERS
            .into_iter()
            .map(|corner| {
                let point = view.page_to_view(corner.position(controls));
                Point::new(point.x, point.y)
            })
            .collect(),
        AnnotationGeometry::Linear { start, end, .. } => [start, end]
            .into_iter()
            .map(|point| {
                let point = view.page_to_view(point);
                Point::new(point.x, point.y)
            })
            .collect(),
    };
    for center in handles {
        let path = canvas::Path::rectangle(
            Point::new(center.x - 7.0, center.y - 7.0),
            iced::Size::new(14.0, 14.0),
        );
        frame.fill(&path, Color::WHITE);
        frame.stroke(
            &path,
            canvas::Stroke::default().with_color(blue).with_width(2.0),
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
    fn arrowhead_dimensions_do_not_depend_on_direction_or_box_aspect() {
        let end = PagePoint::new(200.0, 200.0);
        let horizontal =
            arrowhead_points(PagePoint::new(0.0, 200.0), end, 3.0).unwrap();
        let diagonal_offset = 200.0 / 2.0_f32.sqrt();
        let diagonal = arrowhead_points(
            PagePoint::new(end.x - diagonal_offset, end.y - diagonal_offset),
            end,
            3.0,
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
}
