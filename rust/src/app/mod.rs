mod controller;
mod view;

#[cfg(test)]
use crate::page_canvas::CanvasState;
#[cfg(test)]
use crate::page_canvas::{PageCanvas, page_viewport};
use crate::picker::ImportedImage;
use controller::{shortcut_message, update};
#[cfg(test)]
use fastfill_rust::annotations::ResizeCorner;
use fastfill_rust::annotations::{
    AnnotationColor, Annotations, DragModifiers, Pointer, Tool,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::GestureDelta;
use iced::keyboard;
#[cfg(test)]
use iced::touch;
#[cfg(test)]
use iced::widget::canvas;
#[cfg(test)]
use iced::widget::image::Handle;
#[cfg(test)]
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};
#[cfg(test)]
use view::document;
use view::view;

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
    confirm_delete_all: bool,
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
    ResetRotation,
    DeleteSelected,
    AskDeleteAll,
    ConfirmDeleteAll,
    KeepAll,
    SendToBack,
    BringToFront,
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

fn page_size(state: &FastFill) -> PageSize {
    state.page.as_ref().map_or(DEFAULT_PAGE, |page| page.size)
}

#[cfg(test)]
mod tests;
