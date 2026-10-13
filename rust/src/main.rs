use fastfill_rust::annotations::{
    Annotations, DragModifiers, EditHit, Pointer, RESIZE_CORNERS, ResizeCorner,
    Tool,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::image_input::ImageInfo;
#[cfg(not(target_arch = "wasm32"))]
use fastfill_rust::image_input::MAX_IMAGE_BYTES;
use fastfill_rust::image_input::{validate_image, validate_image_length};
use fastfill_rust::interaction::{GestureDelta, TouchEvent, TouchNavigation};
use iced::touch;
use iced::widget::image::Handle;
use iced::widget::{button, canvas, column, container, responsive, row, text};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Task, Theme};
use iced::{keyboard, mouse};

const DEFAULT_PAGE: PageSize = PageSize::new(794.0, 1_123.0);
const PAGE_MARGIN: f32 = 36.0;
const TOOLBAR_HEIGHT: f32 = 112.0;

#[derive(Default)]
struct FastFill {
    page: Option<ImportedImage>,
    navigation: Navigation,
    annotations: Annotations,
    modifiers: DragModifiers,
    input_epoch: u64,
    image_picker: ImagePicker,
    status: String,
    status_is_error: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ImagePicker {
    #[default]
    Idle,
    ConfirmReplace,
    Picking,
}

#[derive(Clone, Debug)]
struct ImportedImage {
    handle: Handle,
    name: String,
    size: PageSize,
}

#[derive(Clone, Debug)]
enum Message {
    OpenImage,
    ReplacePage,
    KeepPage,
    ImagePicked(Result<Option<ImportedImage>, String>),
    SetTool(Tool),
    DeleteSelected,
    DrawPressed(Pointer, PagePoint, f32),
    DrawMoved(Pointer, PagePoint, bool),
    DrawReleased(Pointer, PagePoint, bool),
    ModifiersChanged(DragModifiers),
    CancelInteraction,
    FocusLost,
    Fit,
    OneToOne(PageRect),
    Pan(PagePoint),
    ZoomAt {
        factor: f32,
        anchor: PagePoint,
        viewport: PageRect,
    },
    TouchGesture {
        delta: GestureDelta,
        viewport: PageRect,
    },
}

#[derive(Clone, Copy, Debug)]
struct PageCanvas<'a> {
    image: Option<&'a ImportedImage>,
    navigation: Navigation,
    page_size: PageSize,
    annotations: &'a Annotations,
    input_epoch: u64,
    input_enabled: bool,
    modifiers: DragModifiers,
    background: bool,
}

#[derive(Default)]
struct CanvasState {
    dragging: Option<mouse::Button>,
    last_position: Option<Point>,
    touch: TouchNavigation,
    input_epoch: u64,
}

fn main() -> iced::Result {
    iced::application(FastFill::default, update, view)
        .title("FastFill Rust prototype")
        .window_size((1_200.0, 800.0))
        .antialiasing(true)
        .subscription(|_| {
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Window(iced::window::Event::Unfocused) => {
                    Some(Message::FocusLost)
                }
                iced::Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                    Some(Message::ModifiersChanged(DragModifiers {
                        square: m.control(),
                        force_move: m.shift(),
                        force_resize: m.alt(),
                    }))
                }
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Delete),
                    ..
                }) => Some(Message::DeleteSelected),
                iced::Event::Window(iced::window::Event::Resized(_))
                | iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::CancelInteraction),
                _ => None,
            })
        })
        .run()
}

impl FastFill {
    fn cancel_interaction(&mut self) {
        self.annotations.cancel();
        self.input_epoch = self.input_epoch.wrapping_add(1);
    }

    fn pick_image(&mut self) -> Task<Message> {
        self.image_picker = ImagePicker::Picking;
        self.status = "Selecting an image…".into();
        self.status_is_error = false;
        Task::perform(pick_image(), Message::ImagePicked)
    }

    fn image_picked(&mut self, result: Result<Option<ImportedImage>, String>) {
        self.image_picker = ImagePicker::Idle;
        match result {
            Ok(Some(image)) => {
                self.cancel_interaction();
                self.status = format!(
                    "Opened {} ({} × {} px)",
                    image.name, image.size.width, image.size.height,
                );
                self.page = Some(image);
                self.navigation.fit();
                let tool = self.annotations.tool();
                self.annotations = Annotations::default();
                self.annotations.set_tool(tool);
                self.status_is_error = false;
            }
            Ok(None) => {
                self.status = "Open canceled.".into();
                self.status_is_error = false;
            }
            Err(error) => {
                self.status = error;
                self.status_is_error = true;
            }
        }
    }
}

fn update(state: &mut FastFill, message: Message) -> Task<Message> {
    match message {
        Message::OpenImage => {
            if state.image_picker == ImagePicker::Idle {
                state.cancel_interaction();
                if state.annotations.rectangles().is_empty() {
                    return state.pick_image();
                }
                state.image_picker = ImagePicker::ConfirmReplace;
            }
        }
        Message::ReplacePage => {
            if state.image_picker == ImagePicker::ConfirmReplace {
                return state.pick_image();
            }
        }
        Message::KeepPage => state.image_picker = ImagePicker::Idle,
        Message::ImagePicked(result) => state.image_picked(result),
        Message::SetTool(tool) => {
            state.cancel_interaction();
            state.annotations.set_tool(tool);
        }
        Message::DeleteSelected => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations.delete_selected();
            }
        }
        Message::DrawPressed(pointer, point, scale) => {
            if state.image_picker == ImagePicker::Idle {
                state.annotations.begin_interaction(
                    pointer,
                    point,
                    page_size(state),
                    scale,
                    state.modifiers,
                );
            }
        }
        Message::DrawMoved(pointer, point, square) => {
            state.annotations.drag(pointer, point, square);
        }
        Message::DrawReleased(pointer, point, square) => {
            state.annotations.finish(pointer, point, square);
        }
        Message::ModifiersChanged(modifiers) => {
            state.modifiers = modifiers;
            state.annotations.constrain_square(modifiers.square);
        }
        Message::CancelInteraction => {
            state.cancel_interaction();
            if state.image_picker == ImagePicker::ConfirmReplace {
                state.image_picker = ImagePicker::Idle;
            }
        }
        Message::FocusLost => {
            state.cancel_interaction();
            state.modifiers = DragModifiers::default();
        }
        Message::Fit => {
            state.cancel_interaction();
            state.navigation.fit();
        }
        Message::OneToOne(viewport) => {
            state.cancel_interaction();
            state.navigation.one_to_one(page_size(state), viewport);
        }
        Message::Pan(delta) => {
            state.annotations.cancel();
            state.navigation.pan_by(delta);
        }
        Message::ZoomAt {
            factor,
            anchor,
            viewport,
        } => {
            state.annotations.cancel();
            state.navigation.zoom_at(
                factor,
                anchor,
                page_size(state),
                viewport,
            );
        }
        Message::TouchGesture { delta, viewport } => {
            state.annotations.cancel();
            state.navigation.zoom_at(
                delta.zoom_factor,
                delta.zoom_anchor,
                page_size(state),
                viewport,
            );
            state.navigation.pan_by(delta.pan);
        }
    }

    Task::none()
}

fn heading(state: &FastFill) -> Element<'_, Message> {
    let status = if state.status.is_empty() {
        "Open a PNG or JPEG, or draw on the blank page."
    } else {
        &state.status
    };
    let status_line: Element<'_, Message> = if state.status_is_error {
        container(text(format!("⚠ {status}")).size(14))
            .style(container::danger)
            .padding([6, 10])
            .into()
    } else {
        text(status).size(14).into()
    };
    let mut heading =
        column![text("FastFill").size(24), status_line].spacing(4);
    if state.image_picker == ImagePicker::ConfirmReplace {
        heading = heading.push(
            container(
                row![
                    text("Replace page and discard its rectangles?")
                        .width(Fill),
                    button("Keep page").on_press(Message::KeepPage),
                    button("Replace").on_press(Message::ReplacePage),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(8),
            )
            .padding(8)
            .style(container::warning),
        );
    }
    container(heading).padding([10, 20]).into()
}

fn view(state: &FastFill) -> Element<'_, Message> {
    let body = responsive(move |available| {
        let canvas_size = iced::Size::new(
            available.width,
            (available.height - TOOLBAR_HEIGHT).max(1.0),
        );
        let viewport = page_viewport(canvas_size);
        let center = viewport.center();
        let zoom_out = Message::ZoomAt {
            factor: 0.8,
            anchor: center,
            viewport,
        };
        let zoom_in = Message::ZoomAt {
            factor: 1.25,
            anchor: center,
            viewport,
        };
        let controls = row![
            button("Open image")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle)
                        .then_some(Message::OpenImage),
                )
                .height(40),
            button("−").on_press(zoom_out).height(40),
            button("+").on_press(zoom_in).height(40),
            button("1:1")
                .on_press(Message::OneToOne(viewport))
                .height(40),
            button("Fit").on_press(Message::Fit).height(40),
        ]
        .align_y(iced::Alignment::Center)
        .height(TOOLBAR_HEIGHT / 2.0)
        .padding([8, 12])
        .spacing(8);
        let mut tools = row![
            tool_button("Navigate", Tool::Navigate, state.annotations.tool()),
            tool_button("Select", Tool::Select, state.annotations.tool()),
            tool_button("Rectangle", Tool::Rectangle, state.annotations.tool()),
            button("Delete")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && state.annotations.selected_index().is_some()
                        && state.annotations.active_pointer().is_none())
                    .then_some(Message::DeleteSelected),
                )
                .height(40),
        ];
        if available.width >= 520.0 {
            tools = tools.push(
                text(format!(
                    "{} rectangles",
                    state.annotations.rectangles().len(),
                ))
                .size(14),
            );
        }
        let tools = tools
            .align_y(iced::Alignment::Center)
            .height(TOOLBAR_HEIGHT / 2.0)
            .padding([8, 12])
            .spacing(8);
        column![controls, tools, document(state)]
            .width(Fill)
            .height(Fill)
            .into()
    });

    column![heading(state), body]
        .width(Fill)
        .height(Fill)
        .into()
}

fn document(state: &FastFill) -> Element<'_, Message> {
    let page = PageCanvas {
        image: state.page.as_ref(),
        navigation: state.navigation,
        page_size: page_size(state),
        annotations: &state.annotations,
        input_epoch: state.input_epoch,
        input_enabled: state.image_picker == ImagePicker::Idle,
        modifiers: state.modifiers,
        background: false,
    };
    // iced batches images above paths within one layer. Stack the overlay.
    let background = PageCanvas {
        background: true,
        input_enabled: false,
        ..page
    };
    iced::widget::stack![
        canvas(background).width(Fill).height(Fill),
        canvas(page).width(Fill).height(Fill),
    ]
    .into()
}

fn tool_button(
    label: &str,
    tool: Tool,
    selected: Tool,
) -> iced::widget::Button<'_, Message> {
    button(text(label))
        .on_press(Message::SetTool(tool))
        .style(if tool == selected {
            button::primary
        } else {
            button::secondary
        })
        .height(40)
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
                    self.annotations.rectangles().iter().enumerate()
                {
                    if self.annotations.edited_index() == Some(index) {
                        continue;
                    }
                    draw_rectangle(&mut frame, view, *annotation, scale, false);
                }
                if let Some(preview) = self.annotations.preview() {
                    draw_rectangle(&mut frame, view, preview, scale, true);
                }
                if self.annotations.tool() != Tool::Navigate
                    && let Some(controls) =
                        self.annotations.selection_bounds(scale)
                {
                    draw_selection(&mut frame, view, controls, scale);
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
        } else if self.annotations.tool() == Tool::Rectangle {
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

fn draw_rectangle(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    annotation: fastfill_rust::annotations::RectangleAnnotation,
    scale: f32,
    preview: bool,
) {
    let bounds = annotation.bounds;
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

fn edit_cursor(hit: EditHit) -> mouse::Interaction {
    match hit {
        EditHit::Move => mouse::Interaction::Move,
        EditHit::Resize(ResizeCorner::TopLeft | ResizeCorner::BottomRight) => {
            mouse::Interaction::ResizingDiagonallyDown
        }
        EditHit::Resize(ResizeCorner::TopRight | ResizeCorner::BottomLeft) => {
            mouse::Interaction::ResizingDiagonallyUp
        }
    }
}

fn draw_selection(
    frame: &mut canvas::Frame,
    view: fastfill_rust::geometry::PageView,
    controls: PageRect,
    scale: f32,
) {
    let blue = Color::from_rgb8(0, 102, 204);
    let origin = view.page_to_view(PagePoint::new(controls.x, controls.y));
    let path = canvas::Path::rectangle(
        Point::new(origin.x, origin.y),
        iced::Size::new(controls.width * scale, controls.height * scale),
    );
    frame.stroke(&path, canvas::Stroke::default().with_color(blue));
    for corner in RESIZE_CORNERS {
        let center = view.page_to_view(corner.position(controls));
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

fn page_size(state: &FastFill) -> PageSize {
    state.page.as_ref().map_or(DEFAULT_PAGE, |page| page.size)
}

fn page_viewport(size: iced::Size) -> PageRect {
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

async fn pick_image() -> Result<Option<ImportedImage>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Open image")
        .add_filter("Image", &["png", "jpg", "jpeg"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let length = std::fs::metadata(file.path())
            .map_err(|error| format!("Could not inspect image: {error}"))?
            .len();
        if length > MAX_IMAGE_BYTES as u64 {
            return Err("The image file is larger than 100 MiB.".into());
        }
    }

    let name = file.file_name();
    let bytes = file.read().await;
    validate_image_length(bytes.len()).map_err(|error| error.to_string())?;
    let info = validate_image(&bytes).map_err(|error| error.to_string())?;

    Ok(Some(ImportedImage {
        handle: Handle::from_bytes(bytes),
        name,
        size: page_size_from_pixels(info),
    }))
}

#[allow(clippy::cast_precision_loss)]
fn page_size_from_pixels(info: ImageInfo) -> PageSize {
    // iced uses f32 coordinates; integer precision above 16 MP is immaterial.
    PageSize::new(info.width as f32, info.height as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_drag_is_pending_escape_reverts_and_delete_removes_selection() {
        let mut state = annotated_page();
        let _ = update(&mut state, Message::SetTool(Tool::Select));
        let original = state.annotations.rectangles()[0];
        let start = screen_point(&state, PagePoint::new(30.0, 45.0));
        let end = Point::new(start.x + 20.0, start.y + 30.0);
        let mut canvas_state = CanvasState::default();
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            start,
        );
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::CursorMoved { position: end }),
            end,
        );
        assert!(state.annotations.preview().is_some());
        assert_eq!(state.annotations.rectangles()[0], original);
        let _ = update(&mut state, Message::CancelInteraction);
        assert_eq!(state.annotations.rectangles()[0], original);
        let _ = update(&mut state, Message::DeleteSelected);
        assert_eq!(state.annotations.rectangles(), []);
    }

    #[test]
    fn rectangle_mode_can_resize_selected_object_through_its_handle() {
        let mut state = annotated_page();
        let original = state.annotations.rectangles()[0];
        let view = state
            .navigation
            .view(DEFAULT_PAGE, page_viewport(CANVAS.size()))
            .unwrap();
        let scale = view.page_rect().width / DEFAULT_PAGE.width;
        let controls = state.annotations.selection_bounds(scale).unwrap();
        let handle = ResizeCorner::BottomRight.position(controls);
        let start = screen_point(&state, handle);
        let end = Point::new(start.x + 20.0, start.y + 30.0);
        let mut canvas_state = CanvasState::default();
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            start,
        );
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            end,
        );
        assert_eq!(state.annotations.rectangles().len(), 1);
        let resized = state.annotations.rectangles()[0];
        assert!(resized.bounds.width > original.bounds.width);
        assert!(resized.bounds.height > original.bounds.height);
        assert_eq!(resized.stroke_width, original.stroke_width);
    }

    #[test]
    fn lost_touch_in_select_mode_reverts_edit_without_navigating() {
        let mut state = annotated_page();
        let _ = update(&mut state, Message::SetTool(Tool::Select));
        let original = state.annotations.rectangles()[0];
        let navigation = state.navigation;
        let start = screen_point(&state, PagePoint::new(30.0, 45.0));
        let end = Point::new(start.x + 20.0, start.y + 30.0);
        let mut canvas_state = CanvasState::default();
        for event in [
            touch::Event::FingerPressed {
                id: touch::Finger(1),
                position: start,
            },
            touch::Event::FingerMoved {
                id: touch::Finger(1),
                position: end,
            },
        ] {
            canvas_event(
                &mut state,
                &mut canvas_state,
                &canvas::Event::Touch(event),
                end,
            );
        }
        assert!(state.annotations.preview().is_some());
        assert_eq!(state.navigation, navigation);
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Touch(touch::Event::FingerLost {
                id: touch::Finger(1),
                position: end,
            }),
            end,
        );
        assert!(state.annotations.preview().is_none());
        assert_eq!(state.annotations.rectangles()[0], original);
        assert_eq!(state.navigation, navigation);
    }

    #[test]
    fn outlines_render_above_imported_image_and_clip_at_both_dpi_scales() {
        let mut state = FastFill {
            page: Some(ImportedImage {
                handle: Handle::from_rgba(
                    600,
                    600,
                    [240, 230, 210, 255].repeat(600 * 600),
                ),
                name: "paper.png".into(),
                size: PageSize::new(600.0, 600.0),
            }),
            ..FastFill::default()
        };
        state.annotations.set_tool(Tool::Rectangle);
        for (start, end) in [
            (PagePoint::new(100.0, 100.0), PagePoint::new(200.0, 200.0)),
            (PagePoint::new(20.0, 20.0), PagePoint::new(-100.0, -100.0)),
        ] {
            state
                .annotations
                .begin(Pointer::Mouse, start, page_size(&state));
            state.annotations.finish(Pointer::Mouse, end, false);
        }
        state.annotations.set_tool(Tool::Navigate);
        for (scale, dpi) in [(1_u32, 1.0), (2, 2.0)] {
            let pixels = render_document(&state, scale, dpi);
            let scale = usize::try_from(scale).unwrap();
            let pixel = |x: usize, y: usize| {
                let offset = (y * scale * 600 * scale + x * scale) * 4;
                &pixels[offset..offset + 4]
            };
            assert!(pixel(124, 150)[0] < 64, "outline must be above image");
            assert_eq!(
                pixel(150, 150),
                [240, 230, 210, 255],
                "image stays visible at {dpi} scale",
            );
            assert_eq!(
                pixel(54, 20),
                [67, 72, 78, 255],
                "clip at page edge at {dpi} scale; corner {:?}",
                pixel(10, 10),
            );
        }
    }

    #[test]
    fn edit_preview_replaces_the_committed_outline() {
        let mut state = rendered_page();
        state.annotations.set_tool(Tool::Rectangle);
        state.annotations.begin(
            Pointer::Mouse,
            PagePoint::new(100.0, 100.0),
            page_size(&state),
        );
        state.annotations.finish(
            Pointer::Mouse,
            PagePoint::new(200.0, 200.0),
            false,
        );
        state.annotations.set_tool(Tool::Select);
        assert!(state.annotations.begin_interaction(
            Pointer::Mouse,
            PagePoint::new(150.0, 150.0),
            page_size(&state),
            0.88,
            DragModifiers::default(),
        ));
        state.annotations.drag(
            Pointer::Mouse,
            PagePoint::new(350.0, 350.0),
            false,
        );

        let pixels = render_document(&state, 1, 1.0);
        let pixel = |x: usize, y: usize| {
            let offset = (y * 600 + x) * 4;
            &pixels[offset..offset + 4]
        };
        assert_eq!(pixel(124, 150), [240, 230, 210, 255]);
        assert_ne!(pixel(300, 344), [240, 230, 210, 255]);
    }

    fn rendered_page() -> FastFill {
        FastFill {
            page: Some(ImportedImage {
                handle: Handle::from_rgba(
                    600,
                    600,
                    [240, 230, 210, 255].repeat(600 * 600),
                ),
                name: "paper.png".into(),
                size: PageSize::new(600.0, 600.0),
            }),
            ..FastFill::default()
        }
    }

    fn render_document(state: &FastFill, scale: u32, dpi: f32) -> Vec<u8> {
        use iced::advanced::Renderer as _;
        use iced::advanced::renderer::{Headless, Style};
        use iced::advanced::{Layout, layout, widget};

        let mut renderer =
            iced::futures::executor::block_on(<Renderer as Headless>::new(
                iced::Font::DEFAULT,
                iced::Pixels(16.0),
                Some("tiny-skia"),
            ))
            .expect("software renderer needs no display or GPU");
        let mut element = document(state);
        let mut tree = widget::Tree::new(element.as_widget());
        let size = iced::Size::new(600.0, 600.0);
        renderer.reset(Rectangle::new(Point::ORIGIN, size));
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        element.as_widget().draw(
            &tree,
            &mut renderer,
            &Theme::Light,
            &Style::default(),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &Rectangle::new(Point::ORIGIN, size),
        );
        renderer.screenshot(
            iced::Size::new(600 * scale, 600 * scale),
            dpi,
            Color::WHITE,
        )
    }

    fn annotated_page() -> FastFill {
        let mut state = FastFill::default();
        state.annotations.set_tool(Tool::Rectangle);
        state.annotations.begin(
            Pointer::Mouse,
            PagePoint::new(10.0, 20.0),
            DEFAULT_PAGE,
        );
        state.annotations.finish(
            Pointer::Mouse,
            PagePoint::new(50.0, 70.0),
            false,
        );
        state
    }

    #[test]
    fn canceling_page_replacement_keeps_annotations() {
        let mut state = annotated_page();
        let _ = update(&mut state, Message::OpenImage);
        assert_eq!(state.image_picker, ImagePicker::ConfirmReplace);

        let _ = update(&mut state, Message::KeepPage);
        assert_eq!(state.image_picker, ImagePicker::Idle);
        assert_eq!(state.annotations.rectangles().len(), 1);
    }

    #[test]
    fn canceled_or_failed_picker_keeps_annotations() {
        let mut state = annotated_page();
        let _ = update(&mut state, Message::OpenImage);
        let _ = update(&mut state, Message::ReplacePage);
        assert_eq!(state.image_picker, ImagePicker::Picking);
        let _ = update(&mut state, Message::ImagePicked(Ok(None)));
        assert_eq!(state.image_picker, ImagePicker::Idle);
        assert_eq!(state.annotations.rectangles().len(), 1);

        let _ = update(
            &mut state,
            Message::ImagePicked(Err("Corrupt image".into())),
        );
        assert!(state.status_is_error);
        assert_eq!(state.annotations.rectangles().len(), 1);
    }

    #[test]
    fn successful_import_resets_annotations_and_retains_tool() {
        let mut state = annotated_page();
        let image = ImportedImage {
            handle: Handle::from_rgba(1, 1, vec![255; 4]),
            name: "replacement.png".into(),
            size: PageSize::new(1.0, 1.0),
        };
        let _ = update(&mut state, Message::OpenImage);
        let _ = update(&mut state, Message::ReplacePage);
        let _ = update(&mut state, Message::ImagePicked(Ok(Some(image))));

        assert_eq!(state.annotations.rectangles(), []);
        assert_eq!(state.annotations.tool(), Tool::Rectangle);
        assert_eq!(state.page.unwrap().name, "replacement.png");
    }

    #[test]
    fn cancel_and_navigation_changes_never_commit_a_draft() {
        let viewport = PageRect::new(36.0, 36.0, 1_000.0, 600.0);
        for message in [
            Message::CancelInteraction,
            Message::FocusLost,
            Message::SetTool(Tool::Navigate),
            Message::Fit,
            Message::OneToOne(viewport),
            Message::ZoomAt {
                factor: 2.0,
                anchor: viewport.center(),
                viewport,
            },
        ] {
            let mut state = annotated_page();
            state.annotations.begin(
                Pointer::Mouse,
                PagePoint::new(100.0, 100.0),
                DEFAULT_PAGE,
            );
            let _ = update(&mut state, message);
            assert!(state.annotations.active_pointer().is_none());
            assert_eq!(state.annotations.rectangles().len(), 1);
        }
    }

    const CANVAS: Rectangle = Rectangle::new(
        Point::new(14.0, 192.0),
        iced::Size::new(1_200.0, 650.0),
    );

    fn screen_point(state: &FastFill, point: PagePoint) -> Point {
        let view = state
            .navigation
            .view(page_size(state), page_viewport(CANVAS.size()))
            .unwrap();
        let local = view.page_to_view(point);
        Point::new(local.x + CANVAS.x, local.y + CANVAS.y)
    }

    fn canvas_event(
        state: &mut FastFill,
        canvas_state: &mut CanvasState,
        event: &canvas::Event,
        position: Point,
    ) -> Option<Message> {
        let page = PageCanvas {
            image: state.page.as_ref(),
            navigation: state.navigation,
            page_size: page_size(state),
            annotations: &state.annotations,
            input_epoch: state.input_epoch,
            input_enabled: state.image_picker == ImagePicker::Idle,
            modifiers: state.modifiers,
            background: false,
        };
        let message = canvas::Program::update(
            &page,
            canvas_state,
            event,
            CANVAS,
            mouse::Cursor::Available(position),
        )?
        .into_inner()
        .0?;
        let _ = update(state, message.clone());
        Some(message)
    }

    #[test]
    fn mouse_drawing_uses_canvas_offset_and_ctrl_without_panning() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
        let mut canvas_state = CanvasState::default();
        let start = screen_point(&state, PagePoint::new(100.0, 200.0));
        let end = screen_point(&state, PagePoint::new(220.0, 250.0));
        let navigation = state.navigation;

        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            start,
        );
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::CursorMoved { position: end }),
            end,
        );
        let _ = update(
            &mut state,
            Message::ModifiersChanged(DragModifiers {
                square: true,
                ..DragModifiers::default()
            }),
        );
        let preview = state.annotations.preview().unwrap().bounds;
        assert!((preview.width - preview.height).abs() < 0.001);
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            end,
        );
        assert_eq!(state.navigation, navigation);
        let rectangle = state.annotations.rectangles()[0].bounds;
        assert!((rectangle.x - 100.0).abs() < 0.001);
        assert!((rectangle.y - 200.0).abs() < 0.001);
        assert!((rectangle.width - 120.0).abs() < 0.001);
        assert!((rectangle.height - 120.0).abs() < 0.001);
    }

    #[test]
    fn right_drag_pans_in_rectangle_mode_without_creating_an_object() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
        let mut canvas_state = CanvasState::default();
        let start = screen_point(&state, PagePoint::new(100.0, 200.0));
        let end = Point::new(start.x + 20.0, start.y + 30.0);
        let before = screen_point(&state, PagePoint::new(0.0, 0.0));

        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Right,
            )),
            start,
        );
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::CursorMoved { position: end }),
            end,
        );
        let after = screen_point(&state, PagePoint::new(0.0, 0.0));
        assert!((after.x - before.x - 20.0).abs() < 0.001);
        assert!((after.y - before.y - 30.0).abs() < 0.001);
        assert_eq!(state.annotations.rectangles(), []);
    }

    #[test]
    fn wheel_zooms_around_pointer_in_all_tools() {
        for tool in [Tool::Navigate, Tool::Select, Tool::Rectangle] {
            let mut state = FastFill::default();
            let _ = update(&mut state, Message::SetTool(tool));
            let mut canvas_state = CanvasState::default();
            let anchor = PagePoint::new(100.0, 200.0);
            let before = screen_point(&state, anchor);
            let message = canvas_event(
                &mut state,
                &mut canvas_state,
                &canvas::Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
                }),
                before,
            );
            assert!(matches!(message, Some(Message::ZoomAt { .. })));
            assert!((state.navigation.zoom() - 1.2).abs() < 0.001);
            let after = screen_point(&state, anchor);
            assert!((after.x - before.x).abs() < 0.001);
            assert!((after.y - before.y).abs() < 0.001);
            assert_eq!(state.annotations.rectangles(), []);
        }
    }

    #[test]
    fn wheel_does_not_interrupt_active_mouse_or_touch_gestures() {
        for tool in [Tool::Navigate, Tool::Rectangle] {
            for pointer in 0..3 {
                let mut state = FastFill::default();
                let _ = update(&mut state, Message::SetTool(tool));
                let mut canvas_state = CanvasState::default();
                let start = screen_point(&state, PagePoint::new(100.0, 200.0));
                let press = match pointer {
                    0 => canvas::Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    )),
                    1 => canvas::Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Right,
                    )),
                    _ => canvas::Event::Touch(touch::Event::FingerPressed {
                        id: touch::Finger(1),
                        position: start,
                    }),
                };
                canvas_event(&mut state, &mut canvas_state, &press, start);
                let navigation = state.navigation;
                let preview = state.annotations.preview();
                let message = canvas_event(
                    &mut state,
                    &mut canvas_state,
                    &canvas::Event::Mouse(mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
                    }),
                    start,
                );
                assert!(message.is_none());
                assert_eq!(state.navigation, navigation);
                assert_eq!(state.annotations.preview(), preview);
            }
        }
    }

    #[test]
    fn mouse_release_outside_canvas_keeps_partial_page_geometry() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
        let mut canvas_state = CanvasState::default();
        let start = screen_point(&state, PagePoint::new(100.0, 200.0));
        let end = Point::new(CANVAS.x - 20.0, CANVAS.y + 20.0);
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
            start,
        );
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonReleased(
                mouse::Button::Left,
            )),
            end,
        );
        assert!(state.annotations.rectangles()[0].bounds.x < 0.0);
        assert!(state.annotations.active_pointer().is_none());
    }

    #[test]
    fn drawing_touch_ignores_second_finger_and_lost_touch_cancels() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
        let mut canvas_state = CanvasState::default();
        let first = touch::Finger(1);
        let second = touch::Finger(2);
        let start = screen_point(&state, PagePoint::new(100.0, 200.0));
        let end = screen_point(&state, PagePoint::new(220.0, 250.0));
        let navigation = state.navigation;

        for event in [
            touch::Event::FingerPressed {
                id: first,
                position: start,
            },
            touch::Event::FingerPressed {
                id: second,
                position: end,
            },
            touch::Event::FingerMoved {
                id: first,
                position: end,
            },
            touch::Event::FingerMoved {
                id: second,
                position: start,
            },
            touch::Event::FingerLifted {
                id: second,
                position: start,
            },
        ] {
            canvas_event(
                &mut state,
                &mut canvas_state,
                &canvas::Event::Touch(event),
                start,
            );
        }
        assert_eq!(state.navigation, navigation);
        assert_eq!(state.annotations.active_pointer(), Some(Pointer::Touch(1)));
        assert_eq!(state.annotations.rectangles(), []);
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Touch(touch::Event::FingerLost {
                id: first,
                position: end,
            }),
            end,
        );
        assert!(state.annotations.active_pointer().is_none());
        assert_eq!(state.annotations.rectangles(), []);

        for event in [
            touch::Event::FingerPressed {
                id: first,
                position: start,
            },
            touch::Event::FingerLifted {
                id: first,
                position: end,
            },
        ] {
            canvas_event(
                &mut state,
                &mut canvas_state,
                &canvas::Event::Touch(event),
                end,
            );
        }
        assert_eq!(state.annotations.rectangles().len(), 1);
        assert_eq!(state.navigation, navigation);
    }
}
