//! Read-only shortcut reference; never owns document edits or history.

use super::{FastFill, ImagePicker, Message, TEXT_EDITOR_ID};
use crate::appearance::{self, CONTROL_HEIGHT};
use fastfill_rust::annotations::DragModifiers;
use iced::widget::{
    button, column, container, mouse_area, opaque, row, scrollable, table, text,
};
use iced::{Element, Fill, FillPortion, Font, Task, font};

const COMMAND: &str = if cfg!(target_os = "macos") {
    "Cmd"
} else {
    "Ctrl"
};

pub(super) fn can_open(state: &FastFill) -> bool {
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    if state.camera.is_open() {
        return false;
    }
    !state.show_shortcuts
        && state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.page_editor.is_open()
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none()
        && state.annotations().active_pointer().is_none()
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if matches!(message, Message::ShowShortcuts) {
        let task = if can_open(state) {
            state.show_shortcuts = true;
            state.input_epoch = state.input_epoch.wrapping_add(1);
            iced_runtime::task::widget(
                iced::advanced::widget::operation::focusable::unfocus(),
            )
        } else {
            Task::none()
        };
        return Some(task);
    }
    if !state.show_shortcuts {
        return None;
    }
    match message {
        Message::HideShortcuts | Message::CancelInteraction => {
            state.show_shortcuts = false;
            state.input_epoch = state.input_epoch.wrapping_add(1);
            return Some(if state.font_size_input.is_some() {
                iced::widget::operation::focus("font-size-input")
            } else if state.annotations().has_text_edit() {
                iced::widget::operation::focus(TEXT_EDITOR_ID)
            } else {
                Task::none()
            });
        }
        Message::CloseRequested(_) => {
            state.show_shortcuts = false;
            return None; // Normal save/discard guards still own window close.
        }
        Message::ModifiersChanged(modifiers) => state.modifiers = *modifiers,
        Message::FocusLost => state.modifiers = DragModifiers::default(),
        Message::ViewportResized => {
            state.input_epoch = state.input_epoch.wrapping_add(1);
        }
        // Let existing owners finish work; never strand their running jobs.
        Message::SmartSnap(super::snap::Event::Ready(..))
        | Message::PageEdit(super::page_edit::Event::Ready(..)) => return None,
        #[cfg(not(target_arch = "wasm32"))]
        Message::Recovery(_) => return None,
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Message::Camera(super::camera::Event::Framed(..)) => return None,
        _ => {} // Ignore every other queued background UI command.
    }
    Some(Task::none())
}

pub(super) fn view(state: &FastFill) -> Option<Element<'static, Message>> {
    if !state.show_shortcuts {
        return None;
    }
    let bold = Font {
        weight: font::Weight::Bold,
        ..Font::DEFAULT
    };
    let table = table::table(
        [
            table::column(text("Context").font(bold), |entry: Entry| {
                text(entry.0).size(13)
            })
            .width(FillPortion(2)),
            table::column(text("Shortcut").font(bold), move |entry: Entry| {
                text(shortcut_label(entry.1)).font(bold).size(14)
            })
            .width(FillPortion(3)),
            table::column(text("Action").font(bold), |entry: Entry| {
                text(entry.2).size(14)
            })
            .width(FillPortion(5)),
        ],
        ENTRIES.iter().copied().filter(|entry| {
            entry.0 != "Camera"
                || cfg!(all(
                    feature = "native-camera",
                    not(target_arch = "wasm32")
                ))
        }),
    )
    .padding_x(8)
    .padding_y(10)
    .separator_x(0);
    let card = container(
        column![
            row![
                text("Shortcuts").size(24).width(Fill),
                button("Close")
                    .height(CONTROL_HEIGHT)
                    .on_press(Message::HideShortcuts)
                    .style(button::secondary),
            ]
            .align_y(iced::Alignment::Center),
            scrollable(container(table).padding(iced::Padding {
                right: 12.0,
                ..iced::Padding::ZERO
            }))
            .height(Fill),
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Fill)
    .height(Fill)
    .max_width(900)
    .max_height(720)
    .style(appearance::confirmation_card);
    Some(opaque(
        mouse_area(
            container(card)
                .padding(16)
                .center_x(Fill)
                .center_y(Fill)
                .style(appearance::modal_scrim),
        )
        .on_press(Message::ModalBackdrop)
        .on_scroll(|_| Message::ModalBackdrop),
    ))
}

fn shortcut_label(shortcut: &str) -> String {
    shortcut.replace("Mod", COMMAND)
}

type Entry = (&'static str, &'static str, &'static str);

// Describe implemented input only; tool names are not letter-key shortcuts.
const ENTRIES: &[Entry] = &[
    ("Help", "F1 / Escape", "Open help / close help."),
    (
        "Project",
        "Mod+N",
        "New project; ask before discarding changes.",
    ),
    ("Project", "Mod+O", "Open a .docscan project."),
    (
        "Project",
        "Mod+S",
        "Save project; download a copy in the browser.",
    ),
    (
        "Project",
        "Mod+Shift+S",
        "Save as; download again in the browser.",
    ),
    ("History", "Mod+Z", "Undo the last edit on this page."),
    (
        "History",
        "Mod+Y / Mod+Shift+Z",
        "Redo the last undone edit.",
    ),
    ("Selection", "Click / tap", "Select an object with Select."),
    (
        "Selection",
        "Drag",
        "Marquee from empty space or an unselected object.",
    ),
    (
        "Selection",
        "Ctrl+click",
        "Add/remove one object from the selection.",
    ),
    (
        "Selection",
        "Ctrl+drag",
        "Force a fresh marquee, even over selection.",
    ),
    (
        "Selection",
        "Drag selected",
        "Move the selected object or group.",
    ),
    (
        "Selection",
        "Delete",
        "Delete selection; text fields own their Delete.",
    ),
    (
        "Selection",
        "Escape",
        "Cancel a pending edit, otherwise clear selection.",
    ),
    (
        "Move / resize",
        "Shift / Alt",
        "Force move / resize with visible handles.",
    ),
    (
        "Draw / resize",
        "Ctrl+drag",
        "Square/circle or 45° line/arrow. In Select, grab handle before Ctrl.",
    ),
    (
        "Rotation",
        "Ctrl+drag knob",
        "Snap supported objects to 45° increments.",
    ),
    (
        "Text tool",
        "Click / tap",
        "Start text or edit at the clicked caret; snapped fields reuse text.",
    ),
    (
        "Inline text",
        "Enter",
        "Accept text and its pending formatting.",
    ),
    (
        "Inline text",
        "Shift+Enter",
        "Insert a line break; no automatic wrapping.",
    ),
    (
        "Inline text",
        "Click away",
        "Accept text; tool changes also accept.",
    ),
    (
        "Inline text",
        "Escape",
        "Revert pending text AND formatting.",
    ),
    (
        "Text fields",
        "Mod+A / C / X / V",
        "Select all / copy / cut / paste.",
    ),
    (
        "Font size",
        "Wheel / − / +",
        "Step by 1 pt; range 6–144 pt.",
    ),
    (
        "Font size",
        "Enter / Escape",
        "Apply / discard the numeric draft first.",
    ),
    (
        "Navigation",
        "Wheel",
        "Zoom at the pointer in any tool while idle.",
    ),
    ("Navigation", "Right-drag", "Pan in any tool while idle."),
    ("Navigate tool", "Drag / one finger", "Pan the page."),
    ("Navigate tool", "Two fingers", "Pan and pinch to zoom."),
    (
        "Page rail",
        "Drag thumbnail",
        "Preview reordering; drop in the rail.",
    ),
    (
        "Page rail",
        "Escape / drop outside",
        "Cancel reordering; keep page order.",
    ),
    ("Edit page", "Drag corner", "Adjust crop freely."),
    (
        "Edit page",
        "Ctrl+drag corner",
        "Snap to nearby image edges/corners.",
    ),
    (
        "Edit page",
        "Escape",
        "Cancel pending corrections; keep the page.",
    ),
    (
        "Camera",
        "Escape",
        "Cancel capture; do not add the pending photo.",
    ),
];

#[cfg(test)]
mod tests;
