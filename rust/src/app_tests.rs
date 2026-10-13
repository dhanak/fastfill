use super::*;

#[test]
fn undo_redo_messages_apply_only_to_committed_edits() {
    let mut state = annotated_page();
    let rectangle = state.annotations.shapes()[0].clone();
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations.shapes(), []);
    let _ = update(&mut state, Message::Redo);
    assert_eq!(state.annotations.shapes(), std::slice::from_ref(&rectangle),);

    state.annotations.begin_interaction(
        Pointer::Mouse,
        PagePoint::new(30.0, 45.0),
        DEFAULT_PAGE,
        1.0,
        DragModifiers::default(),
    );
    state
        .annotations
        .drag(Pointer::Mouse, PagePoint::new(40.0, 55.0), false);
    let preview = state.annotations.preview();
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations.preview(), preview);
    assert_eq!(state.annotations.shapes(), [rectangle]);
}

#[test]
fn keyboard_shortcuts_map_to_history_and_edit_commands() {
    let ctrl = keyboard::Modifiers::CTRL;
    let ctrl_shift = ctrl | keyboard::Modifiers::SHIFT;
    assert!(matches!(
        shortcut_message(keyboard::Key::Character("z".into()), ctrl),
        Some(Message::Undo),
    ));
    assert!(matches!(
        shortcut_message(keyboard::Key::Character("Z".into()), ctrl_shift,),
        Some(Message::Redo),
    ));
    assert!(matches!(
        shortcut_message(keyboard::Key::Character("y".into()), ctrl),
        Some(Message::Redo),
    ));
    assert!(
        shortcut_message(
            keyboard::Key::Character("z".into()),
            keyboard::Modifiers::empty(),
        )
        .is_none()
    );
}

#[test]
fn select_drag_is_pending_escape_reverts_and_delete_removes_selection() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let original = state.annotations.shapes()[0].clone();
    let start = screen_point(&state, PagePoint::new(30.0, 45.0));
    let end = Point::new(start.x + 20.0, start.y + 30.0);
    let mut canvas_state = CanvasState::default();
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: end }),
        end,
    );
    assert!(state.annotations.preview().is_some());
    assert_eq!(state.annotations.shapes()[0], original);
    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(state.annotations.shapes()[0], original);
    let _ = update(&mut state, Message::DeleteSelected);
    assert_eq!(state.annotations.shapes(), []);
}

#[test]
fn rectangle_mode_can_resize_selected_object_through_its_handle() {
    let mut state = annotated_page();
    let original = state.annotations.shapes()[0].clone();
    let view = state
        .navigation
        .view(DEFAULT_PAGE, page_viewport(CANVAS.size()))
        .unwrap();
    let scale = view.page_rect().width / DEFAULT_PAGE.width;
    let controls = state.annotations.selection_bounds(scale).unwrap();
    let handle = ResizeCorner::BottomRight.position(controls);
    let start = screen_point(&state, handle);
    let end = Point::new(start.x + 20.0, start.y + 30.0);
    let mut canvas_state = CanvasState::default();
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        end,
    );
    assert_eq!(state.annotations.shapes().len(), 1);
    let resized = &state.annotations.shapes()[0];
    assert!(resized.bounds().width > original.bounds().width);
    assert!(resized.bounds().height > original.bounds().height);
    assert_eq!(resized.stroke_width, original.stroke_width);
}

#[test]
fn lost_touch_in_select_mode_reverts_edit_without_navigating() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let original = state.annotations.shapes()[0].clone();
    let navigation = state.navigation;
    let start = screen_point(&state, PagePoint::new(30.0, 45.0));
    let end = Point::new(start.x + 20.0, start.y + 30.0);
    let mut canvas_state = CanvasState::default();
    for event in [
        touch::Event::FingerPressed {
            id: touch::Finger(1),
            position: start,
        },
        touch::Event::FingerMoved {
            id: touch::Finger(1),
            position: end,
        },
    ] {
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Touch(event),
            end,
        );
    }
    assert!(state.annotations.preview().is_some());
    assert_eq!(state.navigation, navigation);
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Touch(touch::Event::FingerLost {
            id: touch::Finger(1),
            position: end,
        }),
        end,
    );
    assert!(state.annotations.preview().is_none());
    assert_eq!(state.annotations.shapes()[0], original);
    assert_eq!(state.navigation, navigation);
}

#[test]
fn outlines_render_above_imported_image_and_clip_at_both_dpi_scales() {
    let mut state = FastFill {
        page: Some(ImportedImage {
            handle: Handle::from_rgba(
                600,
                600,
                [240, 230, 210, 255].repeat(600 * 600),
            ),
            name: "paper.png".into(),
            size: PageSize::new(600.0, 600.0),
        }),
        ..FastFill::default()
    };
    state.annotations.set_tool(Tool::Rectangle);
    for (start, end) in [
        (PagePoint::new(100.0, 100.0), PagePoint::new(200.0, 200.0)),
        (PagePoint::new(20.0, 20.0), PagePoint::new(-100.0, -100.0)),
    ] {
        state
            .annotations
            .begin(Pointer::Mouse, start, page_size(&state));
        state.annotations.finish(Pointer::Mouse, end, false);
    }
    state.annotations.set_tool(Tool::Navigate);
    for (scale, dpi) in [(1_u32, 1.0), (2, 2.0)] {
        let pixels = render_document(&state, scale, dpi);
        let scale = usize::try_from(scale).unwrap();
        let pixel = |x: usize, y: usize| {
            let offset = (y * scale * 600 * scale + x * scale) * 4;
            &pixels[offset..offset + 4]
        };
        assert!(pixel(124, 150)[0] < 64, "outline must be above image");
        assert_eq!(
            pixel(150, 150),
            [240, 230, 210, 255],
            "image stays visible at {dpi} scale",
        );
        assert_eq!(
            pixel(54, 20),
            [67, 72, 78, 255],
            "clip at page edge at {dpi} scale; corner {:?}",
            pixel(10, 10),
        );
    }
}

#[test]
fn edit_preview_replaces_the_committed_outline() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Rectangle);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations.set_tool(Tool::Select);
    assert!(state.annotations.begin_interaction(
        Pointer::Mouse,
        PagePoint::new(150.0, 150.0),
        page_size(&state),
        0.88,
        DragModifiers::default(),
    ));
    state
        .annotations
        .drag(Pointer::Mouse, PagePoint::new(350.0, 350.0), false);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert_eq!(pixel(124, 150), [240, 230, 210, 255]);
    assert_ne!(pixel(300, 344), [240, 230, 210, 255]);
}

#[test]
fn draft_and_edit_preview_use_the_annotation_color() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Rectangle);
    state.annotations.set_color(AnnotationColor::Red);
    state.annotations.set_filled(true);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page_size(&state),
    );
    state
        .annotations
        .drag(Pointer::Mouse, PagePoint::new(200.0, 200.0), false);

    let pixels = render_document(&state, 1, 1.0);
    assert_eq!(&pixels[(168 * 600 + 168) * 4..][..4], [220, 32, 32, 255]);

    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    assert!(state.annotations.set_color(AnnotationColor::Green));
    assert!(state.annotations.begin_interaction(
        Pointer::Mouse,
        PagePoint::new(150.0, 150.0),
        page_size(&state),
        0.88,
        DragModifiers::default(),
    ));
    state
        .annotations
        .drag(Pointer::Mouse, PagePoint::new(250.0, 250.0), false);

    let pixels = render_document(&state, 1, 1.0);
    assert_eq!(&pixels[(256 * 600 + 256) * 4..][..4], [0, 160, 72, 255]);
}

#[test]
fn oval_renders_a_curved_unfilled_outline() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Oval);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations.set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert_eq!(pixel(124, 124), [240, 230, 210, 255]);
    assert!(pixel(168, 124)[0] < 64);
    assert_eq!(pixel(168, 168), [240, 230, 210, 255]);
}

#[test]
fn line_renders_above_the_page_and_clips_at_its_edge() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Line);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(500.0, 500.0),
        false,
    );
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(20.0, 300.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(-100.0, 300.0),
        false,
    );
    state.annotations.set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert!(pixel(300, 300)[0] < 64);
    assert_eq!(pixel(20, 300), [67, 72, 78, 255]);
}

#[test]
fn arrow_renders_a_directional_head_and_clips_all_segments() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Arrow);
    for (start, end) in [
        (PagePoint::new(100.0, 300.0), PagePoint::new(500.0, 300.0)),
        (PagePoint::new(20.0, 400.0), PagePoint::new(-100.0, 400.0)),
    ] {
        state
            .annotations
            .begin(Pointer::Mouse, start, page_size(&state));
        state.annotations.finish(Pointer::Mouse, end, false);
    }
    state.annotations.set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let dark_near = |x: usize, y: usize| {
        (y - 2..=y + 2).any(|row| {
            (x - 2..=x + 2).any(|column| pixels[(row * 600 + column) * 4] < 128)
        })
    };
    assert!(dark_near(300, 300), "arrow shaft missing");
    assert!(dark_near(462, 294), "upper arrowhead side missing");
    assert!(dark_near(462, 306), "lower arrowhead side missing");
    assert_eq!(
        &pixels[(388 * 600 + 20) * 4..(388 * 600 + 20) * 4 + 4],
        [67, 72, 78, 255],
    );
}

#[test]
fn marks_render_as_strokes_without_a_surrounding_box() {
    let mut state = rendered_page();
    for (tool, center) in [
        (Tool::Checkmark, PagePoint::new(300.0, 300.0)),
        (Tool::Cross, PagePoint::new(400.0, 400.0)),
    ] {
        state.annotations.set_tool(tool);
        state
            .annotations
            .begin(Pointer::Mouse, center, page_size(&state));
        state.annotations.finish(Pointer::Mouse, center, false);
    }
    state.annotations.set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    let dark_near = |x: usize, y: usize| {
        (y - 2..=y + 2).any(|row| {
            (x - 2..=x + 2).any(|column| pixels[(row * 600 + column) * 4] < 128)
        })
    };
    assert!(dark_near(298, 305), "checkmark missing");
    assert!(dark_near(388, 388), "cross missing");
    assert_eq!(pixel(292, 292), [240, 230, 210, 255]);
    assert_eq!(pixel(388, 380), [240, 230, 210, 255]);
    assert_eq!(pixel(298, 310), [240, 230, 210, 255]);

    let viewport = page_viewport(iced::Size::new(600.0, 600.0));
    let page = page_size(&state);
    state
        .navigation
        .zoom_at(4.0, viewport.center(), page, viewport);
    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert_eq!(pixel(291, 336), [240, 230, 210, 255]);
}

#[test]
fn colors_and_filled_shapes_render_without_filling_oval_corners() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Rectangle);
    state.annotations.set_color(AnnotationColor::Red);
    state.annotations.set_filled(true);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations.set_tool(Tool::Oval);
    state.annotations.set_color(AnnotationColor::Green);
    state.annotations.set_filled(true);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(300.0, 100.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(500.0, 200.0),
        false,
    );
    state.annotations.set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    let red = pixel(150, 150);
    assert!(
        i16::from(red[0]) > i16::from(red[1]) + 100 && red[2] < 80,
        "red rectangle fill missing: {red:?}",
    );
    let green = pixel(400, 150);
    assert!(
        i16::from(green[1]) > i16::from(green[0]) + 80 && green[2] < 140,
        "green oval fill missing: {green:?}",
    );
    assert_eq!(
        pixel(310, 110),
        [240, 230, 210, 255],
        "oval fill must not paint its rectangular corner",
    );
}

#[test]
fn pen_fill_and_translucent_highlighter_render_over_the_page() {
    let mut state = rendered_page();
    state.annotations.set_tool(Tool::Pen);
    state.annotations.set_color(AnnotationColor::Blue);
    state.annotations.set_filled(true);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page_size(&state),
    );
    state
        .annotations
        .drag(Pointer::Mouse, PagePoint::new(200.0, 100.0), false);
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(150.0, 200.0),
        false,
    );

    state.annotations.set_tool(Tool::Highlighter);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(300.0, 100.0),
        page_size(&state),
    );
    state.annotations.finish(
        Pointer::Mouse,
        PagePoint::new(500.0, 100.0),
        false,
    );
    state.annotations.set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert_eq!(pixel(168, 150), [0, 102, 204, 255]);
    let highlight = pixel(388, 124);
    assert!(highlight[0] >= 240, "highlight red: {highlight:?}");
    assert!(
        (195..230).contains(&highlight[1]),
        "highlight green: {highlight:?}",
    );
    assert!(
        (100..200).contains(&highlight[2]),
        "highlight blue: {highlight:?}",
    );
}

fn rendered_page() -> FastFill {
    FastFill {
        page: Some(ImportedImage {
            handle: Handle::from_rgba(
                600,
                600,
                [240, 230, 210, 255].repeat(600 * 600),
            ),
            name: "paper.png".into(),
            size: PageSize::new(600.0, 600.0),
        }),
        ..FastFill::default()
    }
}

fn render_document(state: &FastFill, scale: u32, dpi: f32) -> Vec<u8> {
    use iced::advanced::Renderer as _;
    use iced::advanced::renderer::{Headless, Style};
    use iced::advanced::{Layout, layout, widget};

    let mut renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .expect("software renderer needs no display or GPU");
    let mut element = document(state);
    let mut tree = widget::Tree::new(element.as_widget());
    let size = iced::Size::new(600.0, 600.0);
    renderer.reset(Rectangle::new(Point::ORIGIN, size));
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(size, size),
    );
    element.as_widget().draw(
        &tree,
        &mut renderer,
        &Theme::Light,
        &Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::new(Point::ORIGIN, size),
    );
    renderer.screenshot(
        iced::Size::new(600 * scale, 600 * scale),
        dpi,
        Color::WHITE,
    )
}

fn annotated_page() -> FastFill {
    let mut state = FastFill::default();
    state.annotations.set_tool(Tool::Rectangle);
    state.annotations.begin(
        Pointer::Mouse,
        PagePoint::new(10.0, 20.0),
        DEFAULT_PAGE,
    );
    state
        .annotations
        .finish(Pointer::Mouse, PagePoint::new(50.0, 70.0), false);
    state
}

#[test]
fn canceling_page_replacement_keeps_annotations() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::OpenImage);
    assert_eq!(state.image_picker, ImagePicker::ConfirmReplace);

    let _ = update(&mut state, Message::KeepPage);
    assert_eq!(state.image_picker, ImagePicker::Idle);
    assert_eq!(state.annotations.shapes().len(), 1);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations.shapes(), []);
}

#[test]
fn canceled_or_failed_picker_keeps_annotations() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::OpenImage);
    let _ = update(&mut state, Message::ReplacePage);
    assert_eq!(state.image_picker, ImagePicker::Picking);
    let _ = update(&mut state, Message::ImagePicked(Ok(None)));
    assert_eq!(state.image_picker, ImagePicker::Idle);
    assert_eq!(state.annotations.shapes().len(), 1);

    let _ = update(
        &mut state,
        Message::ImagePicked(Err("Corrupt image".into())),
    );
    assert!(state.status_is_error);
    assert_eq!(state.annotations.shapes().len(), 1);
}

#[test]
fn successful_import_resets_annotations_and_retains_tool() {
    let mut state = annotated_page();
    let image = ImportedImage {
        handle: Handle::from_rgba(1, 1, vec![255; 4]),
        name: "replacement.png".into(),
        size: PageSize::new(1.0, 1.0),
    };
    let _ = update(&mut state, Message::OpenImage);
    let _ = update(&mut state, Message::ReplacePage);
    let _ = update(&mut state, Message::ImagePicked(Ok(Some(image))));

    assert_eq!(state.annotations.shapes(), []);
    assert_eq!(state.annotations.tool(), Tool::Rectangle);
    assert!(!state.annotations.can_undo());
    assert!(!state.annotations.can_redo());
    assert_eq!(state.page.unwrap().name, "replacement.png");
}

#[test]
fn cancel_and_navigation_changes_never_commit_a_draft() {
    let viewport = PageRect::new(36.0, 36.0, 1_000.0, 600.0);
    for message in [
        Message::CancelInteraction,
        Message::FocusLost,
        Message::SetTool(Tool::Navigate),
        Message::Fit,
        Message::OneToOne(viewport),
        Message::ZoomAt {
            factor: 2.0,
            anchor: viewport.center(),
            viewport,
        },
    ] {
        let mut state = annotated_page();
        state.annotations.begin(
            Pointer::Mouse,
            PagePoint::new(100.0, 100.0),
            DEFAULT_PAGE,
        );
        let _ = update(&mut state, message);
        assert!(state.annotations.active_pointer().is_none());
        assert_eq!(state.annotations.shapes().len(), 1);
    }
}

const CANVAS: Rectangle =
    Rectangle::new(Point::new(14.0, 192.0), iced::Size::new(1_200.0, 650.0));

fn screen_point(state: &FastFill, point: PagePoint) -> Point {
    let view = state
        .navigation
        .view(page_size(state), page_viewport(CANVAS.size()))
        .unwrap();
    let local = view.page_to_view(point);
    Point::new(local.x + CANVAS.x, local.y + CANVAS.y)
}

fn canvas_event(
    state: &mut FastFill,
    canvas_state: &mut CanvasState,
    event: &canvas::Event,
    position: Point,
) -> Option<Message> {
    let page = PageCanvas {
        image: state.page.as_ref(),
        navigation: state.navigation,
        page_size: page_size(state),
        annotations: &state.annotations,
        input_epoch: state.input_epoch,
        input_enabled: state.image_picker == ImagePicker::Idle,
        modifiers: state.modifiers,
        background: false,
    };
    let message = canvas::Program::update(
        &page,
        canvas_state,
        event,
        CANVAS,
        mouse::Cursor::Available(position),
    )?
    .into_inner()
    .0?;
    let _ = update(state, message.clone());
    Some(message)
}

#[test]
fn mouse_drawing_uses_canvas_offset_and_ctrl_without_panning() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let mut canvas_state = CanvasState::default();
    let start = screen_point(&state, PagePoint::new(100.0, 200.0));
    let end = screen_point(&state, PagePoint::new(220.0, 250.0));
    let navigation = state.navigation;

    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: end }),
        end,
    );
    let _ = update(
        &mut state,
        Message::ModifiersChanged(DragModifiers {
            square: true,
            ..DragModifiers::default()
        }),
    );
    let preview = state.annotations.preview().unwrap().bounds();
    assert!((preview.width - preview.height).abs() < 0.001);
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        end,
    );
    assert_eq!(state.navigation, navigation);
    let rectangle = state.annotations.shapes()[0].bounds();
    assert!((rectangle.x - 100.0).abs() < 0.001);
    assert!((rectangle.y - 200.0).abs() < 0.001);
    assert!((rectangle.width - 120.0).abs() < 0.001);
    assert!((rectangle.height - 120.0).abs() < 0.001);
}

#[test]
fn right_drag_pans_in_rectangle_mode_without_creating_an_object() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let mut canvas_state = CanvasState::default();
    let start = screen_point(&state, PagePoint::new(100.0, 200.0));
    let end = Point::new(start.x + 20.0, start.y + 30.0);
    let before = screen_point(&state, PagePoint::new(0.0, 0.0));

    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Right,
        )),
        start,
    );
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::CursorMoved { position: end }),
        end,
    );
    let after = screen_point(&state, PagePoint::new(0.0, 0.0));
    assert!((after.x - before.x - 20.0).abs() < 0.001);
    assert!((after.y - before.y - 30.0).abs() < 0.001);
    assert_eq!(state.annotations.shapes(), []);
}

#[test]
fn wheel_zooms_around_pointer_in_all_tools() {
    for tool in [
        Tool::Navigate,
        Tool::Select,
        Tool::Rectangle,
        Tool::Oval,
        Tool::Line,
        Tool::Arrow,
        Tool::Checkmark,
        Tool::Cross,
    ] {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::SetTool(tool));
        let mut canvas_state = CanvasState::default();
        let anchor = PagePoint::new(100.0, 200.0);
        let before = screen_point(&state, anchor);
        let message = canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            before,
        );
        assert!(matches!(message, Some(Message::ZoomAt { .. })));
        assert!((state.navigation.zoom() - 1.2).abs() < 0.001);
        let after = screen_point(&state, anchor);
        assert!((after.x - before.x).abs() < 0.001);
        assert!((after.y - before.y).abs() < 0.001);
        assert_eq!(state.annotations.shapes(), []);
    }
}

#[test]
fn wheel_does_not_interrupt_active_mouse_or_touch_gestures() {
    for tool in [Tool::Navigate, Tool::Rectangle, Tool::Oval] {
        for pointer in 0..3 {
            let mut state = FastFill::default();
            let _ = update(&mut state, Message::SetTool(tool));
            let mut canvas_state = CanvasState::default();
            let start = screen_point(&state, PagePoint::new(100.0, 200.0));
            let press = match pointer {
                0 => canvas::Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Left,
                )),
                1 => canvas::Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Right,
                )),
                _ => canvas::Event::Touch(touch::Event::FingerPressed {
                    id: touch::Finger(1),
                    position: start,
                }),
            };
            canvas_event(&mut state, &mut canvas_state, &press, start);
            let navigation = state.navigation;
            let preview = state.annotations.preview();
            let message = canvas_event(
                &mut state,
                &mut canvas_state,
                &canvas::Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
                }),
                start,
            );
            assert!(message.is_none());
            assert_eq!(state.navigation, navigation);
            assert_eq!(state.annotations.preview(), preview);
        }
    }
}

#[test]
fn mouse_release_outside_canvas_keeps_partial_page_geometry() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let mut canvas_state = CanvasState::default();
    let start = screen_point(&state, PagePoint::new(100.0, 200.0));
    let end = Point::new(CANVAS.x - 20.0, CANVAS.y + 20.0);
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        start,
    );
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Mouse(mouse::Event::ButtonReleased(
            mouse::Button::Left,
        )),
        end,
    );
    assert!(state.annotations.shapes()[0].bounds().x < 0.0);
    assert!(state.annotations.active_pointer().is_none());
}

#[test]
fn drawing_touch_ignores_second_finger_and_lost_touch_cancels() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    let mut canvas_state = CanvasState::default();
    let first = touch::Finger(1);
    let second = touch::Finger(2);
    let start = screen_point(&state, PagePoint::new(100.0, 200.0));
    let end = screen_point(&state, PagePoint::new(220.0, 250.0));
    let navigation = state.navigation;

    for event in [
        touch::Event::FingerPressed {
            id: first,
            position: start,
        },
        touch::Event::FingerPressed {
            id: second,
            position: end,
        },
        touch::Event::FingerMoved {
            id: first,
            position: end,
        },
        touch::Event::FingerMoved {
            id: second,
            position: start,
        },
        touch::Event::FingerLifted {
            id: second,
            position: start,
        },
    ] {
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Touch(event),
            start,
        );
    }
    assert_eq!(state.navigation, navigation);
    assert_eq!(state.annotations.active_pointer(), Some(Pointer::Touch(1)));
    assert_eq!(state.annotations.shapes(), []);
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Touch(touch::Event::FingerLost {
            id: first,
            position: end,
        }),
        end,
    );
    assert!(state.annotations.active_pointer().is_none());
    assert_eq!(state.annotations.shapes(), []);

    for event in [
        touch::Event::FingerPressed {
            id: first,
            position: start,
        },
        touch::Event::FingerLifted {
            id: first,
            position: end,
        },
    ] {
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Touch(event),
            end,
        );
    }
    assert_eq!(state.annotations.shapes().len(), 1);
    assert_eq!(state.navigation, navigation);
}
