use super::*;
use crate::app::tests::render_themed_element;
use iced::advanced::{Layout, layout, widget};
use iced::{Element, Theme, mouse};

fn shown() -> FastFill {
    let mut state = ready();
    let draft = state.camera.draft.as_mut().unwrap();
    draft.sources.loading = false;
    draft.sources.mode = Some(Spec::default());
    draft.sources.modes = vec![Spec::default()];
    draft.sources.device = Some(Device {
        index: 0,
        name: "Preview camera".into(),
    });
    draft.sources.devices = vec![draft.sources.device.clone().unwrap()];
    draft.sources.devices.push(Device {
        index: 1,
        name: "Second camera".into(),
    });
    draft.last = Some(display_frame(
        Arc::new(RgbaImage::from_pixel(
            640,
            480,
            image::Rgba([40, 100, 220, 255]),
        )),
        Instant::now(),
    ));
    state
}

fn targets(layout: Layout<'_>) -> Vec<iced::Rectangle> {
    let children = layout.children().collect::<Vec<_>>();
    let bounds = layout.bounds();
    if (bounds.height - 40.0).abs() < 0.001
        && children.len() == 1
        && children[0].children().next().is_none()
    {
        let label = children[0].bounds();
        assert!((label.center().x - bounds.center().x).abs() < 0.01);
        vec![bounds]
    } else {
        children.into_iter().flat_map(targets).collect()
    }
}

#[test]
fn capture_is_no_larger_than_neighboring_camera_actions() {
    let state = shown();
    let mut element = view(&state);
    let mut tree = widget::Tree::new(element.as_widget());
    let size = iced::Size::new(1200.0, 800.0);
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer(),
        &layout::Limits::new(size, size),
    );
    let buttons = targets(Layout::new(&node));
    assert_eq!(buttons.len(), 7);
    assert!(
        buttons
            .iter()
            .all(|b| b.size() == iced::Size::new(40.0, 40.0))
    );
}

#[test]
fn camera_actions_have_centered_touch_targets_in_live_and_review_states() {
    let mut state = shown();
    let renderer = renderer();
    for captured in [false, true] {
        state.camera.draft.as_mut().unwrap().captured = captured;
        for size in [
            iced::Size::new(1200.0, 800.0),
            iced::Size::new(480.0, 400.0),
            iced::Size::new(1800.0, 800.0),
            iced::Size::new(800.0, 1000.0),
            iced::Size::new(320.0, 240.0),
        ] {
            let mut element: Element<'_, Message> = view(&state);
            let mut tree = widget::Tree::new(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let buttons = targets(Layout::new(&node));
            assert_eq!(buttons.len(), 7, "{size:?}: {buttons:?}");
            assert!(buttons.iter().all(|b| b.width >= 40.0));
            for (i, bounds) in buttons.iter().enumerate() {
                assert!(bounds.x >= 0.0 && bounds.y >= 0.0);
                assert!(bounds.x + bounds.width <= size.width);
                assert!(bounds.y + bounds.height <= size.height);
                for other in &buttons[i + 1..] {
                    assert!(bounds.intersection(other).is_none());
                }
            }
        }
    }
}

#[test]
fn camera_screen_renders_both_themes_and_dpi_scales() {
    let mut state = shown();
    for captured in [false, true] {
        state.camera.draft.as_mut().unwrap().captured = captured;
        for theme in [Theme::Light, Theme::Dark] {
            for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
                for (width, height) in
                    [(1200, 800), (480, 400), (1800, 800), (800, 1000)]
                {
                    let pixels = render_themed_element(
                        view(&state),
                        iced::Size::new(width, height),
                        scale,
                        dpi,
                        &theme,
                        mouse::Cursor::Unavailable,
                    );
                    if let Some(path) = std::env::var_os("FASTFILL_H2_SNAPSHOT")
                    {
                        let file = format!(
                            "camera-{captured}-{width}-{theme:?}-{scale}.png"
                        );
                        image::save_buffer(
                            std::path::PathBuf::from(path).join(file),
                            &pixels,
                            width * scale,
                            height * scale,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                    }
                    assert_eq!(
                        pixels.len(),
                        usize::try_from(width * height * scale * scale * 4)
                            .unwrap()
                    );
                    assert_preview_contained(
                        &state, pixels, width, height, scale,
                    );
                }
            }
        }
    }
}

fn assert_preview_contained(
    state: &FastFill,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    scale: u32,
) {
    let size = iced::Size::new(
        f32::from(u16::try_from(width).unwrap()),
        f32::from(u16::try_from(height).unwrap()),
    );
    let frame = state.camera.draft.as_ref().unwrap().last.as_ref().unwrap();
    #[allow(clippy::cast_precision_loss)]
    let source = iced::Size::new(
        frame.pixels.width() as f32,
        frame.pixels.height() as f32,
    );
    let fit = (size.width / source.width).min(size.height / source.height);
    let preview = iced::Rectangle {
        x: (size.width - source.width * fit) / 2.0,
        y: (size.height - source.height * fit) / 2.0,
        width: source.width * fit,
        height: source.height * fit,
    }
    .expand(1);
    let image =
        RgbaImage::from_raw(width * scale, height * scale, pixels).unwrap();
    let mut ink = 0;
    let mut notice = 0;
    for (x, y, pixel) in image.enumerate_pixels() {
        let point = iced::Point::new(
            f32::from(u16::try_from(x / scale).unwrap()),
            f32::from(u16::try_from(y / scale).unwrap()),
        );
        if pixel[0] < 70 && pixel[1] > 80 && pixel[1] < 160 && pixel[2] > 180 {
            assert!(preview.contains(point), "preview leaked into controls");
            ink += 1;
        }
        // Translucent dark lozenge mixes with the blue image, not the gutter.
        if pixel[0] < 40
            && pixel[1] < 90
            && pixel[2] > pixel[0] + 20
            && pixel[2] < 150
        {
            assert!(preview.contains(point), "notice left the camera image");
            notice += 1;
        }
    }
    assert!(ink > 100, "camera preview disappeared");
    assert!(notice > 100, "translucent notice disappeared");
}

#[test]
fn camera_actions_route_mouse_and_touch_in_live_review_and_saving_states() {
    use crate::app::tests::click_element;
    let mut state = shown();
    let renderer = renderer();
    for (captured, saving) in [(false, false), (true, false), (true, true)] {
        let draft = state.camera.draft.as_mut().unwrap();
        draft.captured = captured;
        draft.saving = saving;
        let id = draft.id;
        let enabled = if saving {
            [true, false, false, false, false, false, false]
        } else if captured {
            [true, false, false, true, true, false, false]
        } else {
            [true, true, true, true, true, true, true]
        };
        for touch in [false, true] {
            let mut element = view(&state);
            let mut tree = widget::Tree::new(element.as_widget());
            let size = iced::Size::new(1200.0, 1000.0);
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let buttons = targets(Layout::new(&node));
            let expected = [
                Event::Cancel,
                Event::Rotate,
                Event::Mirror,
                if captured {
                    Event::UsePhoto
                } else {
                    Event::Capture
                },
                Event::Start(id),
                Event::Settings(true),
                Event::Device(Device {
                    index: 1,
                    name: "Second camera".into(),
                }),
            ];
            for ((bounds, expected), enabled) in
                buttons.iter().zip(expected).zip(enabled)
            {
                let messages = click_element(
                    &mut element,
                    &mut tree,
                    &node,
                    &renderer,
                    bounds.center(),
                    touch,
                );
                if enabled {
                    assert_eq!(messages.len(), 1, "{expected:?}: {messages:?}");
                    let Message::Camera(actual) = &messages[0] else {
                        panic!("camera command");
                    };
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                } else {
                    assert!(messages.is_empty(), "disabled {expected:?}");
                }
            }
        }
    }
}

#[test]
fn settings_close_on_escape_without_discarding_the_camera_draft() {
    let mut state = shown();
    let before = state.pages.identity();
    let id = state.camera.draft.as_ref().unwrap().id;
    let _ = update(&mut state, &Message::Camera(Event::Settings(true)));
    assert!(state.camera.draft.as_ref().unwrap().settings_open);
    let _ = update(&mut state, &Message::CancelInteraction);
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(!draft.settings_open);
    assert_eq!(draft.id, id);
    assert!(draft.last.is_some());
    assert_eq!(state.pages.identity(), before);
    let _ = update(&mut state, &Message::Camera(Event::Capture));
    let _ = update(&mut state, &Message::Camera(Event::Settings(true)));
    let draft = state.camera.draft.as_ref().unwrap();
    assert!(draft.captured);
    assert!(!draft.settings_open);
    let _ = update(&mut state, &Message::CancelInteraction);
    assert!(!state.camera.is_open());
    assert_eq!(state.pages.identity(), before);
}

#[test]
fn settings_popover_routes_actions_and_blocks_the_underlying_dock() {
    use crate::app::tests::click_element;
    let mut state = shown();
    state.camera.draft.as_mut().unwrap().settings_open = true;
    let renderer = renderer();
    for touch in [false, true] {
        let mut element = view(&state);
        let mut tree = widget::Tree::new(element.as_widget());
        let size = iced::Size::new(1200.0, 800.0);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        let buttons = targets(Layout::new(&node));
        assert_eq!(buttons.len(), 9);
        for (index, expected) in [
            (0, Event::Settings(false)),
            (7, Event::Settings(false)),
            (8, Event::Refresh),
        ] {
            let messages = click_element(
                &mut element,
                &mut tree,
                &node,
                &renderer,
                buttons[index].center(),
                touch,
            );
            assert_eq!(messages.len(), 1, "{messages:?}");
            let Message::Camera(actual) = &messages[0] else {
                panic!("camera command");
            };
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }
    }
}

#[test]
fn auto_capture_does_not_transfer_a_held_press_to_use_photo_or_retake() {
    let renderer = renderer();
    let size = iced::Size::new(1200.0, 800.0);
    for index in [3, 4] {
        for touch in [false, true] {
            let mut state = shown();
            let mut element = view(&state);
            let mut tree = widget::Tree::new(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let point = targets(Layout::new(&node))[index].center();
            let mut messages = Vec::new();
            let event = if touch {
                iced::Event::Touch(iced::touch::Event::FingerPressed {
                    id: iced::touch::Finger(1),
                    position: point,
                })
            } else {
                iced::Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Left,
                ))
            };
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut iced::advanced::Shell::new(&mut messages),
                &node.bounds(),
            );
            assert!(messages.is_empty());
            drop(element);
            state.camera.draft.as_mut().unwrap().captured = true;
            let mut element = view(&state);
            tree.diff(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let event = if touch {
                iced::Event::Touch(iced::touch::Event::FingerLifted {
                    id: iced::touch::Finger(1),
                    position: point,
                })
            } else {
                iced::Event::Mouse(mouse::Event::ButtonReleased(
                    mouse::Button::Left,
                ))
            };
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut iced::advanced::Shell::new(&mut messages),
                &node.bounds(),
            );
            assert!(messages.is_empty(), "{messages:?}");
        }
    }
}
