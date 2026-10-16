use super::{
    CanvasLayer, CanvasState, PageCanvas, draw_annotation, draw_group_member,
    draw_group_selection, draw_marquee, draw_selection, draw_shape, draw_text,
    draw_text_group_member, draw_text_selection, draw_text_underline,
    page_viewport,
};
use crate::app::Message;
use crate::appearance::{
    PAGE_SHADOW_X, PAGE_SHADOW_Y, canvas_background, page_shadow,
};
use fastfill_rust::annotations::{
    AnnotationRef, EditHit, LineEndpoint, Pointer, ResizeCorner, TextEditHit,
    Tool,
};
use fastfill_rust::geometry::Point as PagePoint;
use fastfill_rust::interaction::TouchEvent;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse, touch};

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
        if self.annotations.tool().handles_annotations()
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
                    || !self.annotations.tool().handles_annotations())
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
        if matches!(self.layer, CanvasLayer::Background) {
            frame.fill_rectangle(
                Point::ORIGIN,
                bounds.size(),
                canvas_background(),
            );
        }

        let Some(view) = self
            .navigation
            .view(self.page_size, page_viewport(bounds.size()))
        else {
            return vec![frame.into_geometry()];
        };
        let view_scale = view.page_rect().width / self.page_size.width;
        match self.layer {
            CanvasLayer::Annotation(annotation) => {
                match annotation {
                    AnnotationRef::Shape(index, _)
                        if !self.annotations.is_edited(index) =>
                    {
                        draw_annotation(
                            &mut frame, view, annotation, view_scale,
                        );
                    }
                    AnnotationRef::Text(index, _)
                        if !self.annotations.is_text_edited(index) =>
                    {
                        draw_annotation(
                            &mut frame, view, annotation, view_scale,
                        );
                    }
                    AnnotationRef::Shape(_, _) | AnnotationRef::Text(_, _) => {}
                }
                return vec![frame.into_geometry()];
            }
            CanvasLayer::Overlay => {
                self.draw_overlay(&mut frame, view, view_scale);
                return vec![frame.into_geometry()];
            }
            CanvasLayer::Background => {}
        }
        let page = view.page_rect();
        let size = iced::Size::new(page.width, page.height);
        frame.fill_rectangle(
            Point::new(page.x + PAGE_SHADOW_X, page.y + PAGE_SHADOW_Y),
            size,
            page_shadow(),
        );
        frame.fill_rectangle(Point::new(page.x, page.y), size, Color::WHITE);
        if let Some(image) = self.image {
            frame.draw_image(
                Rectangle::new(Point::new(page.x, page.y), size),
                image.display_handle(),
            );
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
        } else if !self.annotations.tool().handles_annotations() {
            mouse::Interaction::Grab
        } else if self.annotations.tool() == Tool::Select
            && (self.annotations.is_selecting()
                || self.modifiers.square
                    && self.annotations.active_pointer().is_none()
                    && !self.modifiers.force_move
                    && !self.modifiers.force_resize)
        {
            mouse::Interaction::Crosshair
        } else if self.annotations.tool() == Tool::Select
            && let Some(point) = cursor.position_in(bounds)
            && let Some(point) =
                self.to_page(PagePoint::new(point.x, point.y), bounds)
            && let Some(hit) = self.annotations.hit_text_interaction(
                point,
                Pointer::Mouse,
                self.scale(bounds).unwrap_or(1.0),
            )
        {
            match hit {
                TextEditHit::Move => mouse::Interaction::Move,
                TextEditHit::Rotate => mouse::Interaction::Crosshair,
            }
        } else if let Some(point) = cursor.position_in(bounds)
            && let Some(point) =
                self.to_page(PagePoint::new(point.x, point.y), bounds)
            && self.annotations.tool() == Tool::Text
            && self.annotations.text_hit(point).is_some()
        {
            mouse::Interaction::Text
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
        } else if self.annotations.tool().creates_annotations()
            || self.annotations.tool() == Tool::Select
        {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::None
        }
    }
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
        EditHit::Rotate => mouse::Interaction::Crosshair,
    }
}

impl PageCanvas<'_> {
    fn draw_overlay(
        self,
        frame: &mut canvas::Frame,
        view: fastfill_rust::geometry::PageView,
        scale: f32,
    ) {
        for preview in self.annotations.previews() {
            draw_shape(frame, view, &preview, scale);
        }
        for preview in self.annotations.text_move_previews() {
            draw_text(frame, view, &preview, scale);
        }
        if let Some(mut preview) = self.annotations.text_preview() {
            // The editor stays horizontal even for rotated text.
            preview.rotation = 0.0;
            draw_text_underline(frame, view, &preview, scale);
        }
        self.draw_selection_overlay(frame, view, scale);
    }

    fn draw_selection_overlay(
        self,
        frame: &mut canvas::Frame,
        view: fastfill_rust::geometry::PageView,
        scale: f32,
    ) {
        if !self.annotations.tool().handles_annotations() {
            return;
        }
        if let Some(annotation) = self.annotations.selection_annotation(scale) {
            draw_selection(frame, view, &annotation, scale);
        } else if self.annotations.selected_count() > 1
            && let Some(bounds) = self.annotations.selection_bounds(scale)
        {
            for annotation in self.annotations.selection_annotations(scale) {
                draw_group_member(frame, view, &annotation, scale);
            }
            if self.annotations.selected_count() > 1 {
                for text in self.annotations.selection_texts() {
                    draw_text_group_member(frame, view, &text);
                }
            }
            draw_group_selection(frame, view, bounds);
        }
        if let Some(bounds) = self.annotations.selection_box() {
            draw_marquee(frame, view, bounds);
        }
        if !self.annotations.has_text_edit()
            && let Some(annotation) =
                self.annotations.selected_text_annotation()
        {
            draw_text_selection(frame, view, &annotation, scale);
        }
    }

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
        if self.annotations.tool().handles_annotations() {
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
