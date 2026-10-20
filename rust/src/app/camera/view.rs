//! Fit the full camera image first; controls use spare space or float over it.

mod controls;
mod icons;

use super::{Draft, Event, FastFill, Message, overlay};
use iced::widget::{
    Space, container, mouse_area, opaque, pin, responsive, stack,
};
use iced::widget::{scrollable, text};
use iced::{Color, Element, Fill, Rectangle, Size};
use std::time::Instant;

const BUTTON: f32 = 40.0;
const GAP: f32 = 8.0;
const PADDING: f32 = 8.0;
const MARGIN: f32 = 12.0;
const DOCK_LENGTH: f32 = BUTTON * 6.0 + GAP * 5.0 + PADDING * 2.0;
const DOCK_WIDTH: f32 = BUTTON + PADDING * 2.0;

pub(crate) fn view(state: &FastFill) -> Element<'_, Message> {
    responsive(move |available| screen(state, available)).into()
}

fn screen(state: &FastFill, available: Size) -> Element<'_, Message> {
    let draft = state.camera.draft.as_ref().unwrap();
    let now = Instant::now();
    let placement = Placement::new(draft, available);
    let crop = if draft.captured {
        draft.crop
    } else {
        draft.tracking.outline(now)
    };
    let preview: Element<'_, Message> = match &draft.last {
        Some(frame) => {
            overlay::preview(frame, crop, draft.orientation.mirrored)
        }
        None => container(text("Camera preview")).center(Fill).into(),
    };
    let mut layers = stack![
        container(Space::new())
            .width(Fill)
            .height(Fill)
            .style(container::dark),
        at(preview, placement.image),
        at(
            notice(draft, now, placement.notice.size()),
            placement.notice,
        ),
        at(
            controls::dock(draft, placement.vertical, placement.compact, now),
            placement.dock
        ),
        at(controls::switch(draft), placement.switch),
    ]
    .width(Fill)
    .height(Fill)
    .clip(true);
    if draft.settings_open && !draft.captured {
        // The native stack/backdrop blocks the dock, not ongoing preview.
        layers = layers.push(opaque(
            mouse_area(Space::new().width(Fill).height(Fill))
                .on_press(Message::Camera(Event::Settings(false))),
        ));
        let width = (available.width - MARGIN * 2.0).clamp(1.0, 320.0);
        layers = layers.push(
            pin(opaque(controls::settings(
                draft,
                width,
                (available.height - MARGIN * 2.0).max(1.0),
            )))
            .x((available.width - width - MARGIN).max(0.0))
            .y(MARGIN),
        );
    }
    layers.into()
}

fn at<'a>(
    content: impl Into<Element<'a, Message>>,
    bounds: Rectangle,
) -> Element<'a, Message> {
    pin(container(content).width(bounds.width).height(bounds.height))
        .position(bounds.position())
        .into()
}

fn notice(
    draft: &Draft,
    now: Instant,
    available: Size,
) -> Element<'_, Message> {
    let label = if let Some(error) = &draft.error {
        error.as_str()
    } else if draft.saving {
        "Preparing photo…"
    } else if draft.sources.loading {
        "Finding cameras and supported resolutions…"
    } else if draft.captured {
        "Photo ready — use photo or retake."
    } else if let Some(reason) = draft.paused {
        reason
    } else if draft.running.is_some() && draft.last.is_none() {
        "Starting camera…"
    } else {
        draft.tracking.status(now)
    };
    let bubble = container(
        scrollable(text(label).size(14).align_x(iced::Center))
            .height(iced::Shrink),
    )
    .padding([8, 14])
    .max_width(available.width.min(520.0))
    .max_height(available.height)
    .style(move |_| iced::widget::container::Style {
        background: Some(Color::from_rgba8(20, 23, 28, 0.82).into()),
        text_color: Some(Color::WHITE),
        border: iced::Border {
            color: if draft.error.is_some() {
                Color::from_rgb8(240, 110, 100)
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: 20.0.into(),
        },
        ..Default::default()
    });
    container(bubble)
        .width(Fill)
        .height(Fill)
        .align_x(iced::Center)
        .align_y(iced::Bottom)
        .into()
}

#[derive(Debug)]
struct Placement {
    image: Rectangle,
    dock: Rectangle,
    notice: Rectangle,
    switch: Rectangle,
    vertical: bool,
    compact: bool,
}

impl Placement {
    #[allow(clippy::cast_precision_loss)]
    fn new(draft: &Draft, available: Size) -> Self {
        let source = draft.last.as_ref().map_or_else(
            || {
                draft.sources.mode.map_or(available, |mode| {
                    let size = Size::new(mode.width as f32, mode.height as f32);
                    if draft.orientation.degrees().is_multiple_of(180) {
                        size
                    } else {
                        Size::new(size.height, size.width)
                    }
                })
            },
            |frame| {
                Size::new(
                    frame.pixels.width() as f32,
                    frame.pixels.height() as f32,
                )
            },
        );
        let scale = (available.width / source.width.max(1.0))
            .min(available.height / source.height.max(1.0));
        let image = Rectangle::new(
            iced::Point::new(
                (available.width - source.width * scale) / 2.0,
                (available.height - source.height * scale) / 2.0,
            ),
            Size::new(source.width * scale, source.height * scale),
        );
        let right = available.width - (image.x + image.width);
        let bottom = available.height - (image.y + image.height);
        let vertical = right >= DOCK_WIDTH + MARGIN * 2.0
            && available.height >= DOCK_LENGTH + MARGIN * 2.0;
        let below = !vertical
            && bottom >= DOCK_WIDTH + MARGIN * 2.0
            && available.width >= DOCK_LENGTH + MARGIN * 2.0;
        let compact =
            !vertical && !below && image.width < DOCK_LENGTH + MARGIN * 2.0;
        let size = if vertical {
            Size::new(DOCK_WIDTH, DOCK_LENGTH)
        } else if compact {
            Size::new(
                BUTTON * 3.0 + GAP * 2.0 + PADDING * 2.0,
                BUTTON * 2.0 + GAP + PADDING * 2.0,
            )
        } else {
            Size::new(DOCK_LENGTH, DOCK_WIDTH)
        };
        let dock = Rectangle::new(
            iced::Point::new(
                if vertical {
                    image.x + image.width + MARGIN
                } else {
                    ((available.width - size.width) / 2.0).max(0.0)
                },
                if vertical {
                    (available.height - size.height) / 2.0
                } else if below {
                    image.y + image.height + MARGIN
                } else {
                    (image.y + image.height - MARGIN - size.height).max(0.0)
                },
            ),
            size,
        );
        let notice_bottom = if vertical || below {
            image.y + image.height - MARGIN
        } else {
            dock.y - GAP
        };
        Self {
            image,
            dock,
            notice: Rectangle {
                x: image.x + MARGIN,
                y: image.y + MARGIN,
                width: (image.width - MARGIN * 2.0).max(1.0),
                height: (notice_bottom - image.y - MARGIN).max(1.0),
            },
            switch: Rectangle {
                x: (image.x + image.width - MARGIN - BUTTON).max(image.x),
                y: image.y + MARGIN,
                width: BUTTON,
                height: BUTTON,
            },
            vertical,
            compact,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::camera::tests::ready;
    use fastfill_rust::camera::Spec;

    #[test]
    fn dock_uses_existing_margins_without_shrinking_the_image() {
        let mut state = ready();
        let draft = state.camera.draft.as_mut().unwrap();
        draft.sources.mode = Some(Spec {
            width: 1600,
            height: 900,
            ..Spec::default()
        });
        for (size, vertical, below) in [
            (Size::new(1600.0, 900.0), false, false),
            (Size::new(1800.0, 700.0), true, false),
            (Size::new(1000.0, 1000.0), false, true),
            (Size::new(480.0, 270.0), false, false),
            (Size::new(320.0, 180.0), false, false),
        ] {
            let p = Placement::new(draft, size);
            assert_eq!(p.vertical, vertical);
            assert_eq!(p.dock.y >= p.image.y + p.image.height, below);
            assert!(
                (p.image.width / p.image.height - 16.0 / 9.0).abs() < 0.001
            );
            assert!(
                (p.image.width - size.width).abs() < 0.001
                    || (p.image.height - size.height).abs() < 0.001
            );
            assert!(p.image.contains(p.notice.position()));
            assert!(p.image.contains(iced::Point::new(
                p.notice.x + p.notice.width,
                p.notice.y + p.notice.height
            )));
            assert!(p.dock.x >= 0.0 && p.dock.y >= 0.0);
            assert!(p.dock.x + p.dock.width <= size.width);
            assert!(p.dock.y + p.dock.height <= size.height);
            if !vertical && !below {
                assert!(p.notice.y + p.notice.height < p.dock.y);
            }
        }
        draft.orientation.rotate();
        let rotated = Placement::new(draft, Size::new(1200.0, 800.0));
        assert!(rotated.vertical);
        assert!(
            (rotated.image.width / rotated.image.height - 9.0 / 16.0).abs()
                < 0.001
        );
    }
}
