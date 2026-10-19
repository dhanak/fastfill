use super::{Draft, Event, FastFill, Message, lifecycle, overlay};
use crate::appearance::CONTROL_HEIGHT;
use iced::widget::{button, checkbox, column, container, pick_list, row, text};
use iced::{Element, Fill};
use std::time::Instant;

pub(crate) fn view(state: &FastFill) -> Element<'_, Message> {
    let draft = state.camera.draft.as_ref().unwrap();
    let now = Instant::now();
    let crop = if draft.captured {
        draft.crop
    } else {
        draft.tracking.outline(now)
    };
    let progress = if draft.captured {
        if draft.crop.is_some() { 1.0 } else { 0.0 }
    } else {
        draft.tracking.progress(now)
    };
    let mut heading = column![
        row![
            control("Cancel", Event::Cancel, true),
            text("Camera").size(24).width(Fill),
            control(
                "Refresh cameras",
                Event::Refresh,
                !draft.captured && !draft.saving && !draft.sources.loading
            ),
            playback_control(draft),
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center),
        choices(draft),
        orientation_controls(draft),
        text(if draft.saving {
            "Preparing photo…"
        } else if draft.sources.loading {
            "Finding cameras and supported capture modes…"
        } else if draft.captured {
            "Use photo adds the page; canceling corrections keeps the photo."
        } else if let Some(reason) = draft.paused {
            reason
        } else if draft.running.is_some() && draft.last.is_none() {
            "Starting camera…"
        } else {
            draft.tracking.status(now)
        }),
    ]
    .spacing(12);
    if let Some(error) = &draft.error {
        let mut details = column![text(error)].spacing(8);
        if !draft.captured && !draft.saving {
            details = details.push(text(lifecycle::permission_help()).size(14));
        }
        heading = heading.push(
            container(details)
                .padding(12)
                .width(Fill)
                .style(container::danger),
        );
    }
    let preview: Element<'_, Message> = match &draft.last {
        Some(frame) => {
            overlay::preview(frame, crop, draft.orientation.mirrored)
        }
        None => container(text("Camera preview")).center(Fill).into(),
    };
    column![
        container(heading).padding(16),
        preview,
        container(
            row![
                capture_button(draft.can_capture(), progress),
                control(
                    "Use photo",
                    Event::UsePhoto,
                    draft.captured && !draft.saving
                ),
            ]
            .spacing(16)
            .align_y(iced::Alignment::Center)
        )
        .center_x(Fill)
        .padding(16),
    ]
    .height(Fill)
    .into()
}

fn control(
    label: &'static str,
    event: Event,
    enabled: bool,
) -> iced::widget::Button<'static, Message> {
    button(text(label))
        .height(CONTROL_HEIGHT)
        .on_press_maybe(enabled.then_some(Message::Camera(event)))
}

fn orientation_controls(draft: &Draft) -> Element<'static, Message> {
    let enabled = draft.sources.mode.is_some()
        && !draft.sources.loading
        && !draft.captured
        && !draft.saving;
    row![
        control("Rotate 90°", Event::Rotate, enabled),
        text(format!("{}°", draft.orientation.degrees())),
        checkbox(draft.orientation.mirrored)
            .label("Mirror preview")
            .on_toggle_maybe(
                enabled.then_some(|_| Message::Camera(Event::Mirror))
            ),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center)
    .into()
}

fn playback_control(draft: &Draft) -> iced::widget::Button<'static, Message> {
    let (label, event) = if draft.captured {
        ("Retake", Event::Start(draft.id))
    } else if draft.running.is_some() {
        ("Pause", Event::Pause)
    } else if draft.paused.is_some() {
        ("Resume preview", Event::Start(draft.id))
    } else {
        ("Start preview", Event::Start(draft.id))
    };
    control(
        label,
        event,
        !draft.saving && !draft.sources.loading && draft.sources.mode.is_some(),
    )
}

fn choices(draft: &Draft) -> Element<'_, Message> {
    let sources = &draft.sources;
    if draft.captured || draft.saving {
        let frame = draft.last.as_ref().unwrap();
        return text(format!(
            "Photo {}×{}; sensor mode {}",
            frame.pixels.width(),
            frame.pixels.height(),
            sources
                .mode
                .map_or_else(|| "unknown".into(), |m| m.to_string())
        ))
        .into();
    }
    row![
        column![
            text("Camera"),
            pick_list(
                sources.devices.as_slice(),
                sources.device.as_ref(),
                |device| Message::Camera(Event::Device(device)),
            )
            .placeholder("Select camera")
            .width(Fill)
            .padding(10),
        ]
        .spacing(4)
        .width(Fill),
        column![
            text("Preview and photo resolution"),
            pick_list(sources.modes.as_slice(), sources.mode, |mode| {
                Message::Camera(Event::Mode(mode))
            },)
            .placeholder("Select supported mode")
            .width(Fill)
            .padding(10),
        ]
        .spacing(4)
        .width(Fill),
    ]
    .spacing(12)
    .into()
}

fn capture_button(enabled: bool, progress: f64) -> Element<'static, Message> {
    iced::widget::stack![
        overlay::countdown(progress),
        container(
            button(text("Capture").size(14))
                .on_press_maybe(
                    enabled.then_some(Message::Camera(Event::Capture))
                )
                .width(80)
                .height(80)
                .style(|theme, status| {
                    let mut style = button::primary(theme, status);
                    style.border.radius = 40.into();
                    style
                })
        )
        .width(96)
        .height(96)
        .center_x(Fill)
        .center_y(Fill),
    ]
    .into()
}
