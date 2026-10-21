//! Project identity and file actions in the compact application header.

use super::{FastFill, ImagePicker, Message};
use crate::appearance::{self, primary_action};
use crate::icons::{self, Glyph};
use iced::widget::{button, row, text};
use iced::{Element, Length};

pub(super) fn enabled(state: &FastFill) -> bool {
    state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && state.dragged_page.is_none()
}

pub(super) fn menu(state: &FastFill, width: f32) -> Element<'static, Message> {
    let available = enabled(state);
    let name = format!(
        "{}{}{}",
        if width >= 220.0 { "FastFill · " } else { "" },
        if width < 190.0 {
            state
                .project
                .name
                .strip_suffix(".docscan")
                .unwrap_or(&state.project.name)
        } else {
            &state.project.name
        },
        if state.project.is_dirty(&state.pages) {
            "*"
        } else {
            ""
        }
    );
    button(
        row![
            text(name)
                .size(14)
                .width(iced::Fill)
                .wrapping(iced::advanced::text::Wrapping::None),
            icons::view::<Message>(
                if state.open_menu == Some(super::menu::Kind::Project) {
                    Glyph::ChevronUp
                } else {
                    Glyph::ChevronDown
                },
                false,
                available,
            ),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .width(width)
    .height(appearance::CONTROL_HEIGHT)
    .padding([8, 10])
    .on_press_maybe(
        available.then_some(Message::ToggleMenu(super::menu::Kind::Project)),
    )
    .style(appearance::secondary_action)
    .into()
}

pub(super) fn save(
    state: &FastFill,
    icon_only: bool,
) -> Element<'static, Message> {
    let available = enabled(state);
    let content: Element<'static, Message> = if icon_only {
        icons::view(Glyph::Save, true, available)
    } else {
        row![
            icons::view::<Message>(Glyph::Save, true, available),
            text("Save").size(14)
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    };
    let button = button(content)
        .height(appearance::CONTROL_HEIGHT)
        .width(if icon_only {
            Length::Fixed(appearance::CONTROL_HEIGHT)
        } else {
            Length::Shrink
        })
        .padding([8, if icon_only { 8 } else { 12 }])
        .on_press_maybe(available.then_some(Message::SaveProject))
        .style(primary_action);
    if icon_only {
        super::toolbar::hint(button, "Save project", available)
    } else {
        button.into()
    }
}

pub(super) fn export(state: &FastFill) -> Element<'static, Message> {
    let available = enabled(state);
    button(
        row![
            icons::view::<Message>(Glyph::ExportPdf, false, available),
            text("Export PDF").size(14),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .height(appearance::CONTROL_HEIGHT)
    .padding([8, 12])
    .on_press_maybe(available.then_some(Message::ExportPdf))
    .style(appearance::secondary_action)
    .into()
}

pub(super) fn view_action(
    glyph: Glyph,
    label: &'static str,
    message: Option<Message>,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    button(
        row![
            icons::view::<Message>(glyph, false, enabled),
            text(label).size(14),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .height(appearance::CONTROL_HEIGHT)
    .padding([8, 10])
    .on_press_maybe(message)
    .style(appearance::secondary_action)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{click_element, render_themed_element};
    use iced::advanced::renderer::Headless;
    use iced::advanced::{Layout, layout, widget};
    use iced::{Point, Renderer, Theme, mouse};

    #[test]
    fn labeled_file_actions_center_icons_in_forty_pixel_buttons() {
        let state = FastFill::default();
        let renderer =
            iced::futures::executor::block_on(<Renderer as Headless>::new(
                iced::Font::with_name("Fira Sans"),
                iced::Pixels(16.0),
                Some("tiny-skia"),
            ))
            .unwrap();
        for mut action in [save(&state, false), export(&state)] {
            let mut tree = widget::Tree::new(action.as_widget());
            let node = action.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(
                    iced::Size::new(160.0, 40.0),
                    iced::Size::new(160.0, 40.0),
                ),
            );
            let button = Layout::new(&node);
            let content = button.children().next().unwrap().bounds();
            assert_eq!(content.y, 8.0);
            assert_eq!(content.height, 24.0);
        }
    }

    #[test]
    fn project_button_opens_command_menu_for_mouse_and_touch() {
        let state = FastFill::default();
        let renderer =
            iced::futures::executor::block_on(<Renderer as Headless>::new(
                iced::Font::with_name("Fira Sans"),
                iced::Pixels(16.0),
                Some("tiny-skia"),
            ))
            .unwrap();
        for touch in [false, true] {
            let mut menu = menu(&state, 240.0);
            let mut tree = widget::Tree::new(menu.as_widget());
            let size = iced::Size::new(240.0, 40.0);
            let node = menu.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let messages = click_element(
                &mut menu,
                &mut tree,
                &node,
                &renderer,
                Point::new(120.0, 20.0),
                touch,
            );
            assert!(matches!(
                messages.as_slice(),
                [Message::ToggleMenu(super::super::menu::Kind::Project)]
            ));
        }
    }

    #[test]
    fn project_chevron_points_up_only_while_menu_is_open() {
        let mut state = FastFill::default();
        let render = |state: &FastFill| {
            render_themed_element(
                menu(state, 240.0),
                iced::Size::new(240, 40),
                1,
                1.0,
                &Theme::Dark,
                mouse::Cursor::Unavailable,
            )
        };
        let closed = render(&state);
        state.open_menu = Some(super::super::menu::Kind::Project);
        assert_ne!(render(&state), closed);
        state.open_menu = None;
        assert_eq!(render(&state), closed);
    }
}
