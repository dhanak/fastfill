use super::canvas::CropCanvas;
use super::viewer::{self, Mode};
use super::{Draft, Event, FastFill, Message};
use crate::appearance::{self, CONTROL_HEIGHT, action_button};
use fastfill_rust::correction::corrected_dimensions;
use fastfill_rust::geometry::Size;
use fastfill_rust::project::PageFilterMode;
use iced::widget::{
    Space, button, canvas, column, container, mouse_area, responsive, row,
    rule, scrollable, slider, text,
};
use iced::{Element, Fill, Shrink, mouse};

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
    let can_frame =
        state.page_editor.running.is_none() && draft.original.is_some();
    let wide = available.width >= 760.0;
    let sidebar = container(
        scrollable(
            column![text("Corrections").size(18), options(draft, can_frame)]
                .spacing(12),
        )
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new()
                .width(6)
                .scroller_width(6)
                .margin(2)
                .spacing(2),
        ))
        .height(Fill),
    )
    .padding(12)
    .width(if wide { 312.into() } else { Fill })
    .height(if wide {
        Fill
    } else {
        (available.height * 0.30).clamp(110.0, 220.0).into()
    })
    .style(container::bordered_box);
    let tool_width = if wide {
        available.width - 356.0
    } else {
        available.width - 32.0
    };
    let source =
        column![view_tools(draft, ready, tool_width), image_canvas(state)]
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

fn view_tools(
    draft: &Draft,
    ready: bool,
    width: f32,
) -> Element<'static, Message> {
    let selected = |mode| {
        if draft.viewer.mode == mode {
            appearance::primary_action
        } else {
            appearance::secondary_action
        }
    };
    let compare = draft.viewer.comparing;
    let can_compare = draft.viewer.mode == Mode::Preview && ready;
    let compare_style = move |theme: &iced::Theme| {
        let button_style = if compare {
            appearance::primary_action(theme, button::Status::Pressed)
        } else {
            appearance::secondary_action(
                theme,
                if can_compare {
                    button::Status::Active
                } else {
                    button::Status::Disabled
                },
            )
        };
        container::Style {
            text_color: Some(button_style.text_color),
            background: button_style.background,
            border: button_style.border,
            shadow: button_style.shadow,
            ..container::Style::default()
        }
    };
    let compare_area = mouse_area(
        container(text("Compare").size(14))
            .width(Shrink)
            .height(CONTROL_HEIGHT)
            .padding([0, 12])
            .align_y(iced::Alignment::Center)
            .style(compare_style),
    );
    let compare_button: Element<'static, Message> = if can_compare {
        compare_area
            .on_press(Message::PageEdit(Event::View(viewer::Event::Compare(
                true,
            ))))
            .on_release(Message::PageEdit(Event::View(viewer::Event::Compare(
                false,
            ))))
            .interaction(mouse::Interaction::Pointer)
            .into()
    } else {
        compare_area.into()
    };
    let modes = row![
        action_button(
            "Frame",
            Some(Message::PageEdit(Event::View(viewer::Event::Mode(
                Mode::Frame
            ))))
        )
        .style(selected(Mode::Frame)),
        action_button(
            "Preview",
            Some(Message::PageEdit(Event::View(viewer::Event::Mode(
                Mode::Preview
            ))))
        )
        .style(selected(Mode::Preview)),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    let actions = row![
        compare_button,
        action_button(
            "Fit",
            Some(Message::PageEdit(Event::View(viewer::Event::Fit)))
        )
        .width(48),
        action_button(
            "1:1",
            Some(Message::PageEdit(Event::View(viewer::Event::ActualSize)))
        )
        .width(48),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if width < 380.0 {
        column![modes, actions].spacing(6).into()
    } else {
        row![modes, Space::new().width(Fill), actions]
            .align_y(iced::Alignment::Center)
            .into()
    }
}

fn image_canvas(state: &FastFill) -> Element<'_, Message> {
    let draft = state.page_editor.draft.as_ref().unwrap();
    let page = state.pages.current();
    #[allow(clippy::cast_precision_loss)]
    let source_size = Size::new(
        page.metadata.source_pixel_width as f32,
        page.metadata.source_pixel_height as f32,
    );
    let image = match draft.viewer.mode {
        Mode::Frame => draft.original.as_ref().map(|original| &original.handle),
        Mode::Preview => draft.ready.as_ref().map(|preview| {
            if draft.viewer.comparing {
                &preview.comparison
            } else {
                &preview.handle
            }
        }),
    };
    let display_size = if draft.viewer.mode == Mode::Preview {
        draft.ready.as_ref().map_or(source_size, |preview| {
            let (width, height) = corrected_dimensions(
                preview.original.pixels.width(),
                preview.original.pixels.height(),
                preview.settings,
            )
            .expect("ready preview has valid dimensions");
            #[allow(clippy::cast_precision_loss)]
            Size::new(width as f32, height as f32)
        })
    } else {
        source_size
    };
    let source = CropCanvas {
        image,
        source_size: display_size,
        crop: draft.settings.crop,
        corners: draft
            .original
            .as_ref()
            .and_then(|original| original.corners.as_ref().ok()),
        snap: state.modifiers.square,
        epoch: state.input_epoch.wrapping_add(draft.viewer.epoch),
        background: false,
        mode: draft.viewer.mode,
        view: draft.viewer.current(),
        rotation: draft.settings.rotation_quarter_turns,
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
        } else if draft.viewer.mode == Mode::Preview && draft.viewer.comparing {
            "Before adjustments — same crop and rotation. \
             Release Compare to return to the corrected preview."
        } else if draft.viewer.mode == Mode::Preview && ready {
            "Corrected preview. Scroll to zoom; drag to pan."
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
