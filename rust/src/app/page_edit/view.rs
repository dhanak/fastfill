use super::canvas::CropCanvas;
use super::{Draft, Event, FastFill, Message};
use crate::appearance::{self, CONTROL_HEIGHT, action_button};
use fastfill_rust::geometry::Size;
use fastfill_rust::project::PageFilterMode;
use iced::widget::{
    Space, canvas, column, container, image, responsive, row, rule, scrollable,
    slider, text,
};
use iced::{Element, Fill};

pub(in crate::app) fn view(state: &FastFill) -> Element<'_, Message> {
    responsive(move |available| screen(state, available)).into()
}

fn screen(state: &FastFill, available: iced::Size) -> Element<'_, Message> {
    let draft = state.page_editor.draft.as_ref().expect("active editor");
    let framing = state.page_editor.is_framing();
    let ready = !framing
        && draft.ready.as_ref().is_some_and(|p| {
            p.settings == draft.settings && draft.error.is_none()
        });
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
    let can_frame =
        state.page_editor.running.is_none() && draft.original.is_some();
    let wide = available.width >= 760.0;
    let sidebar = container(
        scrollable(
            column![
                text("Corrections").size(18),
                options(draft, can_frame),
                rule::horizontal(1),
                text("Corrected preview").size(14),
                container(preview).style(container::dark),
            ]
            .spacing(12)
            .padding(iced::Padding {
                right: 12.0,
                ..iced::Padding::ZERO
            }),
        )
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(2),
        ))
        .height(Fill),
    )
    .padding(12)
    .width(if wide { 312.into() } else { Fill })
    .height(if wide {
        Fill
    } else {
        (available.height * 0.35).clamp(100.0, 240.0).into()
    })
    .style(container::bordered_box);
    let source = column![
        text("Original — drag corners; hold Ctrl to snap").size(14),
        original(state),
    ]
    .spacing(8)
    .width(Fill)
    .height(Fill);
    let body: Element<'_, Message> = if wide {
        row![source, sidebar].spacing(12).height(Fill).into()
    } else {
        column![source, sidebar].spacing(12).height(Fill).into()
    };
    column![
        row![
            text(format!("Edit page {}", state.pages.active_index() + 1))
                .size(24)
                .width(Fill),
            action_button("Cancel", Some(Message::PageEdit(Event::Cancel)))
                .width(100),
            action_button(
                "Accept",
                ready.then_some(Message::PageEdit(Event::Accept))
            )
            .width(100)
            .style(appearance::primary_action),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
        status(draft, ready, state.modifiers.square, framing),
        body,
    ]
    .spacing(12)
    .padding(16)
    .width(Fill)
    .height(Fill)
    .into()
}

fn original(state: &FastFill) -> Element<'_, Message> {
    let draft = state.page_editor.draft.as_ref().unwrap();
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
        corners: draft
            .original
            .as_ref()
            .and_then(|original| original.corners.as_ref().ok()),
        snap: state.modifiers.square,
        epoch: state.input_epoch,
        background: false,
    };
    iced::widget::stack![
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
    .clip(true)
    .into()
}

fn status(
    draft: &Draft,
    ready: bool,
    snap: bool,
    framing: bool,
) -> Element<'_, Message> {
    if let Some(error) = &draft.error {
        container(text(error))
            .padding(8)
            .style(container::danger)
            .into()
    } else if !framing && let Some(notice) = &draft.notice {
        container(text(notice))
            .padding(8)
            .style(container::warning)
            .into()
    } else {
        text(if framing {
            "Detecting document frame… Manual changes take priority."
        } else if ready {
            if draft
                .original
                .as_ref()
                .is_some_and(|original| original.corners.is_err())
            {
                "Corner hints unavailable; manual dragging still works."
            } else if snap {
                "Ctrl: snap to nearby corners. Release Ctrl for free dragging."
            } else {
                "Drag freely; hold Ctrl to snap. Accept applies all changes."
            }
        } else {
            "Updating corrected preview…"
        })
        .size(14)
        .into()
    }
}

fn options(draft: &Draft, can_frame: bool) -> Element<'static, Message> {
    let choose = |label, mode| {
        action_button(label, Some(Message::PageEdit(Event::Mode(mode))))
            .width(Fill)
            .style(if draft.settings.filter.mode == mode {
                appearance::primary_action
            } else {
                appearance::secondary_action
            })
    };
    column![
        text("Frame").size(14),
        row![
            action_button(
                "Auto frame",
                can_frame.then_some(Message::PageEdit(Event::AutoFrame))
            )
            .width(Fill),
            action_button(
                "Full frame",
                Some(Message::PageEdit(Event::FullFrame))
            )
            .width(Fill),
        ]
        .spacing(8),
        row![
            action_button(
                "Rotate left",
                Some(Message::PageEdit(Event::Rotate(-1)))
            )
            .width(Fill),
            action_button(
                "Rotate right",
                Some(Message::PageEdit(Event::Rotate(1)))
            )
            .width(Fill),
        ]
        .spacing(8),
        rule::horizontal(1),
        text("Color mode").size(14),
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
        rule::horizontal(1),
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
        row![
            text(label).size(14).width(Fill),
            text(format!("{value:+}")).size(14)
        ],
        row![
            action_button(
                "−",
                (value > -100).then(|| Message::PageEdit(event(value - 1)))
            )
            .width(CONTROL_HEIGHT),
            slider(-100..=100, value, move |value| Message::PageEdit(event(
                value
            ))),
            action_button(
                "+",
                (value < 100).then(|| Message::PageEdit(event(value + 1)))
            )
            .width(CONTROL_HEIGHT),
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
                action_button(
                    "Dismiss",
                    Some(Message::PageEdit(Event::DismissWarning))
                )
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .style(container::warning)
        .padding(8)
        .into(),
    )
}
