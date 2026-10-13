use crate::appearance::{
    CONTROL_GAP, CONTROL_HEIGHT, TOOLBAR_HEIGHT, TOOLBAR_PADDING,
    TOOLBAR_ROW_HEIGHT,
};
#[cfg(test)]
use crate::page_canvas::CanvasState;
use crate::page_canvas::{PageCanvas, page_viewport};
use crate::picker::{ImportedImage, pick_image};
#[cfg(test)]
use fastfill_rust::annotations::ResizeCorner;
use fastfill_rust::annotations::{
    AnnotationColor, Annotations, DragModifiers, Pointer, Tool,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::GestureDelta;
#[cfg(test)]
use iced::touch;
#[cfg(test)]
use iced::widget::image::Handle;
use iced::widget::{
    Row, button, canvas, column, container, responsive, row, text,
};
#[cfg(test)]
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};
use iced::{Element, Fill, Task, keyboard};

const DEFAULT_PAGE: PageSize = PageSize::new(794.0, 1_123.0);

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
    SetColor(AnnotationColor),
    AdjustThickness(f32),
    ToggleFill,
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

    fn adjust_thickness(&mut self, delta: f32) {
        if let Some(style) = self.annotations.current_style() {
            self.annotations.set_stroke_width(
                (style.stroke_width + delta).clamp(1.0, 20.0),
            );
        }
    }

    fn begin_interaction(
        &mut self,
        pointer: Pointer,
        point: PagePoint,
        scale: f32,
    ) {
        if self.image_picker == ImagePicker::Idle {
            self.annotations.begin_interaction(
                pointer,
                point,
                page_size(self),
                scale,
                self.modifiers,
            );
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
        Message::SetColor(color) => {
            state.annotations.set_color(color);
        }
        Message::AdjustThickness(delta) => state.adjust_thickness(delta),
        Message::ToggleFill => {
            if let Some(style) = state.annotations.current_style() {
                state.annotations.set_filled(!style.filled);
            }
        }
        Message::DrawPressed(pointer, point, scale) => {
            state.begin_interaction(pointer, point, scale);
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
        column![
            control_row(state, viewport),
            tool_row(state, available.width),
            style_row(state),
            document(state),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    });

    column![heading(state), body]
        .width(Fill)
        .height(Fill)
        .into()
}

fn control_row(state: &FastFill, viewport: PageRect) -> Element<'_, Message> {
    let zoom = |factor| Message::ZoomAt {
        factor,
        anchor: viewport.center(),
        viewport,
    };
    toolbar_row(row![
        button("Open image")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle)
                    .then_some(Message::OpenImage),
            )
            .height(CONTROL_HEIGHT),
        button("Undo")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations.can_undo())
                .then_some(Message::Undo),
            )
            .height(CONTROL_HEIGHT),
        button("Redo")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations.can_redo())
                .then_some(Message::Redo),
            )
            .height(CONTROL_HEIGHT),
        button("−").on_press(zoom(0.8)).height(CONTROL_HEIGHT),
        button("+").on_press(zoom(1.25)).height(CONTROL_HEIGHT),
        button("1:1")
            .on_press(Message::OneToOne(viewport))
            .height(CONTROL_HEIGHT),
        button("Fit").on_press(Message::Fit).height(CONTROL_HEIGHT),
    ])
    .into()
}

fn tool_row(state: &FastFill, available_width: f32) -> Element<'_, Message> {
    let selected = state.annotations.tool();
    let mut tools = row![
        tool_button("Navigate", Tool::Navigate, selected),
        tool_button("Select", Tool::Select, selected),
        tool_button("Rectangle", Tool::Rectangle, selected),
        tool_button("Oval", Tool::Oval, selected),
        tool_button("Line", Tool::Line, selected),
        tool_button("Arrow", Tool::Arrow, selected),
        tool_button("Check", Tool::Checkmark, selected),
        tool_button("Cross", Tool::Cross, selected),
        tool_button("Pen", Tool::Pen, selected),
        tool_button("Highlight", Tool::Highlighter, selected),
        button("Delete")
            .on_press_maybe(
                (state.image_picker == ImagePicker::Idle
                    && state.annotations.selected_index().is_some()
                    && state.annotations.active_pointer().is_none())
                .then_some(Message::DeleteSelected),
            )
            .height(CONTROL_HEIGHT),
    ];
    if available_width >= 520.0 {
        tools = tools.push(
            text(format!("{} shapes", state.annotations.shapes().len()))
                .size(14),
        );
    }
    toolbar_row(tools).into()
}

fn style_row(state: &FastFill) -> Element<'_, Message> {
    let Some(style) = state.annotations.current_style() else {
        return container(iced::widget::Space::new())
            .height(TOOLBAR_ROW_HEIGHT)
            .into();
    };
    let mut styles = row![
        color_button("Black", AnnotationColor::Black, style.color),
        color_button("White", AnnotationColor::White, style.color),
        color_button("Red", AnnotationColor::Red, style.color),
        color_button("Blue", AnnotationColor::Blue, style.color),
        color_button("Green", AnnotationColor::Green, style.color),
        color_button("Yellow", AnnotationColor::Yellow, style.color),
        button("−")
            .on_press_maybe(
                (style.stroke_width > 1.0)
                    .then_some(Message::AdjustThickness(-1.0)),
            )
            .height(CONTROL_HEIGHT),
        text(format!("{:.0} px", style.stroke_width)).size(14),
        button("+")
            .on_press_maybe(
                (style.stroke_width < 20.0)
                    .then_some(Message::AdjustThickness(1.0)),
            )
            .height(CONTROL_HEIGHT),
    ];
    if state.annotations.fill_available() {
        styles = styles.push(
            button(if style.filled { "Fill ✓" } else { "Fill" })
                .on_press(Message::ToggleFill)
                .style(if style.filled {
                    button::primary
                } else {
                    button::secondary
                })
                .height(CONTROL_HEIGHT),
        );
    }
    toolbar_row(styles).into()
}

fn toolbar_row(content: Row<'_, Message>) -> Row<'_, Message> {
    content
        .align_y(iced::Alignment::Center)
        .height(TOOLBAR_ROW_HEIGHT)
        .padding(TOOLBAR_PADDING)
        .spacing(CONTROL_GAP)
}

fn color_button(
    label: &str,
    color: AnnotationColor,
    selected: AnnotationColor,
) -> iced::widget::Button<'_, Message> {
    button(label)
        .on_press(Message::SetColor(color))
        .style(if color == selected {
            button::primary
        } else {
            button::secondary
        })
        .height(CONTROL_HEIGHT)
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
        .height(CONTROL_HEIGHT)
}

fn page_size(state: &FastFill) -> PageSize {
    state.page.as_ref().map_or(DEFAULT_PAGE, |page| page.size)
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
