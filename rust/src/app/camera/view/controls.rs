//! Camera-only controls; no device IO or capture state transitions.

use super::icons::{self, Icon};
use super::{BUTTON, GAP, Message, PADDING, overlay};
use crate::app::camera::{Draft, Event, lifecycle};
use crate::appearance::{self, action_button};
use fastfill_rust::camera::Spec;
use iced::widget::{Column, Row, button, column, container, pick_list};
use iced::widget::{row, scrollable, stack, text, tooltip};
use iced::{Element, Fill};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Resolution(Spec);

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}×{}", self.0.width, self.0.height)
    }
}

pub(super) fn dock(
    draft: &Draft,
    vertical: bool,
    compact: bool,
    now: Instant,
) -> Element<'static, Message> {
    let editable = !draft.captured && !draft.saving;
    let configured =
        editable && !draft.sources.loading && draft.sources.mode.is_some();
    let (play_icon, play_label, play_event) = if draft.captured {
        (Icon::Retake, "Retake", Event::Start(draft.id))
    } else if draft.running.is_some() {
        (Icon::Pause, "Pause preview", Event::Pause)
    } else {
        (Icon::Play, "Start / resume preview", Event::Start(draft.id))
    };
    let capture = if draft.captured {
        icon(
            Icon::UsePhoto,
            "Use photo",
            (!draft.saving).then_some(Event::UsePhoto),
            true,
            true,
        )
    } else {
        stack![
            icon(
                Icon::Capture,
                "Capture",
                draft.can_capture().then_some(Event::Capture),
                true,
                true
            ),
            overlay::countdown(draft.tracking.progress(now)),
        ]
        .into()
    };
    let playback = icon(
        play_icon,
        play_label,
        (!draft.saving
            && !draft.sources.loading
            && draft.sources.mode.is_some())
        .then_some(play_event),
        false,
        false,
    );
    // A distinct review subtree drops a held Pause press on auto capture.
    let playback = if draft.captured {
        stack![playback].into()
    } else {
        playback
    };
    let actions = vec![
        icon(Icon::Cancel, "Cancel", Some(Event::Cancel), false, false),
        icon(
            Icon::Rotate,
            format!("Rotate 90° ({}°)", draft.orientation.degrees()),
            configured.then_some(Event::Rotate),
            false,
            false,
        ),
        icon(
            Icon::Mirror,
            "Mirror preview",
            configured.then_some(Event::Mirror),
            draft.orientation.mirrored,
            false,
        ),
        capture,
        playback,
        icon(
            Icon::Settings,
            "Camera settings",
            editable.then_some(Event::Settings(true)),
            false,
            false,
        ),
    ];
    let content: Element<'_, Message> = if vertical {
        Column::with_children(actions).spacing(GAP).into()
    } else if compact {
        let mut actions = actions.into_iter();
        column![
            Row::with_children(actions.by_ref().take(3)).spacing(GAP),
            Row::with_children(actions).spacing(GAP),
        ]
        .spacing(GAP)
        .into()
    } else {
        Row::with_children(actions).spacing(GAP).into()
    };
    container(content).padding(PADDING).style(surface).into()
}

pub(super) fn switch(draft: &Draft) -> Element<'static, Message> {
    let sources = &draft.sources;
    let index = sources
        .devices
        .iter()
        .position(|d| Some(d) == sources.device.as_ref())
        .unwrap_or(0);
    let next = sources
        .devices
        .get((index + 1) % sources.devices.len().max(1));
    icon(
        Icon::Switch,
        "Switch camera",
        (!draft.captured
            && !draft.saving
            && !sources.loading
            && sources.devices.len() > 1)
            .then(|| Event::Device(next.unwrap().clone())),
        false,
        false,
    )
}

pub(super) fn settings(
    draft: &Draft,
    width: f32,
    height: f32,
) -> Element<'_, Message> {
    let sources = &draft.sources;
    container(
        scrollable(
            column![
                row![
                    text("Camera settings").size(18).width(Fill),
                    icon(
                        Icon::Cancel,
                        "Close settings",
                        Some(Event::Settings(false)),
                        false,
                        false
                    ),
                ]
                .align_y(iced::Center),
                text("Camera").size(14),
                pick_list(
                    sources.devices.as_slice(),
                    sources.device.as_ref(),
                    |device| Message::Camera(Event::Device(device))
                )
                .placeholder("Select camera")
                .text_size(14)
                .width(Fill)
                .padding(10),
                text("Preview and photo resolution").size(14),
                pick_list(
                    sources
                        .modes
                        .iter()
                        .copied()
                        .map(Resolution)
                        .collect::<Vec<_>>(),
                    sources.mode.map(Resolution),
                    |mode| Message::Camera(Event::Mode(mode.0))
                )
                .placeholder("Select resolution")
                .text_size(14)
                .width(Fill)
                .padding(10),
                action_button(
                    "Refresh cameras",
                    (!sources.loading && !draft.saving && !draft.captured)
                        .then_some(Message::Camera(Event::Refresh))
                )
                .width(Fill),
                text(lifecycle::permission_help()).size(12),
            ]
            .spacing(8)
            .padding(iced::Padding {
                right: 10.0,
                ..iced::Padding::ZERO
            }),
        )
        .height(iced::Shrink),
    )
    .padding(12)
    .width(width)
    .max_height(height)
    .style(appearance::confirmation_card)
    .into()
}

fn icon(
    glyph: Icon,
    label: impl Into<String>,
    event: Option<Event>,
    selected: bool,
    round: bool,
) -> Element<'static, Message> {
    let enabled = event.is_some();
    let control = button(icons::view(glyph, selected, enabled))
        .width(BUTTON)
        .height(BUTTON)
        .padding(8)
        .on_press_maybe(event.map(Message::Camera))
        .style(move |theme, status| {
            let mut style = if selected {
                appearance::primary_action(theme, status)
            } else {
                appearance::secondary_action(theme, status)
            };
            if round {
                style.border.radius = (BUTTON / 2.0).into();
            }
            style
        });
    tooltip(
        control,
        container(text(label.into()).size(14))
            .padding([6, 10])
            .style(container::rounded_box),
        tooltip::Position::FollowCursor,
    )
    .delay(Duration::from_millis(500))
    .into()
}

fn surface(theme: &iced::Theme) -> iced::widget::container::Style {
    let palette = theme.extended_palette();
    iced::widget::container::Style {
        background: Some(
            palette.background.base.color.scale_alpha(0.88).into(),
        ),
        border: iced::Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_label_preserves_capture_spec_but_hides_driver_details() {
        let mode = Spec {
            width: 1920,
            height: 1080,
            fps: 60,
            fourcc: *b"NV12",
        };
        let choice = Resolution(mode);
        assert_eq!(choice.to_string(), "1920×1080");
        assert_eq!(choice.0, mode);
        assert!(mode.to_string().contains("60 FPS · NV12"));
    }
}
