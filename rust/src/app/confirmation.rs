//! Destructive prompts share one modal layer; workflow state stays with owners.

use super::project::{LeaveAction, Operation};
use super::{FastFill, ImagePicker, Message};
use crate::appearance::{self, CONTROL_GAP, action_button};
use iced::widget::{Row, column, container, mouse_area, opaque, row, text};
use iced::{Element, Fill};

pub(super) fn view(state: &FastFill) -> Option<Element<'static, Message>> {
    let action = |label: &'static str, message| {
        action_button(label, Some(message)).width(Fill)
    };
    if let Operation::Confirm(leave) = state.project.operation {
        let description = match leave {
            LeaveAction::New => "Save changes before starting a new project?",
            LeaveAction::Open => "Save changes before opening another project?",
            LeaveAction::Close(_) => "Save changes before closing FastFill?",
        };
        return Some(dialog(
            "Unsaved changes",
            description,
            row![
                action("Save", Message::SaveBeforeLeave)
                    .style(appearance::primary_action),
                action("Discard", Message::DiscardProject)
                    .style(appearance::destructive_action),
                action("Cancel", Message::KeepProject),
            ],
        ));
    }
    let (title, description, keep_label, keep, label, confirm) = if state
        .image_picker
        == ImagePicker::ConfirmReplace
    {
        (
            "Replace page?",
            "Replacing this page discards its annotations.",
            "Keep page",
            Message::KeepPage,
            "Replace",
            Message::ConfirmReplacePage,
        )
    } else if state.confirm_delete_page {
        (
            "Delete page?",
            "This removes the page and all its annotations.",
            "Keep page",
            Message::KeepPage,
            "Delete page",
            Message::ConfirmDeletePage,
        )
    } else if state.confirm_delete_all {
        (
            "Delete all annotations?",
            "This removes every annotation on the current page. You can undo it.",
            "Keep all",
            Message::KeepAll,
            "Delete all",
            Message::ConfirmDeleteAll,
        )
    } else {
        return None;
    };
    Some(dialog(
        title,
        description,
        row![
            action(keep_label, keep),
            action(label, confirm).style(appearance::destructive_action),
        ],
    ))
}

fn dialog(
    title: &'static str,
    description: &'static str,
    actions: Row<'static, Message>,
) -> Element<'static, Message> {
    let card = container(
        column![
            text(title).size(24),
            text(description).size(16),
            actions.spacing(CONTROL_GAP),
        ]
        .spacing(20),
    )
    .padding(24)
    .width(Fill)
    .max_width(520)
    .style(appearance::confirmation_card);
    let backdrop = container(card)
        .padding(16)
        .center_x(Fill)
        .center_y(Fill)
        .style(appearance::modal_scrim);
    // Native widgets absorb backdrop presses/touches/wheels without closing.
    // Existing workflow guards still own keyboard and late queued messages.
    opaque(
        mouse_area(backdrop)
            .on_press(Message::ModalBackdrop)
            .on_scroll(|_| Message::ModalBackdrop),
    )
}

#[cfg(test)]
mod tests;
