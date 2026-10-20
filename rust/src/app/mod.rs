#[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
mod camera;
mod confirmation;
mod controller;
mod export;
mod export_io;
mod help;
#[cfg(test)]
mod interaction_tests;
mod page_edit;
mod pages;
mod pdf;
mod project;
mod project_io;
mod project_view;
mod rail;
#[cfg(not(target_arch = "wasm32"))]
mod recovery;
mod snap;
mod text;
mod text_controls;
mod toolbar;
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
    AnnotationColor, Annotations, DragModifiers, Pointer, TextFormat,
    TextTypeface, Tool,
};
use fastfill_rust::geometry::{Navigation, Point as PagePoint};
use fastfill_rust::geometry::{Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::GestureDelta;
use iced::keyboard;
#[cfg(test)]
use iced::widget::canvas;
#[cfg(test)]
use iced::widget::image::Handle;
use iced::widget::text_editor;
#[cfg(test)]
use iced::{Point, Rectangle, Renderer, Theme};
#[cfg(test)]
use iced::{mouse, touch};
use pages::Pages;
use project::ProjectSession;
use rail::PageDrag;
#[cfg(test)]
use view::document;
use view::view;

const DEFAULT_PAGE: PageSize = PageSize::new(612.0, 612.0 * 1_123.0 / 794.0);
const TEXT_EDITOR_ID: &str = "inline-text-editor";

#[derive(Default)]
// Independent UI flags; document/workflow state stays with its owners.
#[allow(clippy::struct_excessive_bools)]
struct FastFill {
    pages: Pages,
    project: ProjectSession,
    navigation: Navigation,
    theme_override: Option<iced::Theme>,
    modifiers: DragModifiers,
    input_epoch: u64,
    image_picker: ImagePicker,
    status: String,
    status_is_error: bool,
    confirm_delete_all: bool,
    confirm_delete_page: bool,
    show_shortcuts: bool,
    dragged_page: Option<PageDrag>,
    // Shared by the clipped rail and its floating layer; not document state.
    rail_motion: std::cell::RefCell<rail::Motion>,
    page_scroll: f32,
    image_directory: Option<std::path::PathBuf>,
    export_directory: Option<std::path::PathBuf>,
    text_content: text_editor::Content,
    font_size_input: Option<String>,
    page_editor: page_edit::PageEditor,
    smart_snap: snap::SmartSnap,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    camera: camera::CameraScreen,
    #[cfg(not(target_arch = "wasm32"))]
    recovery: recovery::Recovery,
}

impl FastFill {
    fn image(&self) -> Option<&ImportedImage> {
        self.pages.current().image.as_ref()
    }

    fn annotations(&self) -> &Annotations {
        &self.pages.current().annotations
    }

    fn annotations_mut(&mut self) -> &mut Annotations {
        &mut self.pages.current_mut().annotations
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PickPurpose {
    Add,
    Replace,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ImagePicker {
    #[default]
    Idle,
    ConfirmReplace,
    Picking(PickPurpose),
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    SystemThemeChanged(iced::theme::Mode),
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    Camera(camera::Event),
    PageEdit(page_edit::Event),
    SmartSnap(snap::Event),
    ViewportResized,
    #[cfg(not(target_arch = "wasm32"))]
    Recovery(recovery::Event),
    #[cfg(not(target_arch = "wasm32"))]
    RestoreRecovery,
    #[cfg(not(target_arch = "wasm32"))]
    DiscardRecovery,
    #[cfg(not(target_arch = "wasm32"))]
    DeferRecovery,
    NewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    ExportPdf,
    CancelPdfExport,
    PdfExportProgress(u64, usize, usize),
    PdfExported(u64, Result<Option<export_io::ExportReceipt>, String>),
    DiscardProject,
    SaveBeforeLeave,
    KeepProject,
    CloseRequested(iced::window::Id),
    ProjectOpened(
        u64,
        Result<Option<std::sync::Arc<project_io::OpenedProject>>, String>,
    ),
    ProjectSaved(u64, Result<Option<project_io::SaveReceipt>, String>),
    AddPage,
    AskReplacePage,
    ConfirmReplacePage,
    KeepPage,
    ImagePicked(PickPurpose, Result<Option<ImportedImage>, String>),
    ImportPdf,
    CancelPdfImport,
    PdfProgress(u64, usize, usize),
    PdfStage(u64, usize, usize, fastfill_rust::pdf_import::PdfStage),
    PdfImported(u64, Result<Option<std::sync::Arc<pdf::PdfBatch>>, String>),
    SelectPage(usize),
    MovePageLeft,
    MovePageRight,
    PageRail(rail::Event),
    AskDeletePage,
    ConfirmDeletePage,
    SetTool(Tool),
    Undo,
    Redo,
    ResetRotation,
    DeleteSelected,
    AskDeleteAll,
    ConfirmDeleteAll,
    KeepAll,
    ModalBackdrop,
    ShowShortcuts,
    HideShortcuts,
    SendToBack,
    BringToFront,
    SetColor(AnnotationColor),
    SetTextTypeface(TextTypeface),
    SetTextSize(f32),
    FontSize(text_controls::Event),
    ToggleTextBold,
    ToggleTextItalic,
    ToggleTextUnderline,
    AdjustThickness(f32),
    ToggleFill,
    TextAction(text_editor::Action),
    AcceptText,
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
    iced::application(boot, update, view)
        .title(|state: &FastFill| {
            format!(
                "FastFill — {}{}",
                state.project.name,
                if state.project.is_dirty(&state.pages) {
                    " *"
                } else {
                    ""
                }
            )
        })
        .window_size((1_200.0, 800.0))
        .antialiasing(true)
        .theme(|state: &FastFill| state.theme_override.clone())
        .exit_on_close_request(false)
        .subscription(|_| {
            iced::Subscription::batch([
                iced::system::theme_changes().map(Message::SystemThemeChanged),
                iced::event::listen_with(|event, status, id| match event {
                    iced::Event::Window(
                        iced::window::Event::CloseRequested,
                    ) => Some(Message::CloseRequested(id)),
                    iced::Event::Window(iced::window::Event::Unfocused) => {
                        Some(Message::FocusLost)
                    }
                    iced::Event::Keyboard(
                        keyboard::Event::ModifiersChanged(m),
                    ) => Some(Message::ModifiersChanged(DragModifiers {
                        square: m.control(),
                        force_move: m.shift(),
                        force_resize: m.alt(),
                    })),
                    iced::Event::Keyboard(keyboard::Event::KeyPressed {
                        key,
                        modifiers,
                        ..
                    }) => shortcut_message(key, modifiers, status),
                    iced::Event::Window(iced::window::Event::Resized(_)) => {
                        Some(Message::ViewportResized)
                    }
                    _ => None,
                }),
            ])
        })
        .run()
}

fn boot() -> (FastFill, iced::Task<Message>) {
    let state = FastFill::default();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut state = state;
        state.project.operation = project::Operation::Recovering;
        (state, recovery::start())
    }
    #[cfg(target_arch = "wasm32")]
    (state, iced::Task::none())
}

fn page_size(state: &FastFill) -> PageSize {
    state.pages.current().size
}

#[cfg(test)]
mod tests;
