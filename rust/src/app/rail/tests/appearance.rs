use super::*;
use crate::app::tests::render_themed_element;

#[test]
fn compact_page_actions_route_mouse_and_touch() {
    use crate::app::tests::click_element;
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};
    let state = three_pages();
    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    let size = Size::new(164.0, 600.0);
    for (point, command) in [
        (Point::new(28.0, 76.0), "add"),
        (Point::new(76.0, 76.0), "import"),
        (Point::new(28.0, 124.0), "replace"),
        (Point::new(76.0, 124.0), "edit"),
        (Point::new(124.0, 124.0), "delete"),
    ] {
        for touch in [false, true] {
            let mut element = super::super::view::view(&state);
            let mut tree = widget::Tree::new(element.as_widget());
            let node = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(size, size),
            );
            let messages = click_element(
                &mut element,
                &mut tree,
                &node,
                &renderer,
                point,
                touch,
            );
            assert_eq!(messages.len(), 1, "{command}, touch={touch}");
            let correct = matches!(
                (command, &messages[0]),
                ("add", Message::AddPage)
                    | ("import", Message::ImportPdf)
                    | ("replace", Message::AskReplacePage)
                    | ("delete", Message::AskDeletePage)
                    | (
                        "edit",
                        Message::PageEdit(crate::app::page_edit::Event::Begin),
                    )
            );
            assert!(correct, "{command}: {:?}", messages[0]);
        }
    }
}

#[test]
fn page_menu_keeps_both_move_directions_reachable() {
    use crate::app::tests::click_element;
    use iced::Renderer;
    use iced::advanced::renderer::Headless;
    use iced::advanced::{layout, widget};
    assert_eq!(WIDTH, 3.0 * 40.0 + 2.0 * 8.0 + 2.0 * 8.0);
    let mut state = three_pages();
    state.pages.select(1);
    let renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .unwrap();
    for touch in [false, true] {
        let mut rail = super::super::view::view(&state);
        let mut tree = widget::Tree::new(rail.as_widget());
        let size = Size::new(164.0, 600.0);
        let node = rail.as_widget_mut().layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(size, size),
        );
        let messages = click_element(
            &mut rail,
            &mut tree,
            &node,
            &renderer,
            Point::new(124.0, 28.0),
            touch,
        );
        assert!(matches!(
            messages.as_slice(),
            [Message::ToggleMenu(crate::app::menu::Kind::Page)]
        ));
    }
}

#[test]
fn portrait_landscape_and_blank_previews_fill_the_space_after_the_number() {
    let card = Rectangle::new(Point::new(20.0, 30.0), Size::new(130.0, 150.0));
    for size in [
        Size::new(300.0, 400.0),
        Size::new(400.0, 300.0),
        Size::new(612.0, 792.0),
    ] {
        let paper = super::super::layout::paper_bounds(card, size);
        assert!(paper.x >= card.x + 28.0);
        assert!(paper.x + paper.width <= card.x + card.width - 5.9);
        assert!(paper.y >= card.y + 6.0);
        assert!(paper.y + paper.height <= card.y + card.height - 5.9);
        assert_eq!(paper.y, card.y + 6.0);
        assert!(
            (paper.width / paper.height - size.width / size.height).abs()
                < 0.001
        );
        if size.height > size.width {
            assert!(paper.height >= 120.0);
        }
    }
}

#[test]
fn thumbnail_cards_ignore_asset_names_and_annotation_counts() {
    let _ = crate::appearance::text_font(
        fastfill_rust::annotations::TextFormat::default(),
    );
    let mut app = three_pages();
    app.pages.select(0);
    app.page_scroll = 0.0;
    for theme in [Theme::Dark, Theme::Light] {
        for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
            let render = |app: &FastFill| {
                render_themed_element(
                    canvas(Rail { app }).width(148).height(164).into(),
                    Size::new(148, 164),
                    scale,
                    dpi,
                    &theme,
                    mouse::Cursor::Unavailable,
                )
            };
            app.pages.current_mut().image.as_mut().unwrap().name =
                "sample.png".into();
            app.annotations_mut().delete_all();
            app.annotations_mut()
                .set_tool(fastfill_rust::annotations::Tool::Rectangle);
            let before = render(&app);
            app.pages.current_mut().image.as_mut().unwrap().name =
                app.pages.current().metadata.id.to_string();
            app.annotations_mut().begin(
                fastfill_rust::annotations::Pointer::Mouse,
                fastfill_rust::geometry::Point::new(10.0, 20.0),
                PageSize::new(612.0, 816.0),
            );
            assert!(app.annotations_mut().finish(
                fastfill_rust::annotations::Pointer::Mouse,
                fastfill_rust::geometry::Point::new(50.0, 70.0),
                false,
            ));
            assert!(before == render(&app), "metadata changed the page card");
            let scale = usize::try_from(scale).unwrap();
            let offset = (130 * scale * 148 * scale + 80 * scale) * 4;
            assert_eq!(&before[offset..offset + 4], [255, 255, 255, 255]);
        }
    }
}

#[test]
fn page_numbers_one_and_twenty_fit_to_the_left_of_selected_previews() {
    let mut app = three_pages();
    let image = app.image().unwrap().clone();
    assert!(app.pages.import_images(&vec![image; 17]));
    for index in [0, 19] {
        app.pages.select(index);
        app.page_scroll = page_top(&app, index);
        for theme in [Theme::Dark, Theme::Light] {
            for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
                let pixels = render_themed_element(
                    canvas(Rail { app: &app }).width(148).height(164).into(),
                    Size::new(148, 164),
                    scale,
                    dpi,
                    &theme,
                    mouse::Cursor::Unavailable,
                );
                let scale = usize::try_from(scale).unwrap();
                let background =
                    theme.extended_palette().primary.weak.color.into_rgba8();
                assert!(
                    (12 * scale..40 * scale).any(|y| {
                        (7 * scale..27 * scale).any(|x| {
                            let offset = (y * 148 * scale + x) * 4;
                            pixels[offset..offset + 4] != background
                        })
                    }),
                    "page number missing from left gutter"
                );
                assert!(
                    (65 * scale..100 * scale).all(|y| {
                        (7 * scale..27 * scale).all(|x| {
                            let offset = (y * 148 * scale + x) * 4;
                            pixels[offset..offset + 4] == background
                        })
                    }),
                    "page number should be top-aligned, not centered"
                );
                let offset = (130 * scale * 148 * scale + 80 * scale) * 4;
                assert_eq!(&pixels[offset..offset + 4], [255, 255, 255, 255]);
                if let Some(path) = std::env::var_os("FASTFILL_RAIL_SNAPSHOT") {
                    image::save_buffer(
                        std::path::PathBuf::from(path).join(format!(
                            "rail-{}-{theme:?}-{scale}.png",
                            index + 1,
                        )),
                        &pixels,
                        148 * u32::try_from(scale).unwrap(),
                        164 * u32::try_from(scale).unwrap(),
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        }
    }
}
