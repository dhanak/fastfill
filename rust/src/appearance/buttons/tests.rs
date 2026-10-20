use super::*;
use iced::advanced::renderer::Headless;
use iced::advanced::{Layout, Shell, layout, widget};
use iced::{
    Background, Element, Event, Fill, Point, Renderer, Size, mouse, touch,
};

fn renderer() -> Renderer {
    iced::futures::executor::block_on(<Renderer as Headless>::new(
        iced::Font::with_name("Fira Sans"),
        iced::Pixels(16.0),
        Some("tiny-skia"),
    ))
    .unwrap()
}

#[test]
fn action_styles_share_corners_and_dim_disabled_states_in_both_themes() {
    for theme in [Theme::Light, Theme::Dark] {
        for style in [secondary_action, primary_action, destructive_action] {
            let active = style(&theme, Status::Active);
            for status in [
                Status::Active,
                Status::Hovered,
                Status::Pressed,
                Status::Disabled,
            ] {
                let current = style(&theme, status);
                assert_eq!(current.border.radius, 6.0.into());
                assert_eq!(current.shadow, iced::Shadow::default());
            }
            assert!(
                style(&theme, Status::Disabled).text_color.a
                    < active.text_color.a
            );
        }
        let neutral = secondary_action(&theme, Status::Active);
        assert_eq!(neutral.border.width, 1.0);
        assert_eq!(
            neutral.text_color,
            theme.extended_palette().background.base.text
        );
        assert_ne!(
            neutral.background,
            primary_action(&theme, Status::Active).background
        );
        let danger = destructive_action(&theme, Status::Active);
        assert_eq!(danger.background, neutral.background);
        assert_ne!(danger.text_color, neutral.text_color);
        assert!(
            danger.text_color.r > danger.text_color.g * 1.3,
            "{theme:?} {danger:?}",
        );
        assert!(danger.text_color.r > danger.text_color.b * 1.3);
        assert_ne!(
            danger.background,
            destructive_action(&theme, Status::Hovered).background
        );
    }
}

#[test]
fn action_labels_keep_accessible_contrast_in_both_themes() {
    for theme in [Theme::Light, Theme::Dark] {
        for style in [secondary_action, primary_action, destructive_action] {
            for status in [Status::Active, Status::Hovered, Status::Pressed] {
                let style = style(&theme, status);
                let Some(Background::Color(background)) = style.background
                else {
                    panic!("solid action surface");
                };
                assert!(
                    background.relative_contrast(style.text_color) >= 4.5,
                    "{theme:?} {status:?} {style:?}"
                );
            }
        }
    }
}

#[test]
fn actions_center_labels_keep_forty_pixel_targets_and_route_mouse_and_touch() {
    let renderer = renderer();
    for (width, label) in
        [(iced::Shrink, "Save"), (40.into(), "Fit"), (Fill, "Save")]
    {
        for enabled in [false, true] {
            for use_touch in [false, true] {
                let mut element: Element<'_, u8> =
                    action_button(label, enabled.then_some(7))
                        .width(width)
                        .into();
                let mut tree = widget::Tree::new(element.as_widget());
                let node = element.as_widget_mut().layout(
                    &mut tree,
                    &renderer,
                    &layout::Limits::new(Size::ZERO, Size::new(180.0, 80.0)),
                );
                assert_eq!(node.size().height, 40.0);
                assert!(node.size().width >= 40.0);
                if width == iced::Shrink {
                    assert!(node.size().width < 90.0);
                }
                let label = node.children()[0].bounds();
                assert!(
                    (label.center().x - node.bounds().center().x).abs() < 0.01
                );
                assert!(
                    (label.center().y - node.bounds().center().y).abs() < 0.01
                );
                let position = Point::new(3.0, 20.0);
                let events = if use_touch {
                    [
                        Event::Touch(touch::Event::FingerPressed {
                            id: touch::Finger(1),
                            position,
                        }),
                        Event::Touch(touch::Event::FingerLifted {
                            id: touch::Finger(1),
                            position,
                        }),
                    ]
                } else {
                    [
                        Event::Mouse(mouse::Event::ButtonPressed(
                            mouse::Button::Left,
                        )),
                        Event::Mouse(mouse::Event::ButtonReleased(
                            mouse::Button::Left,
                        )),
                    ]
                };
                let mut messages = Vec::new();
                for event in events {
                    element.as_widget_mut().update(
                        &mut tree,
                        &event,
                        Layout::new(&node),
                        mouse::Cursor::Available(position),
                        &renderer,
                        &mut iced::advanced::clipboard::Null,
                        &mut Shell::new(&mut messages),
                        &node.bounds(),
                    );
                }
                assert_eq!(messages, if enabled { vec![7] } else { vec![] });
            }
        }
    }
}
