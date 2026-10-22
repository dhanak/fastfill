use super::*;
use crate::app::controller::update;
use crate::app::tests::{annotated_page, render_themed_element, rendered_page};
use crate::appearance::TOOLBAR_ROW_HEIGHT;
use fastfill_rust::annotations::AnnotationColor;
use iced::advanced::renderer::Headless;
use iced::advanced::{Layout, Shell, layout, widget};
use iced::{Event, Renderer, Theme, mouse, touch};

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
    size: iced::Size,
) -> (widget::Tree, layout::Node) {
    let mut tree = widget::Tree::new(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        renderer,
        &layout::Limits::new(size, size),
    );
    (tree, node)
}

fn send_input(
    element: &mut Element<'_, Message>,
    tree: &mut widget::Tree,
    node: &layout::Node,
    renderer: &Renderer,
    messages: &mut Vec<Message>,
    event: &Event,
    cursor: mouse::Cursor,
) {
    element.as_widget_mut().update(
        tree,
        event,
        Layout::new(node),
        cursor,
        renderer,
        &mut iced::advanced::clipboard::Null,
        &mut Shell::new(messages),
        &node.bounds(),
    );
}

fn tool_targets(layout: Layout<'_>) -> Vec<iced::Rectangle> {
    if layout.bounds().size() == iced::Size::new(40.0, 40.0) {
        vec![layout.bounds()]
    } else {
        layout.children().flat_map(tool_targets).collect()
    }
}

#[test]
fn compact_sidebar_keeps_four_columns_and_options_aligned_when_scrolling() {
    let renderer = renderer();
    let mut state = annotated_page();
    assert_eq!(appearance::TOOLS_PANEL_WIDTH, 208.0);
    for height in [360.0, 850.0] {
        for tool in [Tool::Line, Tool::Text] {
            let _ = update(&mut state, Message::SetTool(tool));
            let mut element = view(&state);
            let (_, node) = lay_out(
                &mut element,
                &renderer,
                iced::Size::new(appearance::TOOLS_PANEL_WIDTH, height),
            );
            let column = Layout::new(&node).children().next().unwrap();
            let scroll = column.children().next().unwrap();
            let content = scroll.children().next().unwrap();
            let targets = tool_targets(content.children().nth(1).unwrap());
            let header = content.children().next().unwrap().bounds();
            assert_eq!(header.width, 184.0);
            assert_eq!(header.x, TOOLS_PANEL_PADDING);
            assert_eq!(
                node.size().width - header.x - header.width,
                TOOLS_PANEL_PADDING,
            );
            assert_eq!(targets.len(), 11);
            assert_eq!(targets[3].x + CONTROL_HEIGHT, header.x + header.width);
            for row in targets.chunks(4) {
                assert_eq!(row[0].x, header.x);
                for (button, column) in row.iter().zip(&targets[..4]) {
                    assert_eq!(button.x, column.x);
                }
                for pair in row.windows(2) {
                    assert_eq!(pair[0].y, pair[1].y);
                    assert_eq!(pair[1].x - pair[0].x, 48.0);
                }
            }
            for section in content.children() {
                assert_eq!(section.bounds().x, header.x);
                assert!(section.bounds().width <= header.width);
            }
        }
    }
}

#[test]
fn common_colors_stay_above_tool_specific_options() {
    let mut state = FastFill::default();
    let mut baseline = None;
    let mut checked = 0;
    for tool in Tool::ALL {
        let _ = update(&mut state, Message::SetTool(tool));
        if properties(&state, true).is_empty() {
            continue;
        }
        let pixels = render_themed_element(
            view(&state),
            iced::Size::new(208, 850),
            1,
            1.0,
            &Theme::Dark,
            mouse::Cursor::Unavailable,
        );
        let red_row = pixels
            .as_chunks::<{ 208 * 4 }>()
            .0
            .iter()
            .position(|row| {
                row.as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[..3] == AnnotationColor::Red.rgb())
            })
            .expect("red color swatch");
        assert_eq!(
            *baseline.get_or_insert(red_row),
            red_row,
            "color properties moved in {tool:?}",
        );
        checked += 1;
    }
    assert_eq!(checked, 9);
}

#[test]
fn workspace_bounds_stay_fixed_across_tools_in_wide_and_narrow_windows() {
    let renderer = renderer();
    let mut state = annotated_page();
    for size in [
        iced::Size::new(1_036.0, 700.0),
        iced::Size::new(1_036.0, 360.0),
        iced::Size::new(476.0, 500.0),
        iced::Size::new(839.0, 600.0),
        iced::Size::new(840.0, 600.0),
    ] {
        let wide = size.width >= appearance::PANEL_MIN_WORKSPACE_WIDTH;
        let expected = iced::Rectangle {
            x: 0.0,
            y: if wide { 0.0 } else { 2.0 * TOOLBAR_ROW_HEIGHT },
            width: size.width
                - if wide {
                    appearance::TOOLS_PANEL_WIDTH
                } else {
                    0.0
                },
            height: size.height
                - if wide { 0.0 } else { 2.0 * TOOLBAR_ROW_HEIGHT },
        };
        for tool in Tool::ALL {
            let _ = update(&mut state, Message::SetTool(tool));
            let mut element = super::super::workspace(&state, size);
            let (_, node) = lay_out(&mut element, &renderer, size);
            let root = Layout::new(&node);
            let document = if wide {
                root.children().next().unwrap()
            } else {
                root.children().nth(2).unwrap()
            };
            assert_eq!(document.bounds(), expected, "{tool:?} at {size:?}");
        }
    }
}

#[test]
fn short_panels_scroll_properties_but_keep_equal_actions_fixed_and_visible() {
    let renderer = renderer();
    let mut state = annotated_page();
    for height in [360.0, 700.0] {
        let mut baseline = None;
        for tool in Tool::ALL {
            let _ = update(&mut state, Message::SetTool(tool));
            let mut element = view(&state);
            let (_, node) = lay_out(
                &mut element,
                &renderer,
                iced::Size::new(appearance::TOOLS_PANEL_WIDTH, height),
            );
            let column = Layout::new(&node).children().next().unwrap();
            let scroll = column.children().next().unwrap().bounds();
            let footer = column.children().nth(1).unwrap();
            assert_eq!(
                baseline.get_or_insert(footer.bounds()),
                &footer.bounds()
            );
            assert!(scroll.height >= CONTROL_HEIGHT);
            assert!(scroll.y + scroll.height <= footer.bounds().y);
            let mut buttons = Vec::new();
            for row in footer.children().skip(2).take(2) {
                buttons.extend(row.children().map(|b| b.bounds()));
            }
            assert_eq!(buttons.len(), 4);
            let reset = footer
                .children()
                .nth(1)
                .unwrap()
                .children()
                .nth(1)
                .unwrap()
                .bounds();
            assert_eq!(reset.width, buttons[1].width);
            assert_eq!(reset.x, buttons[1].x);
            for button in &buttons {
                assert_eq!(button.height, CONTROL_HEIGHT);
                assert_eq!(button.width, buttons[0].width);
                assert!(button.width >= CONTROL_HEIGHT);
                assert!(button.y + button.height <= height - 12.0);
            }
            let delete_row = footer.children().nth(3).unwrap().bounds();
            assert_eq!(delete_row.width, 184.0);
            assert_eq!(delete_row.height, CONTROL_HEIGHT);
            assert_eq!(delete_row.y + delete_row.height, height - 12.0);
        }
    }
}

#[test]
fn panel_grid_and_footer_route_mouse_and_touch_to_existing_commands() {
    let renderer = renderer();
    let state = annotated_page();
    for touch in [false, true] {
        let mut element = view(&state);
        let (mut tree, node) = lay_out(
            &mut element,
            &renderer,
            iced::Size::new(appearance::TOOLS_PANEL_WIDTH, 700.0),
        );
        let column = Layout::new(&node).children().next().unwrap();
        let scroll = column.children().next().unwrap();
        let content = scroll.children().next().unwrap();
        let grid = tool_targets(content.children().nth(1).unwrap());
        let mut targets = grid
            .iter()
            .copied()
            .zip(Tool::ALL)
            .map(|(bounds, tool)| (bounds, Message::SetTool(tool)))
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 11);
        for bounds in grid {
            assert_eq!(bounds.size(), iced::Size::new(40.0, 40.0));
            assert!(bounds.x >= 12.0 && bounds.x + bounds.width <= 196.0);
        }
        let footer = column.children().nth(1).unwrap();
        for (row, messages) in footer.children().skip(2).zip([
            [Message::SendToBack, Message::BringToFront],
            [Message::DeleteSelected, Message::AskDeleteAll],
        ]) {
            targets.extend(
                row.children()
                    .zip(messages)
                    .map(|(button, message)| (button.bounds(), message)),
            );
        }
        for (bounds, expected) in targets {
            let position = bounds.center();
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
                    Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    )),
                    Event::Mouse(mouse::Event::ButtonReleased(
                        mouse::Button::Left,
                    )),
                ]
            };
            let mut messages = Vec::new();
            for (index, event) in events.into_iter().enumerate() {
                let cursor = if touch && index == 1 {
                    mouse::Cursor::Available(iced::Point::new(-10.0, -10.0))
                } else {
                    mouse::Cursor::Available(position)
                };
                element.as_widget_mut().update(
                    &mut tree,
                    &event,
                    Layout::new(&node),
                    cursor,
                    &renderer,
                    &mut iced::advanced::clipboard::Null,
                    &mut Shell::new(&mut messages),
                    &node.bounds(),
                );
            }
            assert_eq!(messages.len(), 1, "{expected:?}, touch={touch}");
            assert_eq!(
                std::mem::discriminant(&messages[0]),
                std::mem::discriminant(&expected),
            );
            if let Message::SetTool(tool) = expected {
                assert!(
                    matches!(messages[0], Message::SetTool(t) if t == tool)
                );
            }
        }
    }
}

#[test]
fn touch_color_tap_uses_release_position_even_with_stale_mouse_cursor() {
    let renderer = renderer();
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Line));
    let mut element = view(&state);
    let (mut tree, node) = lay_out(
        &mut element,
        &renderer,
        iced::Size::new(appearance::TOOLS_PANEL_WIDTH, 700.0),
    );
    let column = Layout::new(&node).children().next().unwrap();
    let scroll = column.children().next().unwrap();
    let content = scroll.children().next().unwrap();
    let properties = content.children().nth(3).unwrap();
    let position = tool_targets(properties)[2].center();
    let mut messages = Vec::new();
    for (event, cursor) in [
        (
            touch::Event::FingerPressed {
                id: touch::Finger(1),
                position,
            },
            mouse::Cursor::Unavailable,
        ),
        (
            touch::Event::FingerLifted {
                id: touch::Finger(1),
                position,
            },
            mouse::Cursor::Unavailable,
        ),
    ] {
        element.as_widget_mut().update(
            &mut tree,
            &Event::Touch(event),
            Layout::new(&node),
            cursor,
            &renderer,
            &mut iced::advanced::clipboard::Null,
            &mut Shell::new(&mut messages),
            &node.bounds(),
        );
    }
    assert!(matches!(
        messages.as_slice(),
        [Message::SetColor(AnnotationColor::Red)]
    ));
    drop(element);
    let _ = update(&mut state, messages.remove(0));
    assert_eq!(
        state.annotations().current_style().unwrap().color,
        AnnotationColor::Red,
    );
}

#[test]
fn compatibility_mouse_press_does_not_steal_the_next_sidebar_tap() {
    let renderer = renderer();
    let mut state = annotated_page();
    let size = iced::Size::new(appearance::TOOLS_PANEL_WIDTH, 700.0);
    let mut element = view(&state);
    let (mut tree, node) = lay_out(&mut element, &renderer, size);
    let column = Layout::new(&node).children().next().unwrap();
    let content = column.children().next().unwrap().children().next().unwrap();
    let targets = tool_targets(content.children().nth(1).unwrap());
    let first = targets[2].center();
    let next = targets[4].center();
    let finger = touch::Finger(1);
    let mut messages = Vec::new();
    for event in [
        touch::Event::FingerPressed {
            id: finger,
            position: first,
        },
        touch::Event::FingerLifted {
            id: finger,
            position: first,
        },
    ] {
        send_input(
            &mut element,
            &mut tree,
            &node,
            &renderer,
            &mut messages,
            &Event::Touch(event),
            mouse::Cursor::Unavailable,
        );
    }
    assert!(matches!(
        messages.as_slice(),
        [Message::SetTool(Tool::Rectangle)]
    ));
    drop(element);
    let _ = update(&mut state, messages.remove(0));

    let mut element = view(&state);
    tree.diff(element.as_widget());
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(size, size),
    );
    for (event, cursor) in [
        (
            Event::Touch(touch::Event::FingerPressed {
                id: finger,
                position: next,
            }),
            mouse::Cursor::Unavailable,
        ),
        (
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            mouse::Cursor::Available(first),
        ),
        (
            Event::Touch(touch::Event::FingerLifted {
                id: finger,
                position: next,
            }),
            mouse::Cursor::Unavailable,
        ),
        (
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            mouse::Cursor::Available(first),
        ),
    ] {
        send_input(
            &mut element,
            &mut tree,
            &node,
            &renderer,
            &mut messages,
            &event,
            cursor,
        );
    }
    assert!(matches!(
        messages.as_slice(),
        [Message::SetTool(Tool::Line)]
    ));
}

#[test]
fn horizontal_tracks_do_not_cover_the_button_targets() {
    let renderer = renderer();
    let state = annotated_page();
    let mut element = super::super::tools(&state);
    let (_, node) = lay_out(
        &mut element,
        &renderer,
        iced::Size::new(200.0, TOOLBAR_ROW_HEIGHT),
    );
    let row = Layout::new(&node).children().next().unwrap();
    for button in row.children().take(11) {
        assert_eq!(button.bounds().height, CONTROL_HEIGHT);
        assert!(button.bounds().y + CONTROL_HEIGHT <= TOOLBAR_ROW_HEIGHT - 8.0);
    }
}

#[test]
fn workspace_previews_render_in_both_layouts_and_themes() {
    // Headless tests must install font fallbacks independently of test order.
    let _ = crate::appearance::text_font(
        fastfill_rust::annotations::TextFormat::default(),
    );
    let mut state = rendered_page();
    for theme in [Theme::Light, Theme::Dark] {
        for width in [400, 640, 1_200] {
            for tool in [Tool::Navigate, Tool::Rectangle, Tool::Text] {
                let _ = update(&mut state, Message::SetTool(tool));
                let pixels = render_themed_element(
                    crate::app::view::view(&state),
                    iced::Size::new(width, 800),
                    1,
                    1.0,
                    &theme,
                    mouse::Cursor::Unavailable,
                );
                assert_eq!(pixels.len(), width as usize * 800 * 4);
                if let Some(path) =
                    std::env::var_os("FASTFILL_WORKSPACE_SNAPSHOT")
                {
                    image::save_buffer(
                        std::path::PathBuf::from(path).join(format!(
                            "workspace-{width}-{theme:?}-{tool:?}.png"
                        )),
                        &pixels,
                        width,
                        800,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        }
    }
}
