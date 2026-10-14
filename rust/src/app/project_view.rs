use super::project::{LeaveAction, Operation};
use super::{FastFill, ImagePicker, Message};
use crate::appearance::CONTROL_HEIGHT;
use iced::widget::{button, container, row, text};
use iced::{Element, Fill};

pub(super) fn controls(state: &FastFill) -> Element<'_, Message> {
    let enabled = state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none();
    let control = |label: &'static str, message| {
        button(label)
            .on_press_maybe(enabled.then_some(message))
            .height(CONTROL_HEIGHT)
    };
    row![
        text(format!(
            "FastFill — {}{}",
            state.project.name,
            if state.project.is_dirty(&state.pages) {
                " *"
            } else {
                ""
            }
        ))
        .size(24)
        .width(Fill),
        control("New", Message::NewProject),
        control("Open", Message::OpenProject),
        control("Save", Message::SaveProject),
        control("Save as", Message::SaveProjectAs),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

pub(super) fn confirmation(state: &FastFill) -> Option<Element<'_, Message>> {
    let Operation::Confirm(action) = state.project.operation else {
        return None;
    };
    let description = match action {
        LeaveAction::New => "Save changes before starting a new project?",
        LeaveAction::Open => "Save changes before opening another project?",
        LeaveAction::Close(_) => "Save changes before closing FastFill?",
    };
    Some(
        container(
            row![
                text(description).width(Fill),
                button("Save")
                    .on_press(Message::SaveBeforeLeave)
                    .height(CONTROL_HEIGHT),
                button("Discard")
                    .on_press(Message::DiscardProject)
                    .height(CONTROL_HEIGHT),
                button("Cancel")
                    .on_press(Message::KeepProject)
                    .height(CONTROL_HEIGHT),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(8),
        )
        .padding(8)
        .style(container::warning)
        .into(),
    )
}
