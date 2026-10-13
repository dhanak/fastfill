#[cfg(test)]
use crate::page_canvas::CanvasState;
use crate::page_canvas::{PageCanvas, page_viewport};
use crate::picker::{ImportedImage, pick_image};
#[cfg(test)]
use fastfill_rust::annotations::ResizeCorner;
use fastfill_rust::annotations::{Annotations, DragModifiers, Pointer, Tool};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::GestureDelta;
#[cfg(test)]
use iced::touch;
#[cfg(test)]
use iced::widget::image::Handle;
use iced::widget::{button, canvas, column, container, responsive, row, text};
#[cfg(test)]
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};
use iced::{Element, Fill, Task, keyboard};

const DEFAULT_PAGE: PageSize = PageSize::new(794.0, 1_123.0);
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
pub(crate) enum Message {
    OpenImage,
    ReplacePage,
    KeepPage,
    ImagePicked(Result<Option<ImportedImage>, String>),
    SetTool(Tool),
    Undo,
    Redo,
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

pub(crate) fn run() -> iced::Result {
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
                    key,
                    modifiers,
                    ..
                }) => shortcut_message(key, modifiers),
                iced::Event::Window(iced::window::Event::Resized(_)) => {
                    Some(Message::CancelInteraction)
                }
                _ => None,
            })
        })
        .run()
}

fn shortcut_message(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    match key {
        keyboard::Key::Character(value)
            if modifiers.command()
                && modifiers.shift()
                && value.eq_ignore_ascii_case("z") =>
        {
            Some(Message::Redo)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("z") =>
        {
            Some(Message::Undo)
        }
        keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("y") =>
        {
            Some(Message::Redo)
        }
        keyboard::Key::Named(keyboard::key::Named::Delete) => {
            Some(Message::DeleteSelected)
        }
        keyboard::Key::Named(keyboard::key::Named::Escape) => {
            Some(Message::CancelInteraction)
        }
        _ => None,
    }
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

    fn undo(&mut self) {
        if self.image_picker == ImagePicker::Idle && self.annotations.undo() {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
    }

    fn redo(&mut self) {
        if self.image_picker == ImagePicker::Idle && self.annotations.redo() {
            self.input_epoch = self.input_epoch.wrapping_add(1);
        }
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
                if state.annotations.shapes().is_empty() {
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
        Message::Undo => state.undo(),
        Message::Redo => state.redo(),
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
                    text("Replace page and discard its annotations?")
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
            button("Undo")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && state.annotations.can_undo())
                    .then_some(Message::Undo),
                )
                .height(40),
            button("Redo")
                .on_press_maybe(
                    (state.image_picker == ImagePicker::Idle
                        && state.annotations.can_redo())
                    .then_some(Message::Redo),
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
            tool_button("Oval", Tool::Oval, state.annotations.tool()),
            tool_button("Line", Tool::Line, state.annotations.tool()),
            tool_button("Arrow", Tool::Arrow, state.annotations.tool()),
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
                text(format!("{} shapes", state.annotations.shapes().len()))
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

fn page_size(state: &FastFill) -> PageSize {
    state.page.as_ref().map_or(DEFAULT_PAGE, |page| page.size)
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
