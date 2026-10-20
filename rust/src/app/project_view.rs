use super::{FastFill, ImagePicker, Message};
use crate::appearance::CONTROL_HEIGHT;
use iced::widget::{button, row, text};
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
    ]
    .extend(
        cfg!(not(target_arch = "wasm32"))
            .then(|| control("Save as", Message::SaveProjectAs).into()),
    )
    .push(control("Export PDF", Message::ExportPdf))
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_controls_match_the_platform_save_workflow() {
        let state = FastFill::default();
        let header = controls(&state);
        // Title, New, Open, Save, Export PDF; native adds Save as.
        let expected = 5 + usize::from(cfg!(not(target_arch = "wasm32")));
        assert_eq!(header.as_widget().children().len(), expected);
    }
}
