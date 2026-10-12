use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::image_input::ImageInfo;
#[cfg(not(target_arch = "wasm32"))]
use fastfill_rust::image_input::MAX_IMAGE_BYTES;
use fastfill_rust::image_input::{validate_image, validate_image_length};
use fastfill_rust::interaction::{GestureDelta, TouchEvent, TouchNavigation};
use iced::mouse;
use iced::touch;
use iced::widget::image::Handle;
use iced::widget::{button, canvas, column, container, responsive, row, text};
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Task, Theme};

const DEFAULT_PAGE: PageSize = PageSize::new(794.0, 1_123.0);
const PAGE_MARGIN: f32 = 36.0;
const TOOLBAR_HEIGHT: f32 = 56.0;

#[derive(Default)]
struct FastFill {
    page: Option<ImportedImage>,
    navigation: Navigation,
    status: String,
    status_is_error: bool,
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
    ImagePicked(Result<Option<ImportedImage>, String>),
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

#[derive(Debug)]
struct PageCanvas<'a> {
    image: Option<&'a ImportedImage>,
    navigation: Navigation,
    page_size: PageSize,
    navigation_enabled: bool,
}

#[derive(Default)]
struct CanvasState {
    dragging: Option<mouse::Button>,
    last_position: Option<Point>,
    touch: TouchNavigation,
}

fn main() -> iced::Result {
    iced::application(FastFill::default, update, view)
        .title("FastFill Rust prototype")
        .window_size((1_200.0, 800.0))
        .antialiasing(true)
        .run()
}

fn update(state: &mut FastFill, message: Message) -> Task<Message> {
    match message {
        Message::OpenImage => {
            state.status = "Selecting an image…".into();
            state.status_is_error = false;
            return Task::perform(pick_image(), Message::ImagePicked);
        }
        Message::ImagePicked(Ok(Some(image))) => {
            state.status = format!(
                "Opened {} ({} × {} px)",
                image.name, image.size.width, image.size.height,
            );
            state.page = Some(image);
            state.navigation.fit();
            state.status_is_error = false;
        }
        Message::ImagePicked(Ok(None)) => {
            state.status = "Open canceled.".into();
            state.status_is_error = false;
        }
        Message::ImagePicked(Err(error)) => {
            state.status = error;
            state.status_is_error = true;
        }
        Message::Fit => state.navigation.fit(),
        Message::OneToOne(viewport) => {
            state.navigation.one_to_one(page_size(state), viewport);
        }
        Message::Pan(delta) => state.navigation.pan_by(delta),
        Message::ZoomAt {
            factor,
            anchor,
            viewport,
        } => {
            state.navigation.zoom_at(
                factor,
                anchor,
                page_size(state),
                viewport,
            );
        }
        Message::TouchGesture { delta, viewport } => {
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

fn view(state: &FastFill) -> Element<'_, Message> {
    let status = if state.status.is_empty() {
        "Open a PNG or JPEG to test navigation."
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
    let heading =
        container(column![text("FastFill").size(24), status_line].spacing(4))
            .padding([10, 20]);

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
        let toolbar = row![
            button("Open image").on_press(Message::OpenImage).height(40),
            button("−").on_press(zoom_out).height(40),
            button("+").on_press(zoom_in).height(40),
            button("1:1")
                .on_press(Message::OneToOne(viewport))
                .height(40),
            button("Fit").on_press(Message::Fit).height(40),
            text("Navigate: drag · right-drag · mouse wheel").size(14),
        ]
        .align_y(iced::Alignment::Center)
        .height(TOOLBAR_HEIGHT)
        .padding([8, 12])
        .spacing(8);
        let page = PageCanvas {
            image: state.page.as_ref(),
            navigation: state.navigation,
            page_size: page_size(state),
            navigation_enabled: true,
        };

        column![toolbar, canvas(page).width(Fill).height(Fill)]
            .width(Fill)
            .height(Fill)
            .into()
    });

    column![heading, body].width(Fill).height(Fill).into()
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
        if let canvas::Event::Touch(event) = event {
            let was_active = state.touch.is_active();
            let touch_event = map_touch_event(*event, bounds)?;
            if matches!(touch_event, TouchEvent::Pressed(_, _)) {
                state.dragging = None;
                state.last_position = None;
            }
            let delta =
                state.touch.handle(touch_event, self.navigation_enabled);
            return if let Some(delta) = delta {
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
            };
        }

        let position = cursor.position_in(bounds);
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(button))
                if matches!(
                    button,
                    mouse::Button::Left | mouse::Button::Right
                ) && (*button == mouse::Button::Right
                    || self.navigation_enabled)
                    && position.is_some()
                    && !state.touch.is_active() =>
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
                let current = position?;
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
                if self.navigation_enabled && !state.touch.is_active() =>
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
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Color::from_rgb8(67, 72, 78),
        );

        if let Some(view) = self
            .navigation
            .view(self.page_size, page_viewport(bounds.size()))
        {
            let page = view.page_rect();
            let size = iced::Size::new(page.width, page.height);
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
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.position_in(bounds).is_none() {
            mouse::Interaction::None
        } else if state.dragging.is_some() || state.touch.is_active() {
            mouse::Interaction::Grabbing
        } else if self.navigation_enabled {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::None
        }
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
