use super::{Event, FastFill, Message, results};
use crate::appearance::{self, action_button};
use crate::icons;
use iced::widget::{Space, button, column, container, mouse_area, opaque};
use iced::widget::{responsive, row, scrollable, text, text_input};
use iced::{Element, Fill, Length, Size};

pub(super) const LIST_ID: &str = "command-palette-list";

pub(super) fn view(state: &FastFill) -> Option<Element<'_, Message>> {
    state
        .palette
        .open
        .then(|| responsive(move |size| overlay(state, size)).into())
}

pub(super) fn overlay(state: &FastFill, size: Size) -> Element<'_, Message> {
    let viewport = super::super::view::canvas_viewport(size);
    let candidates = results(state);
    let entries =
        column(candidates.iter().enumerate().map(|(index, command)| {
            let command = *command;
            let available = command.available(state);
            let selected = state.palette.selected == Some(index);
            let shortcut = command.shortcut();
            let hint = if !available {
                "Unavailable".to_owned()
            } else if shortcut.is_empty() {
                String::new()
            } else if shortcut == "F1" || shortcut == "Delete" {
                shortcut.to_owned()
            } else {
                format!(
                    "{}+{shortcut}",
                    if cfg!(target_os = "macos") {
                        "Cmd"
                    } else {
                        "Ctrl"
                    }
                )
            };
            let mut content = row![
                icons::view::<Message>(command.glyph(), selected, available),
                text(command.label()).size(15).width(Fill),
            ];
            if size.width >= 520.0 {
                content = content
                    .push(text(command.group()).size(12))
                    .push(text(hint).size(12));
            }
            content = content
                .spacing(10)
                .width(Fill)
                .align_y(iced::Alignment::Center);
            button(content)
                .height(40)
                .width(Fill)
                .padding([8, 10])
                .on_press_maybe(available.then_some(Message::Palette(
                    Event::Choose(command, viewport),
                )))
                .style(move |theme, status| {
                    appearance::tool_button(theme, status, selected)
                })
                .into()
        }))
        .spacing(4);
    let list: Element<'_, Message> = if candidates.is_empty() {
        container(text("No matching commands").size(14))
            .padding(12)
            .width(Fill)
            .into()
    } else {
        scrollable(entries).id(LIST_ID).height(Fill).into()
    };
    let width = (size.width - 32.0).clamp(1.0, 640.0);
    let height = (size.height - 32.0).clamp(1.0, 560.0);
    let card = container(
        column![
            row![
                text("Commands").size(22).width(Fill),
                action_button("Close", Some(Message::Palette(Event::Close))),
            ]
            .align_y(iced::Alignment::Center),
            text_input("Search commands", &state.palette.query)
                .id(super::INPUT_ID)
                .on_input(|query| Message::Palette(Event::Query(query)))
                .on_submit(Message::Palette(Event::Run(viewport)))
                .padding(10)
                .size(16),
            list,
            row![
                text("↑↓ Select · Enter Run · Esc Close").size(12),
                Space::new().width(Length::Fill),
                text("F1 for shortcuts").size(12),
            ],
        ]
        .spacing(10),
    )
    .width(width)
    .height(height)
    .padding(16)
    .style(appearance::confirmation_card);
    opaque(
        mouse_area(
            container(card)
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill)
                .style(appearance::modal_scrim),
        )
        .on_press(Message::Palette(Event::Close)),
    )
}
