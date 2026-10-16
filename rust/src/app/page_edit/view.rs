use super::canvas::CropCanvas;
use super::{Draft, Event, FastFill, Message};
use crate::appearance::CONTROL_HEIGHT;
use fastfill_rust::geometry::Size;
use fastfill_rust::project::PageFilterMode;
use iced::widget::{
    Space, button, canvas, column, container, image, row, scrollable, slider,
    text,
};
use iced::{Element, Fill};

pub(in crate::app) fn view(state: &FastFill) -> Element<'_, Message> {
    let draft = state.page_editor.draft.as_ref().expect("active editor");
    let ready = draft
        .ready
        .as_ref()
        .is_some_and(|p| p.settings == draft.settings && draft.error.is_none());
    let page = state.pages.current();
    #[allow(clippy::cast_precision_loss)]
    let source_size = Size::new(
        page.metadata.source_pixel_width as f32,
        page.metadata.source_pixel_height as f32,
    );
    let source = CropCanvas {
        image: draft.original.as_ref().map(|original| &original.handle),
        source_size,
        crop: draft.settings.crop,
        epoch: state.input_epoch,
        background: false,
    };
    let original = iced::widget::stack![
        Space::new().width(Fill).height(Fill),
        canvas(CropCanvas {
            background: true,
            ..source
        })
        .width(Fill)
        .height(Fill),
        canvas(source).width(Fill).height(Fill),
    ]
    .width(Fill)
    .height(Fill)
    .clip(true);
    let handle = draft.ready.as_ref().map(|p| &p.handle).or_else(|| {
        state
            .image()
            .map(crate::picker::ImportedImage::display_handle)
    });
    let preview: Element<'_, Message> = handle.map_or_else(
        || Space::new().height(180).into(),
        |handle| {
            image(handle.clone())
                .width(Fill)
                .height(180)
                .content_fit(iced::ContentFit::Contain)
                .into()
        },
    );
    let sidebar = column![
        text("Corrected preview"),
        preview,
        scrollable(options(draft)).height(Fill)
    ]
    .spacing(8)
    .width(312)
    .height(Fill);
    column![
        row![
            text(format!("Edit page {}", state.pages.active_index() + 1))
                .size(24)
                .width(Fill),
            button("Cancel")
                .on_press(Message::PageEdit(Event::Cancel))
                .height(CONTROL_HEIGHT)
                .width(100),
            button("Accept")
                .on_press_maybe(
                    ready.then_some(Message::PageEdit(Event::Accept))
                )
                .height(CONTROL_HEIGHT)
                .width(100),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        status(draft, ready),
        row![
            column![text("Original — drag corner handles"), original]
                .spacing(8)
                .width(Fill)
                .height(Fill),
            sidebar
        ]
        .spacing(16)
        .height(Fill),
    ]
    .spacing(12)
    .padding(16)
    .width(Fill)
    .height(Fill)
    .into()
}

fn status(draft: &Draft, ready: bool) -> Element<'_, Message> {
    if let Some(error) = &draft.error {
        container(text(error))
            .padding(8)
            .style(container::danger)
            .into()
    } else {
        text(if let Some(notice) = &draft.notice {
            notice.as_str()
        } else if ready {
            "Drag corners on the original. Accept applies all changes."
        } else {
            "Updating corrected preview…"
        })
        .into()
    }
}

fn options(draft: &Draft) -> Element<'static, Message> {
    let choose = |label, mode| {
        button(label)
            .on_press(Message::PageEdit(Event::Mode(mode)))
            .height(CONTROL_HEIGHT)
            .width(Fill)
            .style(if draft.settings.filter.mode == mode {
                button::primary
            } else {
                button::secondary
            })
    };
    column![
        button("Full frame")
            .width(Fill)
            .height(CONTROL_HEIGHT)
            .on_press(Message::PageEdit(Event::FullFrame)),
        row![
            button("Rotate left")
                .width(Fill)
                .height(CONTROL_HEIGHT)
                .on_press(Message::PageEdit(Event::Rotate(-1))),
            button("Rotate right")
                .width(Fill)
                .height(CONTROL_HEIGHT)
                .on_press(Message::PageEdit(Event::Rotate(1))),
        ]
        .spacing(8),
        text("Color mode"),
        row![
            choose("Original", PageFilterMode::Original),
            choose("Enhanced", PageFilterMode::EnhancedColor)
        ]
        .spacing(8),
        row![
            choose("Grayscale", PageFilterMode::Grayscale),
            choose("B&W", PageFilterMode::BlackAndWhite)
        ]
        .spacing(8),
        tone("Brightness", draft.settings.brightness, Event::Brightness),
        tone("Contrast", draft.settings.filter.contrast, Event::Contrast),
    ]
    .spacing(8)
    .into()
}

fn tone(
    label: &'static str,
    value: i32,
    event: fn(i32) -> Event,
) -> Element<'static, Message> {
    column![
        row![text(label).width(Fill), text(format!("{value:+}"))],
        row![
            button("−").height(CONTROL_HEIGHT).width(44).on_press_maybe(
                (value > -100).then(|| Message::PageEdit(event(value - 1)))
            ),
            slider(-100..=100, value, move |value| Message::PageEdit(event(
                value
            ))),
            button("+").height(CONTROL_HEIGHT).width(44).on_press_maybe(
                (value < 100).then(|| Message::PageEdit(event(value + 1)))
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(4)
    .into()
}

pub(in crate::app) fn warning(
    state: &FastFill,
) -> Option<Element<'_, Message>> {
    let message = state.page_editor.warning.as_ref()?;
    Some(
        container(
            row![
                text(message).width(Fill),
                button("Dismiss")
                    .on_press(Message::PageEdit(Event::DismissWarning))
                    .height(CONTROL_HEIGHT)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .style(container::warning)
        .padding(8)
        .into(),
    )
}
