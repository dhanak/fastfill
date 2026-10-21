//! Searchable actions. Existing message handlers remain command owners.

mod commands;
mod view;

use super::{FastFill, Message, TEXT_EDITOR_ID};
use fastfill_rust::annotations::DragModifiers;
use fastfill_rust::geometry::Rect as PageRect;
use iced::{Task, keyboard};

pub(super) use commands::Command;
pub(super) fn view(state: &FastFill) -> Option<iced::Element<'_, Message>> {
    view::view(state)
}

const INPUT_ID: &str = "command-palette-input";

#[derive(Default)]
pub(super) struct Palette {
    pub(super) open: bool,
    pub(super) query: String,
    pub(super) selected: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Toggle,
    Close,
    ShowShortcuts,
    Query(String),
    Move(i8),
    Run(PageRect),
    Choose(Command, PageRect),
}

pub(super) fn can_open(state: &FastFill) -> bool {
    super::help::can_open(state) && !state.palette.open
}

pub(super) fn shortcut(
    key: keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<Message> {
    use keyboard::key::Named;
    let event = match key {
        keyboard::Key::Named(Named::Escape) => Event::Close,
        keyboard::Key::Named(Named::F1) => Event::ShowShortcuts,
        keyboard::Key::Named(Named::ArrowDown) => Event::Move(1),
        keyboard::Key::Named(Named::ArrowUp) => Event::Move(-1),
        keyboard::Key::Character(value)
            if (modifiers.command() || modifiers.control()) && value == "." =>
        {
            Event::Close
        }
        _ => return None,
    };
    Some(Message::Palette(event))
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if let Message::Palette(Event::Toggle) = message {
        if state.palette.open {
            return Some(close(state));
        }
        if can_open(state) {
            state.open_menu = None;
            state.palette.open = true;
            state.palette.query.clear();
            state.palette.selected = first_available(state);
            state.input_epoch = state.input_epoch.wrapping_add(1);
            return Some(iced::widget::operation::focus(INPUT_ID));
        }
        return Some(Task::none());
    }
    if !state.palette.open {
        return None;
    }
    let task = match message {
        Message::Palette(Event::Close | Event::Toggle)
        | Message::CancelInteraction => close(state),
        Message::Palette(Event::ShowShortcuts) => {
            state.palette.open = false;
            super::help::update(state, &Message::ShowShortcuts)
                .unwrap_or_else(Task::none)
        }
        Message::Palette(Event::Query(query)) => {
            state.palette.query.clone_from(query);
            state.palette.selected = first_available(state);
            scroll_selected(state)
        }
        Message::Palette(Event::Move(delta)) => {
            let results = results(state);
            if let Some(index) = state.palette.selected {
                let mut next = index.cast_signed() + isize::from(*delta);
                while (0..results.len().cast_signed()).contains(&next) {
                    if results[next.cast_unsigned()].available(state) {
                        state.palette.selected = Some(next.cast_unsigned());
                        break;
                    }
                    next += isize::from(*delta);
                }
            }
            scroll_selected(state)
        }
        Message::Palette(Event::Run(viewport)) => {
            let command = state
                .palette
                .selected
                .and_then(|i| results(state).get(i).copied());
            choose(state, command, *viewport)
        }
        Message::Palette(Event::Choose(command, viewport)) => {
            choose(state, Some(*command), *viewport)
        }
        Message::ModifiersChanged(modifiers) => {
            state.modifiers = *modifiers;
            Task::none()
        }
        Message::FocusLost => {
            state.modifiers = DragModifiers::default();
            Task::none()
        }
        Message::CloseRequested(_) => {
            state.palette.open = false;
            return None;
        }
        Message::ViewportResized => Task::none(),
        #[cfg(not(target_arch = "wasm32"))]
        Message::Recovery(_) => return None,
        Message::SmartSnap(super::snap::Event::Ready(..))
        | Message::PageEdit(super::page_edit::Event::Ready(..)) => return None,
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Message::Camera(super::camera::Event::Framed(..)) => return None,
        _ => Task::none(),
    };
    Some(task)
}

fn choose(
    state: &mut FastFill,
    command: Option<Command>,
    viewport: PageRect,
) -> Task<Message> {
    let Some(command) = command.filter(|command| {
        command.available(state) && results(state).contains(command)
    }) else {
        return Task::none();
    };
    state.palette.open = false;
    state.input_epoch = state.input_epoch.wrapping_add(1);
    Task::done(command.message(viewport))
}

fn close(state: &mut FastFill) -> Task<Message> {
    state.palette.open = false;
    state.input_epoch = state.input_epoch.wrapping_add(1);
    if state.font_size_input.is_some() {
        iced::widget::operation::focus("font-size-input")
    } else if state.stroke_width_input.is_some() {
        iced::widget::operation::focus("stroke-width-input")
    } else if state.annotations().has_text_edit() {
        iced::widget::operation::focus(TEXT_EDITOR_ID)
    } else {
        Task::none()
    }
}

fn first_available(state: &FastFill) -> Option<usize> {
    results(state)
        .iter()
        .position(|command| command.available(state))
}

fn results(state: &FastFill) -> Vec<Command> {
    commands::all()
        .into_iter()
        .filter(|command| {
            matches_query(
                &state.palette.query,
                &format!(
                    "{} {} {}",
                    command.label(),
                    command.group(),
                    command.aliases()
                ),
            )
        })
        .collect()
}

fn matches_query(query: &str, haystack: &str) -> bool {
    let haystack = haystack.to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

#[allow(clippy::cast_precision_loss)] // Palette has fewer than 50 rows.
fn scroll_selected(state: &FastFill) -> Task<Message> {
    let offset = state.palette.selected.unwrap_or(0).saturating_sub(2);
    iced::widget::operation::scroll_to(
        view::LIST_ID,
        iced::widget::scrollable::AbsoluteOffset {
            x: 0.0,
            y: offset as f32 * 44.0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::controller::update;
    use crate::app::tests::{annotated_page, click_element, paste_text};
    use fastfill_rust::annotations::{Pointer, Tool};
    use fastfill_rust::geometry::Point as PagePoint;
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};
    use iced::{Point, Renderer, Size, Theme, mouse};

    #[test]
    fn search_matches_words_across_labels_groups_and_aliases() {
        assert!(matches_query("page add", "Add image page import"));
        assert!(matches_query("PDF ExPoRt", "Export PDF project"));
        assert!(matches_query("magnify", "Zoom in magnify"));
        assert!(!matches_query("page remove", "Add image page"));
    }

    #[test]
    fn disabled_commands_remain_visible_but_cannot_run() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::Palette(Event::Toggle));
        assert!(state.palette.open);
        let _ = update(
            &mut state,
            Message::Palette(Event::Query("delete selection".into())),
        );
        assert!(results(&state).contains(&Command::DeleteSelected));
        assert_eq!(state.palette.selected, None);
        let _ = update(
            &mut state,
            Message::Palette(Event::Choose(
                Command::DeleteSelected,
                PageRect::new(0.0, 0.0, 100.0, 100.0),
            )),
        );
        assert!(state.palette.open);
    }

    #[test]
    fn select_all_command_is_searchable_and_needs_annotations() {
        let mut empty = FastFill::default();
        empty.palette.query = "select all".into();
        assert!(results(&empty).contains(&Command::SelectAll));
        assert!(!Command::SelectAll.available(&empty));
        assert!(Command::SelectAll.available(&annotated_page()));
        assert!(matches!(
            Command::SelectAll.message(PageRect::new(0.0, 0.0, 1.0, 1.0)),
            Message::SelectAll,
        ));
    }

    #[test]
    fn escape_closes_palette_without_accepting_pending_text() {
        let mut state = annotated_page();
        let _ = update(&mut state, Message::SetTool(Tool::Text));
        let _ = update(
            &mut state,
            Message::DrawPressed(
                Pointer::Mouse,
                PagePoint::new(200.0, 250.0),
                1.0,
            ),
        );
        paste_text(&mut state, "Still pending");
        state.font_size_input = Some("22.5".into());
        let preview = state.annotations().text_preview().unwrap();
        let identity = state.pages.identity();
        let _ = update(&mut state, Message::Palette(Event::Toggle));
        assert!(state.palette.open);
        let _ = update(&mut state, Message::CancelInteraction);
        assert!(!state.palette.open);
        assert_eq!(state.annotations().text_preview().unwrap(), preview);
        assert_eq!(state.font_size_input.as_deref(), Some("22.5"));
        assert_eq!(state.pages.identity(), identity);
    }

    #[test]
    fn shortcut_routes_to_palette_and_modal_owns_navigation_keys() {
        let command = keyboard::Modifiers::COMMAND;
        let open = super::super::controller::shortcut_message(
            keyboard::Key::Character(".".into()),
            command,
            iced::event::Status::Ignored,
        );
        assert!(matches!(open, Some(Message::Palette(Event::Toggle))));
        assert!(matches!(
            shortcut(
                keyboard::Key::Named(keyboard::key::Named::ArrowDown),
                command
            ),
            Some(Message::Palette(Event::Move(1))),
        ));
        assert!(matches!(
            shortcut(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                command
            ),
            Some(Message::Palette(Event::Close)),
        ));
        let f1 = shortcut(
            keyboard::Key::Named(keyboard::key::Named::F1),
            keyboard::Modifiers::empty(),
        );
        assert!(matches!(f1, Some(Message::Palette(Event::ShowShortcuts)),));
    }

    #[test]
    fn f1_switches_from_palette_to_shortcut_reference() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::Palette(Event::Toggle));
        assert!(state.palette.open);
        let f1 = shortcut(
            keyboard::Key::Named(keyboard::key::Named::F1),
            keyboard::Modifiers::empty(),
        )
        .expect("F1 route");
        let _ = update(&mut state, f1);
        assert!(!state.palette.open);
        assert!(state.show_shortcuts);
    }

    #[test]
    fn palette_result_runs_by_mouse_and_touch_without_backdrop_close() {
        let renderer =
            iced::futures::executor::block_on(<Renderer as Headless>::new(
                iced::Font::with_name("Fira Sans"),
                iced::Pixels(16.0),
                Some("tiny-skia"),
            ))
            .unwrap();
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::Palette(Event::Toggle));
        let _ = update(
            &mut state,
            Message::Palette(Event::Query("fit page".into())),
        );
        let size = Size::new(640.0, 480.0);
        for touch in [false, true] {
            let mut element = view::overlay(&state, size);
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
                Point::new(200.0, 148.0),
                touch,
            );
            assert!(matches!(
                messages.as_slice(),
                [Message::Palette(Event::Choose(Command::Fit, _))]
            ));
        }
    }

    #[test]
    fn palette_renders_in_both_themes_and_window_widths() {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::Palette(Event::Toggle));
        for (size, theme) in [
            (Size::new(1_200, 800), Theme::Dark),
            (Size::new(420, 640), Theme::Light),
        ] {
            let pixels = crate::app::tests::render_themed_element(
                view(&state).unwrap(),
                size,
                1,
                1.0,
                &theme,
                mouse::Cursor::Unavailable,
            );
            assert_eq!(
                pixels.len(),
                size.width as usize * size.height as usize * 4
            );
            let snapshot = if theme == Theme::Dark {
                "FASTFILL_PALETTE_SNAPSHOT"
            } else {
                "FASTFILL_PALETTE_NARROW_SNAPSHOT"
            };
            if let Some(path) = std::env::var_os(snapshot) {
                image::save_buffer(
                    path,
                    &pixels,
                    size.width,
                    size.height,
                    image::ColorType::Rgba8,
                )
                .expect("palette snapshot");
            }
        }
    }
}
