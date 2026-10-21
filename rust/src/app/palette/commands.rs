use super::super::{FastFill, ImagePicker, Message};
use crate::icons::Glyph;
use fastfill_rust::annotations::Tool;
use fastfill_rust::geometry::Rect as PageRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    New,
    Open,
    Save,
    #[cfg(not(target_arch = "wasm32"))]
    SaveAs,
    Export,
    Undo,
    Redo,
    Fit,
    ActualSize,
    AddPage,
    ImportPdf,
    #[cfg(all(
        feature = "native-camera",
        any(target_os = "linux", target_os = "windows")
    ))]
    Camera,
    ReplaceImage,
    EditPage,
    DeletePage,
    Earlier,
    Later,
    Tool(Tool),
    SelectAll,
    Back,
    Front,
    DeleteSelected,
    DeleteAll,
    ResetAngle,
    SmartSnap,
    Fill,
    Shortcuts,
}

pub(super) fn all() -> Vec<Command> {
    use Command as C;
    let mut commands = vec![C::New, C::Open, C::Save];
    #[cfg(not(target_arch = "wasm32"))]
    commands.push(C::SaveAs);
    commands.extend([
        C::Export,
        C::Undo,
        C::Redo,
        C::Fit,
        C::ActualSize,
        C::AddPage,
        C::ImportPdf,
    ]);
    #[cfg(all(
        feature = "native-camera",
        any(target_os = "linux", target_os = "windows")
    ))]
    commands.push(C::Camera);
    commands.extend([
        C::ReplaceImage,
        C::EditPage,
        C::DeletePage,
        C::Earlier,
        C::Later,
    ]);
    commands.extend(Tool::ALL.into_iter().map(C::Tool));
    commands.extend([
        C::SelectAll,
        C::Back,
        C::Front,
        C::DeleteSelected,
        C::DeleteAll,
        C::ResetAngle,
        C::SmartSnap,
        C::Fill,
        C::Shortcuts,
    ]);
    commands
}

impl Command {
    pub(super) fn label(self) -> &'static str {
        use Command as C;
        match self {
            C::New => "New project",
            C::Open => "Open project",
            C::Save => "Save project",
            #[cfg(not(target_arch = "wasm32"))]
            C::SaveAs => "Save project as",
            C::Export => "Export PDF",
            C::Undo => "Undo",
            C::Redo => "Redo",
            C::Fit => "Fit page",
            C::ActualSize => "Actual size (1:1)",
            C::AddPage => "Add image page",
            C::ImportPdf => "Import PDF",
            #[cfg(all(
                feature = "native-camera",
                any(target_os = "linux", target_os = "windows")
            ))]
            C::Camera => "Open camera",
            C::ReplaceImage => "Replace page image",
            C::EditPage => "Edit page",
            C::DeletePage => "Delete page",
            C::Earlier => "Move page earlier",
            C::Later => "Move page later",
            C::Tool(tool) => tool.label(),
            C::SelectAll => "Select all annotations",
            C::Back => "Send selection to back",
            C::Front => "Bring selection to front",
            C::DeleteSelected => "Delete selection",
            C::DeleteAll => "Delete all annotations",
            C::ResetAngle => "Reset angle to 0°",
            C::SmartSnap => "Toggle smart snap",
            C::Fill => "Toggle shape fill",
            C::Shortcuts => "Show shortcuts",
        }
    }

    pub(super) fn group(self) -> &'static str {
        use Command as C;
        match self {
            C::New | C::Open | C::Save | C::Export => "Project",
            #[cfg(not(target_arch = "wasm32"))]
            C::SaveAs => "Project",
            C::Undo | C::Redo => "History",
            C::Fit | C::ActualSize => "View",
            C::AddPage
            | C::ImportPdf
            | C::ReplaceImage
            | C::EditPage
            | C::DeletePage
            | C::Earlier
            | C::Later => "Page",
            #[cfg(all(
                feature = "native-camera",
                any(target_os = "linux", target_os = "windows")
            ))]
            C::Camera => "Page",
            C::Tool(_) => "Tool",
            C::SelectAll
            | C::Back
            | C::Front
            | C::DeleteSelected
            | C::DeleteAll
            | C::ResetAngle
            | C::SmartSnap
            | C::Fill => "Annotation",
            C::Shortcuts => "Help",
        }
    }

    pub(super) fn aliases(self) -> &'static str {
        use Command as C;
        match self {
            C::ActualSize => "zoom 100%",
            C::Fit => "zoom fit",
            C::Tool(Tool::Navigate) => "hand pan",
            C::Tool(Tool::Select) => "pointer marquee",
            C::Tool(Tool::Checkmark) => "check tick",
            C::Tool(Tool::Oval) => "ellipse circle",
            C::Back => "layer lower",
            C::Front => "layer raise",
            C::ResetAngle => "rotate zero",
            C::Shortcuts => "help keys F1",
            _ => "",
        }
    }

    pub(super) fn glyph(self) -> Glyph {
        use Command as C;
        match self {
            C::New => Glyph::NewProject,
            C::Open => Glyph::OpenProject,
            C::Save => Glyph::Save,
            #[cfg(not(target_arch = "wasm32"))]
            C::SaveAs => Glyph::SaveAs,
            C::Export => Glyph::ExportPdf,
            C::Undo => Glyph::Undo,
            C::Redo => Glyph::Redo,
            C::Fit => Glyph::Fit,
            C::ActualSize => Glyph::ActualSize,
            C::AddPage => Glyph::AddImage,
            C::ImportPdf => Glyph::ImportPdf,
            #[cfg(all(
                feature = "native-camera",
                any(target_os = "linux", target_os = "windows")
            ))]
            C::Camera => Glyph::Camera,
            C::ReplaceImage => Glyph::ReplaceImage,
            C::EditPage => Glyph::EditPage,
            C::DeletePage | C::DeleteSelected => Glyph::Delete,
            C::Earlier => Glyph::MoveUp,
            C::Later => Glyph::MoveDown,
            C::Tool(tool) => Glyph::Tool(tool),
            C::SelectAll => Glyph::Tool(Tool::Select),
            C::Back => Glyph::ToBack,
            C::Front => Glyph::ToFront,
            C::DeleteAll => Glyph::DeleteAll,
            C::ResetAngle => Glyph::ResetAngle,
            C::SmartSnap => Glyph::SmartSnap,
            C::Fill => Glyph::Fill,
            C::Shortcuts => Glyph::Help,
        }
    }

    pub(super) fn shortcut(self) -> &'static str {
        use Command as C;
        match self {
            C::New => "N",
            C::Open => "O",
            C::Save => "S",
            #[cfg(not(target_arch = "wasm32"))]
            C::SaveAs => "Shift+S",
            C::Undo => "Z",
            C::Redo => "Y",
            C::SelectAll => "A",
            C::DeleteSelected => "Delete",
            C::Shortcuts => "F1",
            _ => "",
        }
    }

    pub(super) fn available(self, state: &FastFill) -> bool {
        use Command as C;
        let enabled = super::super::project_view::enabled(state);
        let selection = state.annotations().has_selection()
            && !state.annotations().has_text_edit()
            && state.annotations().active_pointer().is_none();
        match self {
            C::Undo => enabled && state.annotations().can_undo(),
            C::Redo => enabled && state.annotations().can_redo(),
            C::AddPage => enabled && state.pages.can_add(),
            C::ImportPdf => enabled && state.pages.pdf_capacity() > 0,
            #[cfg(all(
                feature = "native-camera",
                any(target_os = "linux", target_os = "windows")
            ))]
            C::Camera => super::super::camera::can_open(state),
            C::EditPage => enabled && state.image().is_some(),
            C::Earlier => enabled && state.pages.active_index() > 0,
            C::Later => {
                enabled && state.pages.active_index() + 1 < state.pages.len()
            }
            C::Back | C::Front | C::DeleteSelected => enabled && selection,
            C::SelectAll => enabled && !state.annotations().is_empty(),
            C::DeleteAll => enabled && !state.annotations().is_empty(),
            C::ResetAngle => {
                enabled && state.annotations().can_reset_rotation()
            }
            C::SmartSnap => super::super::snap::can_toggle(state),
            C::Fill => enabled && state.annotations().current_fill().is_some(),
            // Closing the palette makes the existing help guard pass.
            C::Shortcuts => true,
            _ => enabled && state.image_picker == ImagePicker::Idle,
        }
    }

    pub(super) fn message(self, viewport: PageRect) -> Message {
        use Command as C;
        match self {
            C::New => Message::NewProject,
            C::Open => Message::OpenProject,
            C::Save => Message::SaveProject,
            #[cfg(not(target_arch = "wasm32"))]
            C::SaveAs => Message::SaveProjectAs,
            C::Export => Message::ExportPdf,
            C::Undo => Message::Undo,
            C::Redo => Message::Redo,
            C::Fit => Message::Fit,
            C::ActualSize => Message::OneToOne(viewport),
            C::AddPage => Message::AddPage,
            C::ImportPdf => Message::ImportPdf,
            #[cfg(all(
                feature = "native-camera",
                any(target_os = "linux", target_os = "windows")
            ))]
            C::Camera => Message::Camera(super::super::camera::Event::Begin),
            C::ReplaceImage => Message::AskReplacePage,
            C::EditPage => {
                Message::PageEdit(super::super::page_edit::Event::Begin)
            }
            C::DeletePage => Message::AskDeletePage,
            C::Earlier => Message::MovePageLeft,
            C::Later => Message::MovePageRight,
            C::Tool(tool) => Message::SetTool(tool),
            C::SelectAll => Message::SelectAll,
            C::Back => Message::SendToBack,
            C::Front => Message::BringToFront,
            C::DeleteSelected => Message::DeleteSelected,
            C::DeleteAll => Message::AskDeleteAll,
            C::ResetAngle => Message::ResetRotation,
            C::SmartSnap => {
                Message::SmartSnap(super::super::snap::Event::Toggle)
            }
            C::Fill => Message::ToggleFill,
            C::Shortcuts => Message::ShowShortcuts,
        }
    }
}
