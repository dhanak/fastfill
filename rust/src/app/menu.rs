//! Small command popovers; 40-pixel triggers need wider command labels.

use super::{FastFill, Message};
use crate::appearance;
use crate::icons::{self, Glyph};
use iced::widget::{Space, button, column, container, mouse_area, opaque};
use iced::widget::{pin, responsive, row, scrollable, stack, text};
use iced::{Element, Fill, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Project,
    Page,
    More,
}

pub(super) fn overlay(state: &FastFill) -> Option<Element<'_, Message>> {
    let kind = state.open_menu?;
    if !super::project_view::enabled(state) {
        return None;
    }
    Some(responsive(move |size| popover(state, kind, size)).into())
}

#[allow(clippy::cast_precision_loss)] // Menus have fewer than ten items.
fn popover(state: &FastFill, kind: Kind, size: Size) -> Element<'_, Message> {
    let items = items(state, kind, size);
    let width = if kind == Kind::Project { 220.0 } else { 200.0 };
    let top = match kind {
        Kind::Project | Kind::More => appearance::HEADER_HEIGHT + 4.0,
        Kind::Page => appearance::HEADER_HEIGHT + 48.0,
    };
    let left = match kind {
        Kind::Project => 12.0,
        Kind::Page => 8.0,
        Kind::More => (size.width - width - 12.0).max(0.0),
    };
    let height =
        (items.len() as f32 * 44.0).min((size.height - top - 20.0).max(40.0));
    let entries = column(items.into_iter().map(|(label, glyph, command)| {
        button(
            row![icons::view::<Message>(glyph, false, true), text(label)]
                .spacing(10)
                .align_y(iced::Alignment::Center),
        )
        .width(Fill)
        .height(appearance::CONTROL_HEIGHT)
        .padding([8, 10])
        .on_press(Message::MenuAction(Box::new(command)))
        .style(appearance::secondary_action)
        .into()
    }))
    .spacing(4);
    let panel = container(scrollable(entries).height(height))
        .width(width)
        .padding(6)
        .style(container::bordered_box);
    stack![
        opaque(
            mouse_area(Space::new().width(Fill).height(Fill))
                .on_press(Message::CloseMenu),
        ),
        pin(opaque(panel)).x(left).y(top),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn items(
    state: &FastFill,
    kind: Kind,
    size: Size,
) -> Vec<(&'static str, Glyph, Message)> {
    match kind {
        Kind::Project => {
            let mut items = vec![
                ("New project", Glyph::NewProject, Message::NewProject),
                ("Open project", Glyph::OpenProject, Message::OpenProject),
            ];
            if cfg!(not(target_arch = "wasm32")) {
                items.push(("Save as", Glyph::SaveAs, Message::SaveProjectAs));
            }
            items
        }
        Kind::Page => {
            let mut items = Vec::new();
            if state.pages.active_index() > 0 {
                items.push((
                    "Move page earlier",
                    Glyph::MoveUp,
                    Message::MovePageLeft,
                ));
            }
            if state.pages.active_index() + 1 < state.pages.len() {
                items.push((
                    "Move page later",
                    Glyph::MoveDown,
                    Message::MovePageRight,
                ));
            }
            items
        }
        Kind::More => {
            let mut items = Vec::new();
            if size.width < 560.0 && state.annotations().can_undo() {
                items.push(("Undo", Glyph::Undo, Message::Undo));
            }
            if size.width < 560.0 && state.annotations().can_redo() {
                items.push(("Redo", Glyph::Redo, Message::Redo));
            }
            let viewport = super::view::canvas_viewport(size);
            items.extend([
                (
                    "Actual size",
                    Glyph::ActualSize,
                    Message::OneToOne(viewport),
                ),
                ("Fit page", Glyph::Fit, Message::Fit),
                ("Export PDF", Glyph::ExportPdf, Message::ExportPdf),
            ]);
            if super::palette::can_open(state) {
                items.push((
                    "Commands",
                    Glyph::Search,
                    Message::Palette(super::palette::Event::Toggle),
                ));
            }
            if super::help::can_open(state) {
                items.push(("Shortcuts", Glyph::Help, Message::ShowShortcuts));
            }
            items
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::controller::update;
    use crate::app::tests::click_element;
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};
    use iced::{Point, Renderer};

    #[test]
    fn escape_closes_open_menu() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::ToggleMenu(Kind::More));
        assert_eq!(state.open_menu, Some(Kind::More));
        let _ = update(&mut state, Message::CancelInteraction);
        assert_eq!(state.open_menu, None);
    }

    #[test]
    fn incidental_updates_do_not_close_open_menu() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::ToggleMenu(Kind::More));
        for message in [
            Message::ViewportResized,
            Message::ModifiersChanged(
                fastfill_rust::annotations::DragModifiers::default(),
            ),
            Message::FocusLost,
            Message::PageEdit(super::super::page_edit::Event::View(
                super::super::page_edit::viewer::Event::Compare(false),
            )),
        ] {
            let _ = update(&mut state, message);
            assert_eq!(state.open_menu, Some(Kind::More));
            assert!(overlay(&state).is_some());
        }
    }

    #[test]
    fn project_menu_has_wide_clickable_commands_and_outside_close() {
        let state = FastFill::default();
        let renderer =
            iced::futures::executor::block_on(<Renderer as Headless>::new(
                iced::Font::with_name("Fira Sans"),
                iced::Pixels(16.0),
                Some("tiny-skia"),
            ))
            .unwrap();
        let size = Size::new(500.0, 400.0);
        for (point, expected) in [
            (Point::new(155.0, 86.0), "new"),
            (Point::new(155.0, 130.0), "open"),
            (Point::new(300.0, 86.0), "close"),
        ] {
            let mut element = popover(&state, Kind::Project, size);
            let mut tree = widget::Tree::new(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let messages = click_element(
                &mut element,
                &mut tree,
                &node,
                &renderer,
                point,
                false,
            );
            assert_eq!(messages.len(), 1, "{expected}");
            assert!(
                matches!(
                    (expected, &messages[0]),
                    ("new", Message::MenuAction(action))
                        if matches!(**action, Message::NewProject)
                ) || matches!(
                    (expected, &messages[0]),
                    ("open", Message::MenuAction(action))
                        if matches!(**action, Message::OpenProject)
                ) || matches!(
                    (expected, &messages[0]),
                    ("close", Message::CloseMenu)
                )
            );
        }
    }

    #[test]
    fn open_project_menu_receives_clicks_above_the_workspace() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::ToggleMenu(Kind::Project));
        let renderer =
            iced::futures::executor::block_on(<Renderer as Headless>::new(
                iced::Font::with_name("Fira Sans"),
                iced::Pixels(16.0),
                Some("tiny-skia"),
            ))
            .unwrap();
        let size = Size::new(500.0, 400.0);
        let mut element = super::super::view::view(&state);
        let mut tree = widget::Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        let messages = click_element(
            &mut element,
            &mut tree,
            &node,
            &renderer,
            Point::new(155.0, 86.0),
            false,
        );
        assert!(matches!(
            messages.as_slice(),
            [Message::MenuAction(action)]
                if matches!(**action, Message::NewProject)
        ));
    }
}
