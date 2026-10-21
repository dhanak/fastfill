use super::*;
use crate::app::tests::render_themed_element;
use crate::appearance::action_button;
use iced::advanced::renderer::Headless;
use iced::advanced::{Layout, Shell, layout, widget};
use iced::{Event, Point, Renderer, Theme, mouse, touch};

fn renderer() -> Renderer {
    iced::futures::executor::block_on(<Renderer as Headless>::new(
        iced::Font::with_name("Fira Sans"),
        iced::Pixels(16.0),
        Some("tiny-skia"),
    ))
    .unwrap()
}

fn lay_out(
    element: &mut Element<'_, Message>,
    renderer: &Renderer,
) -> (widget::Tree, layout::Node) {
    let mut tree = widget::Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        renderer,
        &layout::Limits::new(iced::Size::ZERO, iced::Size::new(1_000.0, 56.0)),
    );
    (tree, node)
}

fn click(mut element: Element<'_, Message>, touch: bool) -> Vec<Message> {
    let renderer = renderer();
    let (mut tree, node) = lay_out(&mut element, &renderer);
    // Press padding, not the glyph itself: the entire target must work.
    let position = Point::new(3.0, 20.0);
    let events = if touch {
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
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
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
    messages
}

fn glyphs() -> Vec<Glyph> {
    Tool::ALL
        .into_iter()
        .map(Glyph::Tool)
        .chain([Glyph::Fill, Glyph::SmartSnap])
        .collect()
}

fn icon_grid() -> Element<'static, Message> {
    let mut grid = iced::widget::Column::new();
    for (selected, enabled) in
        [(false, true), (true, true), (false, false), (true, false)]
    {
        grid =
            grid.push(Row::with_children(glyphs().into_iter().map(|glyph| {
                icon_button(
                    glyph,
                    "Icon",
                    selected,
                    enabled.then_some(Message::ModalBackdrop),
                )
            })));
    }
    grid.into()
}

#[test]
fn zoom_icons_put_distinct_plus_and_minus_inside_the_lens() {
    for theme in [Theme::Light, Theme::Dark] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let draw = |glyph| {
                render_themed_element(
                    icon_button(glyph, "Zoom", false, Some(Message::Fit)),
                    iced::Size::new(40, 40),
                    scale,
                    dpi,
                    &theme,
                    mouse::Cursor::Unavailable,
                )
            };
            let minus = draw(Glyph::ZoomOut);
            let plus = draw(Glyph::ZoomIn);
            let scale = usize::try_from(scale).unwrap();
            assert_ne!(minus, plus);
            assert!(
                delta(
                    pixel(&minus, 40 * scale, 18 * scale, 16 * scale),
                    pixel(&plus, 40 * scale, 18 * scale, 16 * scale),
                ) > 30,
                "zoom plus must be inside the lens at {dpi} DPI"
            );
        }
    }
}

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    let offset = (y * width + x) * 4;
    &pixels[offset..offset + 4]
}

fn delta(a: &[u8], b: &[u8]) -> u16 {
    a.iter()
        .zip(b)
        .take(3)
        .map(|(a, b)| u16::from(a.abs_diff(*b)))
        .sum()
}

#[test]
fn tool_buttons_have_equal_square_touch_targets() {
    let renderer = renderer();
    let mut buttons = Tool::ALL
        .into_iter()
        .map(|tool| tool_button(tool, Tool::Navigate, true))
        .collect::<Vec<_>>();
    buttons.extend([Glyph::Fill, Glyph::SmartSnap].into_iter().map(|glyph| {
        icon_button(glyph, "Option", false, Some(Message::ModalBackdrop))
    }));
    buttons.extend(COLORS.into_iter().map(|(label, color)| {
        color_button(label, color, AnnotationColor::Black, true)
    }));
    for mut element in buttons {
        let (_, node) = lay_out(&mut element, &renderer);
        assert_eq!(node.size(), iced::Size::new(40.0, 40.0));
    }
}

#[test]
fn icon_and_color_targets_route_mouse_and_touch_to_existing_commands() {
    for touch in [false, true] {
        for tool in Tool::ALL {
            let messages =
                click(tool_button(tool, Tool::Navigate, true), touch);
            assert!(
                matches!(messages.as_slice(), [Message::SetTool(chosen)] if *chosen == tool)
            );
            assert!(click(tool_button(tool, tool, false), touch).is_empty());
        }
        for (glyph, message) in [
            (Glyph::Fill, Message::ToggleFill),
            (
                Glyph::SmartSnap,
                Message::SmartSnap(super::super::snap::Event::Toggle),
            ),
        ] {
            let messages = click(
                icon_button(glyph, "Option", false, Some(message.clone())),
                touch,
            );
            assert_eq!(messages.len(), 1);
            assert_eq!(
                std::mem::discriminant(&messages[0]),
                std::mem::discriminant(&message)
            );
            assert!(
                click(icon_button(glyph, "Option", false, None), touch)
                    .is_empty()
            );
        }
        for (label, color) in COLORS {
            let messages =
                click(color_button(label, color, color, true), touch);
            assert!(
                matches!(messages.as_slice(), [Message::SetColor(chosen)] if *chosen == color)
            );
            assert!(
                click(color_button(label, color, color, false), touch)
                    .is_empty()
            );
        }
    }
}

#[test]
fn text_icon_has_distinct_uppercase_and_lowercase_letter_regions() {
    for theme in [Theme::Light, Theme::Dark] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let pixels = render_themed_element(
                icons::view(Glyph::Tool(Tool::Text), false, true),
                iced::Size::new(24, 24),
                scale,
                dpi,
                &theme,
                mouse::Cursor::Unavailable,
            );
            let scale = usize::try_from(scale).unwrap();
            let background = pixel(&pixels, 24 * scale, 0, 0);
            // Both letters need lower-half ink, not one central I/T stem.
            for (left, right) in [(2, 10), (14, 22)] {
                let ink = (10 * scale..18 * scale)
                    .flat_map(|y| {
                        (left * scale..right * scale).map(move |x| (x, y))
                    })
                    .filter(|(x, y)| {
                        delta(pixel(&pixels, 24 * scale, *x, *y), background)
                            > 30
                    })
                    .count();
                assert!(ink > 8 * scale * scale, "missing Aa letter");
            }
        }
    }
}

#[test]
fn every_glyph_stays_visible_inside_its_frame_in_all_theme_states() {
    for theme in [Theme::Light, Theme::Dark] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let pixels = render_themed_element(
                icon_grid(),
                iced::Size::new(520, 160),
                scale,
                dpi,
                &theme,
                mouse::Cursor::Unavailable,
            );
            let scale = usize::try_from(scale).unwrap();
            let width = 520 * scale;
            for (column, glyph) in glyphs().iter().enumerate() {
                let mut strengths = Vec::new();
                for row in 0..4 {
                    let left = column * 40 * scale;
                    let top = row * 40 * scale;
                    let background = pixel(
                        &pixels,
                        width,
                        left + 5 * scale,
                        top + 5 * scale,
                    );
                    let mut painted = Vec::new();
                    let mut strength = 0;
                    for y in 0..24 * scale {
                        for x in 0..24 * scale {
                            let color = pixel(
                                &pixels,
                                width,
                                left + 8 * scale + x,
                                top + 8 * scale + y,
                            );
                            let difference = delta(color, background);
                            strength = strength.max(difference);
                            if difference > 30 {
                                painted.push((x, y));
                            }
                        }
                    }
                    assert!(
                        painted.len() > 16 * scale * scale,
                        "invisible {glyph:?} {theme:?} row {row}"
                    );
                    // Lucide icons may touch their 24px viewBox. The
                    // button still leaves 8px between icon and its frame.
                    strengths.push(strength);
                }
                assert!(
                    strengths[2] < strengths[0],
                    "disabled {glyph:?} {theme:?} must dim: {strengths:?}"
                );
                // Selected backgrounds fade too, so relative contrast can grow.
                let active = appearance::tool_button(
                    &theme,
                    button::Status::Active,
                    true,
                );
                let disabled = appearance::tool_button(
                    &theme,
                    button::Status::Disabled,
                    true,
                );
                assert!(disabled.text_color.a < active.text_color.a);
            }
            if scale == 1
                && let Some(path) = std::env::var_os("FASTFILL_ICON_SNAPSHOT")
            {
                let path = std::path::PathBuf::from(path)
                    .join(format!("icons-{theme:?}.png"));
                image::save_buffer(
                    path,
                    &pixels,
                    520,
                    160,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn box_and_oval_icons_share_center_size_and_stroke_weight() {
    for theme in [Theme::Light, Theme::Dark] {
        let pixels = render_themed_element(
            Row::with_children([
                tool_button(Tool::Rectangle, Tool::Navigate, true),
                tool_button(Tool::Oval, Tool::Navigate, true),
            ])
            .into(),
            iced::Size::new(80, 40),
            1,
            1.0,
            &theme,
            mouse::Cursor::Unavailable,
        );
        let extents = |left: usize| {
            let bg = pixel(&pixels, 80, left + 5, 5);
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for y in 8..32 {
                for x in 8..32 {
                    if delta(pixel(&pixels, 80, left + x, y), bg) > 30 {
                        xs.push(x);
                        ys.push(y);
                    }
                }
            }
            (
                *xs.iter().min().unwrap(),
                *xs.iter().max().unwrap(),
                *ys.iter().min().unwrap(),
                *ys.iter().max().unwrap(),
            )
        };
        assert_eq!(extents(0), extents(40));
        let (left, right, top, bottom) = extents(0);
        assert_eq!(left + right, 39);
        assert_eq!(top + bottom, 39);
        assert!((16..=22).contains(&(right - left)));
        assert!(bottom > top);
    }
}

#[test]
fn swatches_keep_pigment_on_hover_and_a_gap_around_the_selected_ring() {
    for theme in [Theme::Light, Theme::Dark] {
        for (label, color) in COLORS {
            let expected = color.rgb();
            let mut centers = Vec::new();
            for cursor in [
                mouse::Cursor::Unavailable,
                mouse::Cursor::Available(Point::new(20.0, 20.0)),
            ] {
                let pixels = render_themed_element(
                    color_button(label, color, color, true),
                    iced::Size::new(40, 40),
                    1,
                    1.0,
                    &theme,
                    cursor,
                );
                let center = pixel(&pixels, 40, 20, 20);
                assert_eq!(&center[..3], expected);
                centers.push(center.to_vec());
                assert_eq!(
                    pixel(&pixels, 40, 20, 3),
                    pixel(&pixels, 40, 0, 0),
                    "ring needs a visible gap"
                );
                assert_ne!(
                    pixel(&pixels, 40, 20, 1),
                    pixel(&pixels, 40, 20, 3)
                );
                assert_eq!(
                    pixel(&pixels, 40, 3, 3),
                    pixel(&pixels, 40, 0, 0),
                    "patch must be round"
                );
            }
            assert_eq!(centers[0], centers[1]);
            let patch = appearance::color_patch(&theme, color, false);
            let Some(iced::Background::Color(dimmed)) = patch.background else {
                panic!("color patch");
            };
            let pigment = appearance::annotation_color(color);
            assert_eq!(
                (dimmed.r, dimmed.g, dimmed.b),
                (pigment.r, pigment.g, pigment.b)
            );
            assert!(dimmed.a > 0.0 && dimmed.a < pigment.a);
        }
    }
}

#[test]
fn option_visibility_and_snap_state_survive_placing_a_mark() {
    use crate::app::controller::update;
    use fastfill_rust::annotations::Pointer;
    use fastfill_rust::geometry::Point as PagePoint;

    let mut state = FastFill::default();
    for message in [
        Message::SetTool(Tool::Checkmark),
        Message::SmartSnap(super::super::snap::Event::Toggle),
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 100.0), 1.0),
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(100.0, 100.0),
            false,
        ),
    ] {
        let _ = update(&mut state, message);
    }
    assert!(state.smart_snap.enabled);
    assert!(super::super::snap::supports(state.annotations().tool()));
    assert!(super::super::snap::can_toggle(&state));
    assert!(!state.annotations().fill_available());
    assert!(enabled(&state));
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    assert!(state.smart_snap.enabled);
    assert!(!super::super::snap::supports(state.annotations().tool()));
    assert!(state.annotations().fill_available());
    state.confirm_delete_all = true;
    assert!(!enabled(&state));
}

#[test]
fn action_surfaces_render_themed_states_and_round_corners_at_both_dpi_scales() {
    use iced::Background;
    for theme in [Theme::Light, Theme::Dark] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            for style in [
                appearance::secondary_action,
                appearance::primary_action,
                appearance::destructive_action,
            ] {
                for (enabled, hovered) in
                    [(true, false), (true, true), (false, false)]
                {
                    let cursor = if hovered {
                        mouse::Cursor::Available(Point::new(50.0, 20.0))
                    } else {
                        mouse::Cursor::Unavailable
                    };
                    let pixels = render_themed_element(
                        action_button(
                            "Action",
                            enabled.then_some(Message::Fit),
                        )
                        .style(style)
                        .width(100)
                        .into(),
                        iced::Size::new(100, 40),
                        scale,
                        dpi,
                        &theme,
                        cursor,
                    );
                    let scale = usize::try_from(scale).unwrap();
                    let status = if !enabled {
                        button::Status::Disabled
                    } else if hovered {
                        button::Status::Hovered
                    } else {
                        button::Status::Active
                    };
                    let Some(Background::Color(background)) =
                        style(&theme, status).background
                    else {
                        panic!("solid action surface");
                    };
                    if enabled {
                        assert!(
                            delta(
                                pixel(
                                    &pixels,
                                    100 * scale,
                                    10 * scale,
                                    10 * scale
                                ),
                                &background.into_rgba8(),
                            ) < 4
                        );
                    }
                    assert!(
                        delta(
                            pixel(&pixels, 100 * scale, 0, 0),
                            &theme.palette().background.into_rgba8(),
                        ) < 4,
                        "square action corner"
                    );
                }
            }
        }
    }
}
