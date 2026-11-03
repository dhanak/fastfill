use super::*;
use crate::app::tests::render_themed_element;
use iced::advanced::renderer::Headless;
use iced::advanced::{Layout, layout, widget};
use iced::{Element, Renderer, Theme, mouse};

fn targets(layout: Layout<'_>) -> Vec<iced::Rectangle> {
    let children = layout.children().collect::<Vec<_>>();
    let bounds = layout.bounds();
    if (bounds.height - 40.0).abs() < f32::EPSILON
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
fn correction_controls_stay_centered_and_leave_room_for_the_source_image() {
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    for size in [
        iced::Size::new(1200.0, 800.0),
        iced::Size::new(600.0, 500.0),
        iced::Size::new(400.0, 500.0),
        iced::Size::new(1200.0, 360.0),
    ] {
        let mut element: Element<'_, Message> = view(&state);
        let mut tree = widget::Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        let buttons = targets(Layout::new(&node));
        assert_eq!(buttons.len(), 19);
        assert!(buttons.iter().all(|b| b.width >= 40.0), "{buttons:?}");
        for bounds in &buttons[..2] {
            assert!(bounds.x >= 16.0 && bounds.y >= 16.0);
            assert!(bounds.x + bounds.width <= size.width - 16.0);
            assert!(bounds.y + bounds.height <= 60.0);
        }
        assert!(buttons[2..7].iter().all(|bounds| {
            bounds.x >= 16.0 && bounds.x + bounds.width <= size.width - 16.0
        }));
        if size.width < 760.0 {
            fn full_width_preview(node: &layout::Node, width: f32) -> bool {
                (node.children().is_empty()
                    && node.size().height > 80.0
                    && node.size().width >= width - 50.0)
                    || node
                        .children()
                        .iter()
                        .any(|n| full_width_preview(n, width))
            }
            assert!(
                full_width_preview(&node, size.width),
                "source squeezed by sidebar"
            );
        }
    }
}

#[test]
fn correction_screen_renders_both_themes_and_dpi_scales() {
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    send(&mut state, Event::Brightness(20));
    settle(&mut state);
    for mode in [viewer::Mode::Frame, viewer::Mode::Preview] {
        send(&mut state, Event::View(viewer::Event::Mode(mode)));
        for theme in [Theme::Light, Theme::Dark] {
            for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
                for (width, height) in [(1200, 800), (600, 500), (400, 500)] {
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
                        image::save_buffer(
                        std::path::PathBuf::from(path).join(format!(
                            "correction-{mode:?}-{width}-{theme:?}-{scale}.png"
                        )),
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
                    let pixel = |x, y| {
                        let offset = usize::try_from(
                            (y * scale * width * scale + x * scale) * 4,
                        )
                        .unwrap();
                        &pixels[offset..offset + 4]
                    };
                    assert_eq!(
                        pixel(width - 50, 48),
                        theme
                            .extended_palette()
                            .primary
                            .base
                            .color
                            .into_rgba8()
                    );
                    assert_eq!(
                        pixel(width - 158, 48),
                        theme
                            .extended_palette()
                            .background
                            .base
                            .color
                            .into_rgba8()
                    );
                }
            }
        }
    }
}

#[test]
fn correction_actions_route_mouse_and_touch_and_respect_preview_readiness() {
    use crate::app::tests::click_element;
    let mut state = state();
    send(&mut state, Event::Begin);
    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    for ready in [false, true] {
        if ready {
            settle(&mut state);
        }
        for touch in [false, true] {
            let mut element = view(&state);
            let mut tree = widget::Tree::new(element.as_widget());
            let size = iced::Size::new(1200.0, 1200.0);
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let buttons = targets(Layout::new(&node));
            let expected = [
                Event::Cancel,
                Event::Accept,
                Event::View(viewer::Event::Mode(viewer::Mode::Frame)),
                Event::View(viewer::Event::Mode(viewer::Mode::Preview)),
                Event::View(viewer::Event::Compare(true)),
                Event::View(viewer::Event::Fit),
                Event::View(viewer::Event::ActualSize),
                Event::AutoFrame,
                Event::FullFrame,
                Event::Rotate(-1),
                Event::Rotate(1),
                Event::Mode(PageFilterMode::Original),
                Event::Mode(PageFilterMode::EnhancedColor),
                Event::Mode(PageFilterMode::Grayscale),
                Event::Mode(PageFilterMode::BlackAndWhite),
                Event::Brightness(-1),
                Event::Brightness(1),
                Event::Contrast(-1),
                Event::Contrast(1),
            ];
            for (i, (bounds, expected)) in
                buttons.iter().zip(expected).enumerate()
            {
                let messages = click_element(
                    &mut element,
                    &mut tree,
                    &node,
                    &renderer,
                    bounds.center(),
                    touch,
                );
                if (!ready && [1, 4, 7].contains(&i)) || (ready && i == 4) {
                    assert!(messages.is_empty(), "disabled {expected:?}");
                } else {
                    assert_eq!(
                        messages.len(),
                        1,
                        "ready={ready} touch={touch} {expected:?}: {messages:?}"
                    );
                    let Message::PageEdit(actual) = &messages[0] else {
                        panic!("page correction command");
                    };
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                }
            }
        }
    }
}

#[test]
fn compare_button_is_available_on_preview_for_mouse_and_touch() {
    use crate::app::tests::click_element;
    let mut state = state();
    send(&mut state, Event::Begin);
    settle(&mut state);
    send(
        &mut state,
        Event::View(viewer::Event::Mode(viewer::Mode::Preview)),
    );
    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    for touch in [false, true] {
        let mut element = view(&state);
        let mut tree = widget::Tree::new(element.as_widget());
        let size = iced::Size::new(1200.0, 800.0);
        let node = element.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        let compare = targets(Layout::new(&node))[4];
        let messages = click_element(
            &mut element,
            &mut tree,
            &node,
            &renderer,
            compare.center(),
            touch,
        );
        assert!(matches!(
            messages.as_slice(),
            [
                Message::PageEdit(Event::View(viewer::Event::Compare(true))),
                Message::PageEdit(Event::View(viewer::Event::Compare(false)))
            ]
        ));
    }
}
