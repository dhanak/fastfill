use super::controller::update;
use super::tests::paste_text;
use super::*;

#[test]
fn first_text_tool_click_places_the_caret_under_the_pointer() {
    for scale in [0.5, 1.0, 3.0] {
        let mut state = FastFill::default();
        let start = PagePoint::new(200.0, 200.0);
        state.annotations_mut().set_tool(Tool::Text);
        state
            .annotations_mut()
            .begin_text(start, DEFAULT_PAGE, scale);
        let format = state.annotations().current_text_format().unwrap();
        let content = "First line\nárvíztűrő 👋🏽 text";
        let size = text::measure_text(content, format, scale);
        state.annotations_mut().update_text(content, size);
        state.annotations_mut().accept_text();
        let text = state.annotations().texts().next().unwrap().clone();
        let point = PagePoint::new(
            text.bounds.x + 82.0,
            text.bounds.y + format.font_size * 1.3 * 1.5,
        );
        let _ = update(
            &mut state,
            Message::DrawPressed(Pointer::Mouse, point, scale),
        );
        let cursor = state.text_content.cursor();
        assert_eq!(cursor.position.line, 1);
        assert!(cursor.position.column > 0);
        assert!(cursor.position.column < "árvíztűrő 👋🏽 text".len());
        assert!(cursor.selection.is_none());
        assert!("árvíztűrő 👋🏽 text".is_char_boundary(cursor.position.column));
        assert_eq!(
            state.annotations().texts().cloned().collect::<Vec<_>>(),
            [text]
        );
    }
}

#[test]
fn idle_escape_clears_selection_without_editing_the_last_annotation() {
    for tool in [Tool::Rectangle, Tool::Arrow, Tool::Checkmark, Tool::Text] {
        let mut state = FastFill::default();
        let point = PagePoint::new(200.0, 200.0);
        let _ = update(&mut state, Message::SetTool(tool));
        let _ = update(
            &mut state,
            Message::DrawPressed(Pointer::Mouse, point, 1.0),
        );
        if tool == Tool::Text {
            paste_text(&mut state, "Keep me");
            let _ = update(&mut state, Message::AcceptText);
        } else {
            let _ = update(
                &mut state,
                Message::DrawReleased(
                    Pointer::Mouse,
                    PagePoint::new(260.0, 240.0),
                    false,
                ),
            );
        }
        let identity = state.pages.identity();
        let shapes = state.annotations().shapes().cloned().collect::<Vec<_>>();
        let texts = state.annotations().texts().cloned().collect::<Vec<_>>();
        assert!(state.annotations().has_selection());
        let _ = update(&mut state, Message::ViewportResized);
        assert!(state.annotations().has_selection());
        let _ = update(&mut state, Message::CancelInteraction);
        assert!(!state.annotations().has_selection());
        assert_eq!(state.pages.identity(), identity);
        assert_eq!(state.annotations().tool(), tool);
        let _ = update(&mut state, Message::SetColor(AnnotationColor::Red));
        assert_eq!(
            state.annotations().shapes().cloned().collect::<Vec<_>>(),
            shapes
        );
        assert_eq!(
            state.annotations().texts().cloned().collect::<Vec<_>>(),
            texts
        );
        assert_eq!(state.pages.identity(), identity);
        let _ = update(&mut state, Message::Undo);
        assert!(state.annotations().is_empty());
        let _ = update(&mut state, Message::Redo);
        assert_eq!(
            state.annotations().shapes().cloned().collect::<Vec<_>>(),
            shapes
        );
        assert_eq!(
            state.annotations().texts().cloned().collect::<Vec<_>>(),
            texts
        );
    }
}

#[test]
fn changing_tools_clears_selection_before_changing_the_next_color() {
    for tool in [Tool::Arrow, Tool::Text] {
        let mut state = FastFill::default();
        let _ = update(&mut state, Message::SetTool(tool));
        let _ = update(
            &mut state,
            Message::DrawPressed(
                Pointer::Mouse,
                PagePoint::new(200.0, 200.0),
                1.0,
            ),
        );
        if tool == Tool::Text {
            paste_text(&mut state, "Keep me");
            let _ = update(&mut state, Message::AcceptText);
        } else {
            let _ = update(
                &mut state,
                Message::DrawReleased(
                    Pointer::Mouse,
                    PagePoint::new(260.0, 240.0),
                    false,
                ),
            );
        }
        assert!(state.annotations().has_selection());
        let shapes = state.annotations().shapes().cloned().collect::<Vec<_>>();
        let texts = state.annotations().texts().cloned().collect::<Vec<_>>();
        let identity = state.pages.identity();

        let _ = update(&mut state, Message::SetTool(Tool::Rectangle));
        assert!(!state.annotations().has_selection());
        assert_eq!(state.pages.identity(), identity);
        let _ = update(&mut state, Message::SetColor(AnnotationColor::Red));
        assert_eq!(
            state.annotations().shapes().cloned().collect::<Vec<_>>(),
            shapes
        );
        assert_eq!(
            state.annotations().texts().cloned().collect::<Vec<_>>(),
            texts
        );
        assert_eq!(state.pages.identity(), identity);
        assert_eq!(
            state.annotations().current_style().unwrap().color,
            AnnotationColor::Red
        );
        let _ = update(
            &mut state,
            Message::DrawPressed(
                Pointer::Mouse,
                PagePoint::new(300.0, 300.0),
                1.0,
            ),
        );
        let _ = update(
            &mut state,
            Message::DrawReleased(
                Pointer::Mouse,
                PagePoint::new(360.0, 340.0),
                false,
            ),
        );
        assert_eq!(
            state.annotations().shapes().last().unwrap().color,
            AnnotationColor::Red
        );
    }
}

#[test]
fn escape_reverts_a_text_edit_before_clearing_its_selection() {
    let mut state = FastFill::default();
    let point = PagePoint::new(200.0, 200.0);
    let _ = update(&mut state, Message::SetTool(Tool::Text));
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "Original");
    let _ = update(&mut state, Message::AcceptText);
    let original = state.annotations().texts().next().unwrap().clone();
    let identity = state.pages.identity();
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    paste_text(&mut state, "Changed");
    let _ = update(&mut state, Message::ToggleTextBold);
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.annotations().has_text_edit());
    assert!(state.annotations().has_selection());
    assert_eq!(
        state.annotations().texts().cloned().collect::<Vec<_>>(),
        [original]
    );
    assert_eq!(state.pages.identity(), identity);
    let _ = update(&mut state, Message::CancelInteraction);
    assert!(!state.annotations().has_selection());
    assert_eq!(state.pages.identity(), identity);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn tiny_selection_renders_only_its_frame_at_both_dpi_scales() {
    use super::tests::render_element;
    use crate::page_canvas::CanvasLayer;

    let size = iced::Size::new(272, 272);
    let page_size = PageSize::new(200.0, 200.0);
    let view = Navigation::default()
        .view(page_size, PageRect::new(36.0, 36.0, 200.0, 200.0))
        .unwrap();
    for tool in [Tool::Cross, Tool::Checkmark, Tool::Arrow] {
        let mut annotations = Annotations::default();
        annotations.set_tool(tool);
        annotations.begin(
            Pointer::Mouse,
            PagePoint::new(100.0, 100.0),
            page_size,
        );
        annotations.finish(Pointer::Mouse, PagePoint::new(110.0, 110.0), false);
        let bounds = annotations.shapes().next().unwrap().bounds();
        let top_left = view.page_to_view(PagePoint::new(bounds.x, bounds.y));
        for dpi in [1, 2] {
            let page = PageCanvas {
                image: None,
                navigation: Navigation::default(),
                page_size,
                annotations: &annotations,
                input_epoch: 0,
                input_enabled: true,
                modifiers: DragModifiers::default(),
                layer: CanvasLayer::Overlay,
            };
            let pixels = render_element(
                canvas(page).width(272).height(272).into(),
                size,
                dpi,
                dpi as f32,
            );
            let mut count = 0;
            for (i, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
                if *pixel == [255; 4] {
                    continue;
                }
                count += 1;
                let width = 272 * dpi as usize;
                let x = (i % width) as f32 / dpi as f32;
                let y = (i / width) as f32 / dpi as f32;
                assert!(
                    x >= top_left.x - 2.0
                        && x <= top_left.x + bounds.width + 2.0
                        && y >= top_left.y - 2.0
                        && y <= top_left.y + bounds.height + 2.0,
                    "hidden handles must not paint outside the frame"
                );
            }
            assert!(count > 0, "selection frame must remain visible");
        }
    }
}
