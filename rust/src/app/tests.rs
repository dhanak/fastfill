use super::*;

#[test]
fn group_font_changes_preserve_other_properties_and_shape_controls() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(100.0, 200.0), 1.0),
    );
    paste_text(&mut state, "First");
    let _ = update(&mut state, Message::ToggleTextItalic);
    let _ = update(&mut state, Message::SetColor(AnnotationColor::Red));
    let _ = update(&mut state, Message::AcceptText);
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 400.0), 1.0),
    );
    paste_text(&mut state, "A much longer second text");
    let _ = update(&mut state, Message::ToggleTextItalic);
    let _ = update(&mut state, Message::SetTextTypeface(TextTypeface::Serif));
    let _ = update(&mut state, Message::SetTextSize(32.0));
    let _ = update(&mut state, Message::SetColor(AnnotationColor::Blue));
    let _ = update(&mut state, Message::AcceptText);
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(1.0, 1.0), 1.0),
    );
    let _ = update(
        &mut state,
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(600.0, 600.0),
            false,
        ),
    );
    assert_eq!(state.annotations().selected_count(), 3);
    let before = state.annotations().texts().to_vec();
    let shapes = state.annotations().shapes().to_vec();
    assert!(state.annotations().current_text_format().is_some());
    let _ = update(&mut state, Message::ToggleTextBold);
    for (text, original) in state.annotations().texts().iter().zip(&before) {
        assert!(text.format.bold);
        assert_eq!(text.format.typeface, original.format.typeface);
        assert_eq!(text.format.font_size, original.format.font_size);
        assert_eq!(text.format.italic, original.format.italic);
        assert_eq!(text.format.color, original.format.color);
        assert_eq!(text.id, original.id);
        let size = super::text::measure_text(&text.text, text.format, 1.0);
        assert_eq!(
            (text.bounds.width, text.bounds.height),
            (size.width, size.height)
        );
    }
    assert_eq!(state.annotations().shapes(), shapes);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().texts(), before);
    let _ = update(&mut state, Message::Redo);
    let _ = update(&mut state, Message::ToggleTextItalic);
    assert!(
        state
            .annotations()
            .texts()
            .iter()
            .all(|text| text.format.italic)
    );
    let _ = update(&mut state, Message::ToggleTextItalic);
    assert!(
        state
            .annotations()
            .texts()
            .iter()
            .all(|text| !text.format.italic)
    );
    let _ = update(&mut state, Message::ToggleTextUnderline);
    let _ = update(&mut state, Message::SetTextSize(40.0));
    let _ = update(
        &mut state,
        Message::SetTextTypeface(TextTypeface::Monospace),
    );
    assert!(state.annotations().texts().iter().all(
        |text| text.format.underline
            && text.format.typeface == TextTypeface::Monospace
            && (text.format.font_size - 40.0).abs() < 0.01
    ));
    let _ = update(&mut state, Message::SetColor(AnnotationColor::Green));
    assert!(
        state
            .annotations()
            .texts()
            .iter()
            .all(|text| text.format.color == AnnotationColor::Green)
    );
    assert_eq!(
        state.annotations().shapes()[0].color,
        AnnotationColor::Green
    );
    let _ = update(&mut state, Message::AdjustThickness(1.0));
    assert!(
        state.annotations().shapes()[0].stroke_width > shapes[0].stroke_width
    );
}

#[test]
fn select_ctrl_click_and_marquee_keep_text_in_mixed_groups() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let point = PagePoint::new(300.0, 300.0);
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "Group member");
    let _ = update(&mut state, Message::AcceptText);
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    state.modifiers.square = true;
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(30.0, 45.0), 1.0),
    );
    let _ = update(
        &mut state,
        Message::DrawReleased(Pointer::Mouse, PagePoint::new(30.0, 45.0), true),
    );
    assert_eq!(state.annotations().selected_count(), 2);
    assert!(state.annotations().active_pointer().is_none());
    state.modifiers.square = false;
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    let _ = update(
        &mut state,
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(320.0, 330.0),
            false,
        ),
    );
    assert_eq!(state.annotations().selected_count(), 2);
    assert_eq!(state.annotations().texts()[0].bounds.x, 320.0);
    let _ = update(&mut state, Message::Undo);
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(5.0, 5.0), 1.0),
    );
    let _ = update(
        &mut state,
        Message::DrawReleased(
            Pointer::Mouse,
            PagePoint::new(600.0, 600.0),
            false,
        ),
    );
    assert_eq!(state.annotations().selected_count(), 2);
}

#[test]
fn undo_redo_messages_apply_only_to_committed_edits() {
    let mut state = annotated_page();
    let rectangle = state.annotations().shapes()[0].clone();
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().shapes(), []);
    let _ = update(&mut state, Message::Redo);
    assert_eq!(
        state.annotations().shapes(),
        std::slice::from_ref(&rectangle),
    );

    state.annotations_mut().begin_interaction(
        Pointer::Mouse,
        PagePoint::new(30.0, 45.0),
        DEFAULT_PAGE,
        1.0,
        DragModifiers::default(),
    );
    state.annotations_mut().drag(
        Pointer::Mouse,
        PagePoint::new(40.0, 55.0),
        false,
    );
    let preview = state.annotations().preview();
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().preview(), preview);
    assert_eq!(state.annotations().shapes(), [rectangle]);
}

#[test]
fn keyboard_shortcuts_map_to_history_and_edit_commands() {
    let command = keyboard::Modifiers::COMMAND;
    let command_shift = command | keyboard::Modifiers::SHIFT;
    assert!(matches!(
        shortcut_message(keyboard::Key::Character("z".into()), command),
        Some(Message::Undo),
    ));
    assert!(matches!(
        shortcut_message(keyboard::Key::Character("Z".into()), command_shift,),
        Some(Message::Redo),
    ));
    assert!(matches!(
        shortcut_message(keyboard::Key::Character("y".into()), command),
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
fn plain_text_messages_accept_cancel_and_commit_on_click_away() {
    let mut state = FastFill::default();
    let first = PagePoint::new(300.0, 400.0);
    let second = PagePoint::new(400.0, 600.0);
    let _ = update(&mut state, Message::SetTool(Tool::Text));

    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, first, 1.0));
    paste_text(&mut state, "Cancel me");
    let first_bounds = state.annotations().text_preview().unwrap().bounds;
    assert_eq!(first_bounds.x, first.x);
    assert!(first_bounds.y < first.y);
    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(state.annotations().texts(), []);
    assert!(!state.annotations().can_undo());

    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, first, 1.0));
    paste_text(&mut state, "First");
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, second, 1.0),
    );
    assert_eq!(state.annotations().texts()[0].text, "First");
    assert_eq!(
        state.annotations().text_preview().unwrap().bounds.x,
        second.x,
    );

    paste_text(&mut state, "Second");
    let _ = update(&mut state, Message::AcceptText);
    assert_eq!(state.annotations().texts().len(), 2);
    assert!(state.annotations().text_preview().is_none());
}

#[test]
fn switching_tools_accepts_pending_text() {
    let mut state = FastFill::default();
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 400.0), 1.0),
    );
    paste_text(&mut state, "Accepted");

    let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
    assert_eq!(state.annotations().texts()[0].text, "Accepted");
    assert_eq!(state.annotations().tool(), Tool::Rectangle);
}

#[test]
fn multiline_text_reopens_from_text_and_selects_with_select_tool() {
    let mut state = FastFill::default();
    let point = PagePoint::new(300.0, 400.0);
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "First");
    let _ = update(
        &mut state,
        Message::TextAction(text_editor::Action::Edit(
            text_editor::Edit::Enter,
        )),
    );
    paste_text(&mut state, "Second");
    let preview = state.annotations().text_preview().unwrap();
    assert_eq!(preview.text, "First\nSecond");
    assert!(preview.bounds.height > 24.0 * 1.3);
    let _ = update(&mut state, Message::AcceptText);

    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    assert!(state.annotations().has_text_edit());
    let _ = update(&mut state, Message::CancelInteraction);

    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    let _ = update(
        &mut state,
        Message::DrawReleased(Pointer::Mouse, point, false),
    );
    assert!(!state.annotations().has_text_edit());
    assert!(state.annotations().selected_text_annotation().is_some());
}

#[test]
fn text_format_messages_stay_pending_until_acceptance() {
    let mut state = FastFill::default();
    let point = PagePoint::new(300.0, 400.0);
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "Format me");
    let _ = update(&mut state, Message::AcceptText);
    let original = state.annotations().texts()[0].clone();

    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    let _ = update(&mut state, Message::SetTextTypeface(TextTypeface::Serif));
    let _ = update(&mut state, Message::SetTextSize(40.0));
    let _ = update(&mut state, Message::ToggleTextBold);
    let _ = update(&mut state, Message::ToggleTextItalic);
    let _ = update(&mut state, Message::ToggleTextUnderline);
    let _ = update(&mut state, Message::SetColor(AnnotationColor::Blue));
    let preview = state.annotations().text_preview().unwrap();
    assert_eq!(preview.format.typeface, TextTypeface::Serif);
    assert_eq!(preview.format.font_size, 40.0);
    assert!(preview.format.bold);
    assert!(preview.format.italic);
    assert!(preview.format.underline);
    assert_eq!(preview.format.color, AnnotationColor::Blue);
    assert!(preview.bounds.height > original.bounds.height);
    assert_eq!(state.annotations().texts(), std::slice::from_ref(&original),);

    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(state.annotations().texts(), [original]);
}

#[test]
fn accepted_text_requires_confirmation_before_page_replacement() {
    let mut state = FastFill::default();
    state.annotations_mut().set_tool(Tool::Text);
    let page = page_size(&state);
    state
        .annotations_mut()
        .begin_text(PagePoint::new(300.0, 400.0), page, 1.0);
    state
        .annotations_mut()
        .update_text("Keep me", PageSize::new(100.0, 31.2));
    state.annotations_mut().accept_text();

    let _ = update(&mut state, Message::AskReplacePage);
    assert_eq!(state.image_picker, ImagePicker::ConfirmReplace);
    assert_eq!(state.annotations().texts()[0].text, "Keep me");
}

#[test]
fn select_drag_is_pending_escape_reverts_and_delete_removes_selection() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let original = state.annotations().shapes()[0].clone();
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
    assert!(state.annotations().preview().is_some());
    assert_eq!(state.annotations().shapes()[0], original);
    let _ = update(&mut state, Message::CancelInteraction);
    assert_eq!(state.annotations().shapes()[0], original);
    let _ = update(&mut state, Message::DeleteSelected);
    assert_eq!(state.annotations().shapes(), []);
}

#[test]
fn rectangle_mode_moves_tiny_objects_and_resizes_after_zooming_in() {
    for zoom in [1.0, 4.0] {
        let mut state = annotated_page();
        let original = state.annotations().shapes()[0].clone();
        let viewport = page_viewport(CANVAS.size());
        let view = state.navigation.view(DEFAULT_PAGE, viewport).unwrap();
        state.navigation.zoom_at(
            zoom,
            view.page_to_view(original.bounds().center()),
            DEFAULT_PAGE,
            viewport,
        );
        let view = state
            .navigation
            .view(DEFAULT_PAGE, page_viewport(CANVAS.size()))
            .unwrap();
        let scale = view.page_rect().width / DEFAULT_PAGE.width;
        let controls = state.annotations().selection_bounds(scale).unwrap();
        let handle = ResizeCorner::BottomRight.position(controls);
        let start = screen_point(&state, handle);
        let end = Point::new(start.x + 20.0, start.y + 30.0);
        let mut canvas_state = CanvasState::default();
        canvas_event(
            &mut state,
            &mut canvas_state,
            &canvas::Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            )),
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
        assert_eq!(state.annotations().shapes().len(), 1);
        let resized = &state.annotations().shapes()[0];
        if original.edit_handles_visible(scale) {
            assert!(resized.bounds().width > original.bounds().width);
            assert!(resized.bounds().height > original.bounds().height);
        } else {
            assert_eq!(resized.bounds().width, original.bounds().width);
            assert_eq!(resized.bounds().height, original.bounds().height);
            assert!(resized.bounds().x > original.bounds().x);
        }
        assert_eq!(resized.stroke_width, original.stroke_width);
    }
}

#[test]
fn lost_touch_in_select_mode_reverts_edit_without_navigating() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::SetTool(Tool::Select));
    let original = state.annotations().shapes()[0].clone();
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
    assert!(state.annotations().preview().is_some());
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
    assert!(state.annotations().preview().is_none());
    assert_eq!(state.annotations().shapes()[0], original);
    assert_eq!(state.navigation, navigation);
}

#[test]
fn outlines_render_above_imported_image_and_clip_at_both_dpi_scales() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Rectangle);
    for (start, end) in [
        (PagePoint::new(100.0, 100.0), PagePoint::new(200.0, 200.0)),
        (PagePoint::new(20.0, 20.0), PagePoint::new(-100.0, -100.0)),
    ] {
        let page = page_size(&state);
        state.annotations_mut().begin(Pointer::Mouse, start, page);
        state.annotations_mut().finish(Pointer::Mouse, end, false);
    }
    state.annotations_mut().set_tool(Tool::Navigate);
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
    state.annotations_mut().set_tool(Tool::Rectangle);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Select);
    let page = page_size(&state);
    assert!(state.annotations_mut().begin_interaction(
        Pointer::Mouse,
        PagePoint::new(150.0, 150.0),
        page,
        0.88,
        DragModifiers::default(),
    ));
    state.annotations_mut().drag(
        Pointer::Mouse,
        PagePoint::new(350.0, 350.0),
        false,
    );

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
    state.annotations_mut().set_tool(Tool::Rectangle);
    state.annotations_mut().set_color(AnnotationColor::Red);
    state.annotations_mut().set_filled(true);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().drag(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );

    let pixels = render_document(&state, 1, 1.0);
    assert_eq!(&pixels[(168 * 600 + 168) * 4..][..4], [220, 32, 32, 255]);

    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    assert!(state.annotations_mut().set_color(AnnotationColor::Green));
    let page = page_size(&state);
    assert!(state.annotations_mut().begin_interaction(
        Pointer::Mouse,
        PagePoint::new(150.0, 150.0),
        page,
        0.88,
        DragModifiers::default(),
    ));
    state.annotations_mut().drag(
        Pointer::Mouse,
        PagePoint::new(250.0, 250.0),
        false,
    );

    let pixels = render_document(&state, 1, 1.0);
    assert_eq!(&pixels[(256 * 600 + 256) * 4..][..4], [0, 160, 72, 255]);
}

#[test]
fn oval_renders_a_curved_unfilled_outline() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Oval);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Navigate);

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
    state.annotations_mut().set_tool(Tool::Line);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(500.0, 500.0),
        false,
    );
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(20.0, 300.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(-100.0, 300.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Navigate);

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
    state.annotations_mut().set_tool(Tool::Arrow);
    for (start, end) in [
        (PagePoint::new(100.0, 300.0), PagePoint::new(500.0, 300.0)),
        (PagePoint::new(20.0, 400.0), PagePoint::new(-100.0, 400.0)),
    ] {
        let page = page_size(&state);
        state.annotations_mut().begin(Pointer::Mouse, start, page);
        state.annotations_mut().finish(Pointer::Mouse, end, false);
    }
    state.annotations_mut().set_tool(Tool::Navigate);

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
        state.annotations_mut().set_tool(tool);
        let page = page_size(&state);
        state.annotations_mut().begin(Pointer::Mouse, center, page);
        state
            .annotations_mut()
            .finish(Pointer::Mouse, center, false);
    }
    state.annotations_mut().set_tool(Tool::Navigate);

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
    state.annotations_mut().set_tool(Tool::Rectangle);
    state.annotations_mut().set_color(AnnotationColor::Red);
    state.annotations_mut().set_filled(true);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Oval);
    state.annotations_mut().set_color(AnnotationColor::Green);
    state.annotations_mut().set_filled(true);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(300.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(500.0, 200.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Navigate);

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
    state.annotations_mut().set_tool(Tool::Pen);
    state.annotations_mut().set_color(AnnotationColor::Blue);
    state.annotations_mut().set_filled(true);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().drag(
        Pointer::Mouse,
        PagePoint::new(200.0, 100.0),
        false,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(150.0, 200.0),
        false,
    );

    state.annotations_mut().set_tool(Tool::Highlighter);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(300.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(500.0, 100.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Navigate);

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

#[test]
fn custom_shape_alpha_is_not_multiplied_twice_by_the_canvas() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Rectangle);
    state
        .annotations_mut()
        .set_color(AnnotationColor::Argb(0x8000_0000));
    state.annotations_mut().set_filled(true);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(200.0, 200.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Navigate);
    let pixels = render_document(&state, 1, 1.0);
    let offset = (150 * 600 + 150) * 4;
    for (actual, background) in
        pixels[offset..offset + 3].iter().zip([240_u8, 230, 210])
    {
        let expected = u16::from(background) * 127 / 255;
        assert!(
            u16::from(*actual).abs_diff(expected) <= 1,
            "alpha should apply once: {actual}, expected {expected}"
        );
    }
}

#[test]
fn rotated_rectangle_renders_around_its_frame_center() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Rectangle);
    let page = page_size(&state);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(100.0, 100.0),
        page,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(300.0, 200.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Select);
    let handle = state.annotations().shapes()[0]
        .rotation_handle(1.0)
        .unwrap();
    let page = page_size(&state);
    state.annotations_mut().begin_interaction(
        Pointer::Mouse,
        handle,
        page,
        1.0,
        DragModifiers::default(),
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(282.0, 150.0),
        false,
    );
    state.annotations_mut().set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 600 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert!(
        pixel(255, 150)[0] < 64,
        "rotated edge must be visible: {:?}",
        pixel(255, 150),
    );
    assert_eq!(
        pixel(150, 124),
        [240, 230, 210, 255],
        "unrotated edge must disappear",
    );
}

#[test]
fn group_membership_changes_are_visually_distinct() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Rectangle);
    assert!(state.annotations_mut().set_color(AnnotationColor::White));
    for x in [100.0, 400.0, 250.0] {
        let page = page_size(&state);
        assert!(state.annotations_mut().begin(
            Pointer::Mouse,
            PagePoint::new(x, 100.0),
            page,
        ));
        assert!(state.annotations_mut().finish(
            Pointer::Mouse,
            PagePoint::new(x + 50.0, 150.0),
            false,
        ));
    }
    state.annotations_mut().set_tool(Tool::Select);
    let ctrl = DragModifiers {
        square: true,
        ..DragModifiers::default()
    };
    for point in [
        PagePoint::new(275.0, 125.0),
        PagePoint::new(125.0, 125.0),
        PagePoint::new(425.0, 125.0),
    ] {
        let page = page_size(&state);
        assert!(state.annotations_mut().begin_interaction(
            Pointer::Mouse,
            point,
            page,
            1.0,
            ctrl,
        ));
        assert!(state.annotations_mut().finish(Pointer::Mouse, point, true));
    }
    assert_eq!(state.annotations().selected_indices(), [0, 1]);
    let without_middle = render_document(&state, 1, 1.0);

    let page = page_size(&state);
    assert!(state.annotations_mut().begin_interaction(
        Pointer::Mouse,
        PagePoint::new(275.0, 125.0),
        page,
        1.0,
        ctrl,
    ));
    assert!(state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(275.0, 125.0),
        true,
    ));
    assert_eq!(state.annotations().selected_indices(), [0, 1, 2]);
    let with_middle = render_document(&state, 1, 1.0);

    assert!(
        without_middle != with_middle,
        "toggling an interior group member must change selection rendering",
    );
}

#[test]
fn accepted_text_renders_over_the_page() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Text);
    let _ = update(
        &mut state,
        Message::DrawPressed(Pointer::Mouse, PagePoint::new(300.0, 300.0), 1.0),
    );
    paste_text(&mut state, "FastFill");
    let _ = update(&mut state, Message::AcceptText);
    state.annotations_mut().set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let dark = (280..=320)
        .any(|y| (250..=350).any(|x| pixels[(y * 600 + x) * 4] < 128));
    assert!(dark, "accepted text must be visible near its anchor");
}

#[test]
fn text_and_shapes_follow_shared_layer_order() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Text);
    let point = PagePoint::new(250.0, 250.0);
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "Layer");
    let _ = update(&mut state, Message::AcceptText);
    let text = state.annotations().texts()[0].clone();
    state.annotations_mut().set_tool(Tool::Navigate);

    let text_only = render_document(&state, 1, 1.0);
    let pixels = text_only.as_chunks::<4>().0;
    let glyphs: Vec<_> = pixel_indices_for_page_rect(&state, text.bounds)
        .into_iter()
        .filter(|index| dark_pixel(pixels[*index]))
        .collect();
    assert!(!glyphs.is_empty(), "test text must produce dark pixels");

    state.annotations_mut().set_tool(Tool::Rectangle);
    assert!(state.annotations_mut().set_color(AnnotationColor::Yellow));
    assert!(state.annotations_mut().set_filled(true));
    let margin = 10.0;
    let start = PagePoint::new(text.bounds.x - margin, text.bounds.y - margin);
    let end = PagePoint::new(
        text.bounds.x + text.bounds.width + margin,
        text.bounds.y + text.bounds.height + margin,
    );
    let page = page_size(&state);
    assert!(state.annotations_mut().begin(Pointer::Mouse, start, page));
    assert!(state.annotations_mut().finish(Pointer::Mouse, end, false));
    state.annotations_mut().set_tool(Tool::Navigate);
    let covered = render_document(&state, 1, 1.0);
    assert_eq!(dark_pixels_at(&covered, &glyphs), 0);

    state.annotations_mut().set_tool(Tool::Text);
    let page = page_size(&state);
    assert!(state.annotations_mut().begin_text_edit(
        text.bounds.center(),
        page,
        1.0,
    ));
    assert!(state.annotations_mut().accept_text());
    state.annotations_mut().set_tool(Tool::Select);
    assert!(state.annotations_mut().bring_selected_to_front());
    state.annotations_mut().set_tool(Tool::Navigate);
    let raised = render_document(&state, 1, 1.0);
    assert!(dark_pixels_at(&raised, &glyphs) > glyphs.len() / 2);

    assert!(state.annotations_mut().send_selected_to_back());
    let lowered = render_document(&state, 1, 1.0);
    assert_eq!(dark_pixels_at(&lowered, &glyphs), 0);
}

#[test]
fn formatted_text_color_and_rotation_reach_canvas_rendering() {
    let mut state = rendered_page();
    state.annotations_mut().set_tool(Tool::Text);
    let point = PagePoint::new(250.0, 250.0);
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "Rotate");
    let _ = update(&mut state, Message::SetTextSize(40.0));
    let _ = update(&mut state, Message::ToggleTextBold);
    let _ = update(&mut state, Message::ToggleTextItalic);
    let _ = update(&mut state, Message::ToggleTextUnderline);
    let _ = update(&mut state, Message::SetColor(AnnotationColor::Blue));
    let _ = update(&mut state, Message::AcceptText);

    state.annotations_mut().set_tool(Tool::Select);
    let text = state.annotations().texts()[0].clone();
    let handle = text.rotation_handle(1.0).unwrap();
    let center = text.bounds.center();
    let page = page_size(&state);
    assert!(state.annotations_mut().begin_text_move(
        Pointer::Mouse,
        handle,
        page,
        1.0,
    ));
    let end = PagePoint::new(center.x + 100.0, center.y);
    assert!(state.annotations_mut().finish(Pointer::Mouse, end, false));
    state.annotations_mut().set_tool(Tool::Navigate);

    let pixels = render_document(&state, 1, 1.0);
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[0] >= 80 || pixel[2] <= 120 {
            continue;
        }
        let point = (index % 600, index / 600);
        bounds = Some(match bounds {
            None => (point.0, point.1, point.0, point.1),
            Some((left, top, right, bottom)) => (
                left.min(point.0),
                top.min(point.1),
                right.max(point.0),
                bottom.max(point.1),
            ),
        });
    }
    let (left, top, right, bottom) = bounds.expect("blue text must render");
    assert!(bottom - top > right - left, "quarter-turn must be vertical");
}

#[test]
fn inline_editor_and_accepted_text_share_glyph_bounds() {
    for content in ["hello", "First\nSecond"] {
        let mut state = rendered_page();
        state.annotations_mut().set_tool(Tool::Text);
        let view = state
            .navigation
            .view(page_size(&state), page_viewport(CANVAS.size()))
            .unwrap();
        let scale = view.page_rect().width / page_size(&state).width;
        let _ = update(
            &mut state,
            Message::DrawPressed(
                Pointer::Mouse,
                PagePoint::new(300.0, 300.0),
                scale,
            ),
        );
        paste_text(&mut state, content);
        let editing = dark_pixel_bounds(&render_document(&state, 1, 1.0));

        let _ = update(&mut state, Message::AcceptText);
        let accepted = dark_pixel_bounds(&render_document(&state, 1, 1.0));

        assert_eq!(editing, accepted, "text layout changed for {content:?}");
    }
}

pub(super) fn paste_text(state: &mut FastFill, text: &str) {
    let action = text_editor::Action::Edit(text_editor::Edit::Paste(
        std::sync::Arc::new(text.to_owned()),
    ));
    let _ = update(state, Message::TextAction(action));
}

fn dark_pixel_bounds(pixels: &[u8]) -> Option<(usize, usize, usize, usize)> {
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[0] >= 40 || pixel[1] >= 40 || pixel[2] >= 40 {
            continue;
        }
        let point = (index % 600, index / 600);
        bounds = Some(match bounds {
            None => (point.0, point.1, point.0, point.1),
            Some((left, top, right, bottom)) => (
                left.min(point.0),
                top.min(point.1),
                right.max(point.0),
                bottom.max(point.1),
            ),
        });
    }
    bounds
}

fn dark_pixel(pixel: [u8; 4]) -> bool {
    pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100
}

fn dark_pixels_at(pixels: &[u8], indices: &[usize]) -> usize {
    let pixels = pixels.as_chunks::<4>().0;
    indices
        .iter()
        .filter(|index| dark_pixel(pixels[**index]))
        .count()
}

fn pixel_indices_for_page_rect(
    state: &FastFill,
    bounds: fastfill_rust::geometry::Rect,
) -> Vec<usize> {
    let view = state
        .navigation
        .view(
            page_size(state),
            page_viewport(iced::Size::new(600.0, 600.0)),
        )
        .unwrap();
    let start = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
    let end = view.page_to_view(PagePoint::new(
        bounds.x + bounds.width,
        bounds.y + bounds.height,
    ));
    let left = start.x.floor().max(0.0);
    let top = start.y.floor().max(0.0);
    let right = end.x.ceil().min(600.0);
    let bottom = end.y.ceil().min(600.0);
    (0..600 * 600)
        .filter(|index| {
            let x = f32::from(u16::try_from(index % 600).unwrap());
            let y = f32::from(u16::try_from(index / 600).unwrap());
            x >= left && x < right && y >= top && y < bottom
        })
        .collect()
}

fn rendered_page() -> FastFill {
    let mut state = FastFill::default();
    // Renderer fixtures use one page unit per viewport pixel for exact checks.
    state.pages.current_mut().size = PageSize::new(600.0, 600.0);
    state.pages.current_mut().image = Some(ImportedImage {
        handle: Handle::from_rgba(
            600,
            600,
            [240, 230, 210, 255].repeat(600 * 600),
        ),
        name: "paper.png".into(),
        display: None,
        size: PageSize::new(600.0, 600.0),
        info: fastfill_rust::image_input::ImageInfo {
            width: 600,
            height: 600,
            format: fastfill_rust::image_input::ImageFormatKind::Png,
        },
        directory: None,
    });
    state
}

fn render_document(state: &FastFill, scale: u32, dpi: f32) -> Vec<u8> {
    render_element(document(state), iced::Size::new(600, 600), scale, dpi)
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn render_element(
    mut element: iced::Element<'_, Message>,
    size: iced::Size<u32>,
    scale: u32,
    dpi: f32,
) -> Vec<u8> {
    use iced::advanced::Renderer as _;
    use iced::advanced::renderer::{Headless, Style};
    use iced::advanced::{Layout, layout, widget};

    let mut renderer =
        iced::futures::executor::block_on(<Renderer as Headless>::new(
            // Headless containers need the bundled font, not system fonts.
            iced::Font::with_name("Fira Sans"),
            iced::Pixels(16.0),
            Some("tiny-skia"),
        ))
        .expect("software renderer needs no display or GPU");
    let mut tree = widget::Tree::new(element.as_widget());
    // These test viewports are small enough to fit exactly in f32.
    let logical = iced::Size::new(size.width as f32, size.height as f32);
    renderer.reset(Rectangle::new(Point::ORIGIN, logical));
    let node = element.as_widget_mut().layout(
        &mut tree,
        &renderer,
        &layout::Limits::new(logical, logical),
    );
    // Iced initializes button status on RedrawRequested, before drawing.
    element.as_widget_mut().update(
        &mut tree,
        &iced::Event::Window(iced::window::Event::RedrawRequested(
            iced::time::Instant::now(),
        )),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut iced::advanced::clipboard::Null,
        &mut iced::advanced::Shell::new(&mut Vec::new()),
        &Rectangle::new(Point::ORIGIN, logical),
    );
    element.as_widget().draw(
        &tree,
        &mut renderer,
        &Theme::Light,
        &Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::new(Point::ORIGIN, logical),
    );
    renderer.screenshot(
        iced::Size::new(size.width * scale, size.height * scale),
        dpi,
        Color::WHITE,
    )
}

#[test]
fn main_window_keeps_the_page_rail_separate_from_the_canvas() {
    let state = rendered_page();
    let size = iced::Size::new(1_200, 800);
    let pixels = render_element(view(&state), size, 1, 1.0);
    let pixel = |x: usize, y: usize| {
        let offset = (y * 1_200 + x) * 4;
        &pixels[offset..offset + 4]
    };
    assert_eq!(pixel(180, 780), [67, 72, 78, 255]);
    assert_eq!(pixel(680, 500), [240, 230, 210, 255]);
    assert_ne!(pixel(80, 500), [67, 72, 78, 255]);

    if let Some(path) = std::env::var_os("FASTFILL_UI_SNAPSHOT") {
        image::save_buffer(
            path,
            &pixels,
            size.width,
            size.height,
            image::ColorType::Rgba8,
        )
        .expect("UI snapshot");
    }
}

#[test]
fn zoomed_panned_page_cannot_paint_over_toolbars_or_the_rail() {
    let mut state = rendered_page();
    let viewport = PageRect::new(0.0, 0.0, 1_036.0, 568.0);
    state.navigation.zoom_at(
        4.0,
        viewport.center(),
        page_size(&state),
        viewport,
    );
    state.navigation.pan_by(PagePoint::new(-300.0, -500.0));
    for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
        let pixels = render_element(
            view(&state),
            iced::Size::new(1_200, 800),
            scale,
            dpi,
        );
        let pixels = pixels.as_chunks::<4>().0;
        let width = 1_200 * usize::try_from(scale).unwrap();
        let scale = usize::try_from(scale).unwrap();
        let page_color = [240, 230, 210, 255];
        assert!(
            pixels[..200 * scale * width]
                .iter()
                .all(|pixel| *pixel != page_color),
            "page image must not cover the toolbar at {dpi} DPI scale"
        );
        for y in 400 * scale..780 * scale {
            for x in 160 * scale..164 * scale {
                assert_ne!(
                    pixels[y * width + x],
                    page_color,
                    "page image must not cover the rail border"
                );
            }
        }
    }
}

#[test]
fn scrolled_thumbnails_cannot_paint_over_page_controls() {
    // Keep concurrent font initialization out of the pixel comparison.
    let _ = crate::appearance::text_font(TextFormat::default());
    let mut state = rendered_page();
    let image = state.image().unwrap().clone();
    assert!(state.pages.import_images(&vec![image; 6]));
    for (scale, dpi) in [(1, 1.0), (2, 2.0)] {
        state.page_scroll = 0.0;
        let before = render_element(
            super::rail::view(&state),
            iced::Size::new(164, 600),
            scale,
            dpi,
        );
        state.page_scroll = 80.0;
        let after = render_element(
            super::rail::view(&state),
            iced::Size::new(164, 600),
            scale,
            dpi,
        );
        let scale = usize::try_from(scale).unwrap();
        let control_bytes = 164 * 260 * scale * scale * 4;
        assert!(
            before[..control_bytes] == after[..control_bytes],
            "scrolling must not change the page controls at {dpi} DPI scale"
        );
    }
}

pub(super) fn annotated_page() -> FastFill {
    let mut state = FastFill::default();
    state.annotations_mut().set_tool(Tool::Rectangle);
    state.annotations_mut().begin(
        Pointer::Mouse,
        PagePoint::new(10.0, 20.0),
        DEFAULT_PAGE,
    );
    state.annotations_mut().finish(
        Pointer::Mouse,
        PagePoint::new(50.0, 70.0),
        false,
    );
    state
}

#[test]
fn canceling_page_replacement_keeps_annotations() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::AskReplacePage);
    assert_eq!(state.image_picker, ImagePicker::ConfirmReplace);

    let _ = update(&mut state, Message::KeepPage);
    assert_eq!(state.image_picker, ImagePicker::Idle);
    assert_eq!(state.annotations().shapes().len(), 1);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().shapes(), []);
}

#[test]
fn canceled_or_failed_picker_keeps_annotations() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::AskReplacePage);
    let _ = update(&mut state, Message::ConfirmReplacePage);
    assert_eq!(
        state.image_picker,
        ImagePicker::Picking(PickPurpose::Replace)
    );
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Replace, Ok(None)),
    );
    assert_eq!(state.image_picker, ImagePicker::Idle);
    assert_eq!(state.annotations().shapes().len(), 1);

    let _ = update(&mut state, Message::AskReplacePage);
    let _ = update(&mut state, Message::ConfirmReplacePage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Replace, Err("Corrupt image".into())),
    );
    assert!(state.status_is_error);
    assert_eq!(state.annotations().shapes().len(), 1);
}

#[test]
fn successful_import_resets_annotations_and_retains_tool() {
    let mut state = annotated_page();
    let image = ImportedImage {
        handle: Handle::from_rgba(1, 1, vec![255; 4]),
        display: None,
        name: "replacement.png".into(),
        size: PageSize::new(1.0, 1.0),
        info: fastfill_rust::image_input::ImageInfo {
            width: 1,
            height: 1,
            format: fastfill_rust::image_input::ImageFormatKind::Png,
        },
        directory: None,
    };
    let _ = update(&mut state, Message::AskReplacePage);
    let _ = update(&mut state, Message::ConfirmReplacePage);
    let _ = update(
        &mut state,
        Message::ImagePicked(PickPurpose::Replace, Ok(Some(image))),
    );

    assert_eq!(state.annotations().shapes(), []);
    assert_eq!(state.annotations().tool(), Tool::Rectangle);
    assert!(!state.annotations().can_undo());
    assert!(!state.annotations().can_redo());
    assert_eq!(state.image().unwrap().name, "replacement.png");
}

#[test]
fn delete_all_requires_confirmation_and_remains_undoable() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::AskDeleteAll);
    assert!(state.confirm_delete_all);
    assert_eq!(state.annotations().shapes().len(), 1);

    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().shapes().len(), 1);
    let _ = update(&mut state, Message::KeepAll);
    assert!(!state.confirm_delete_all);

    let _ = update(&mut state, Message::AskDeleteAll);
    let _ = update(&mut state, Message::ConfirmDeleteAll);
    assert!(!state.confirm_delete_all);
    assert_eq!(state.annotations().shapes(), []);
    let _ = update(&mut state, Message::Undo);
    assert_eq!(state.annotations().shapes().len(), 1);
}

#[test]
fn escape_closes_delete_all_confirmation_without_deleting() {
    let mut state = annotated_page();
    let _ = update(&mut state, Message::AskDeleteAll);
    let _ = update(&mut state, Message::CancelInteraction);

    assert!(!state.confirm_delete_all);
    assert_eq!(state.annotations().shapes().len(), 1);
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
        state.annotations_mut().begin(
            Pointer::Mouse,
            PagePoint::new(100.0, 100.0),
            DEFAULT_PAGE,
        );
        let _ = update(&mut state, message);
        assert!(state.annotations().active_pointer().is_none());
        assert_eq!(state.annotations().shapes().len(), 1);
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
        image: state.image(),
        navigation: state.navigation,
        page_size: page_size(state),
        annotations: state.annotations(),
        input_epoch: state.input_epoch,
        input_enabled: state.image_picker == ImagePicker::Idle
            && !state.confirm_delete_all,
        modifiers: state.modifiers,
        layer: crate::page_canvas::CanvasLayer::Overlay,
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
    let preview = state.annotations().preview().unwrap().bounds();
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
    let rectangle = state.annotations().shapes()[0].bounds();
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
    assert_eq!(state.annotations().shapes(), []);
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
        assert_eq!(state.annotations().shapes(), []);
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
            let preview = state.annotations().preview();
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
            assert_eq!(state.annotations().preview(), preview);
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
    assert!(state.annotations().shapes()[0].bounds().x < 0.0);
    assert!(state.annotations().active_pointer().is_none());
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
    assert_eq!(
        state.annotations().active_pointer(),
        Some(Pointer::Touch(1))
    );
    assert_eq!(state.annotations().shapes(), []);
    canvas_event(
        &mut state,
        &mut canvas_state,
        &canvas::Event::Touch(touch::Event::FingerLost {
            id: first,
            position: end,
        }),
        end,
    );
    assert!(state.annotations().active_pointer().is_none());
    assert_eq!(state.annotations().shapes(), []);

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
    assert_eq!(state.annotations().shapes().len(), 1);
    assert_eq!(state.navigation, navigation);
}
