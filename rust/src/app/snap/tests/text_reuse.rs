use super::*;

#[test]
fn clicking_a_filled_rule_reopens_its_text_at_the_end_without_a_duplicate() {
    for scale in [0.25, 1.0, 4.0] {
        for pointer in [Pointer::Mouse, Pointer::Touch(9)] {
            let mut state = fixture(Tool::Text);
            send(&mut state, Event::Toggle);
            settle(&mut state);
            let _ = update(&mut state, Message::SetTextSize(22.0));
            let _ =
                update(&mut state, Message::SetColor(AnnotationColor::Green));
            click(&mut state, pointer, 450.0, 350.0, scale);
            paste_text(&mut state, "First\nárvíztűrő");
            let _ = update(&mut state, Message::AcceptText);
            let original = state.annotations().texts()[0].clone();
            let identity = state.pages.identity();
            let _ = update(&mut state, Message::CancelInteraction);

            // Click far past the short value, still on the same field.
            click(&mut state, pointer, 600.0, 350.0, scale);
            assert_eq!(
                state.annotations().text_preview(),
                Some(original.clone())
            );
            let cursor = state.text_content.cursor();
            assert_eq!(cursor.position.line, 1);
            assert_eq!(cursor.position.column, "árvíztűrő".len());
            assert!(cursor.selection.is_none());
            assert_eq!(state.pages.identity(), identity);
            assert_eq!(state.annotations().len(), 1);
            let _ = update(&mut state, Message::CancelInteraction);
            assert_eq!(
                state.annotations().texts(),
                std::slice::from_ref(&original)
            );
            assert_eq!(state.pages.identity(), identity);

            click(&mut state, pointer, 600.0, 350.0, scale);
            paste_text(&mut state, "!");
            let _ = update(&mut state, Message::AcceptText);
            let changed = state.annotations().texts()[0].clone();
            assert_eq!(state.annotations().len(), 1);
            assert_eq!(changed.id, original.id);
            assert_eq!(changed.text, "First\nárvíztűrő!");
            assert_eq!(changed.format, original.format);
            assert_eq!(changed.rotation, original.rotation);
            let _ = update(&mut state, Message::Undo);
            assert_eq!(state.annotations().texts(), [original]);
            let _ = update(&mut state, Message::Redo);
            assert_eq!(state.annotations().texts(), [changed]);
        }
    }
}

#[test]
fn direct_clicks_still_pick_the_caret_and_missing_features_never_reuse() {
    let mut state = fixture(Tool::Text);
    send(&mut state, Event::Toggle);
    settle(&mut state);
    click(&mut state, Pointer::Mouse, 450.0, 350.0, 1.0);
    paste_text(&mut state, "Existing field");
    let _ = update(&mut state, Message::AcceptText);
    let original = state.annotations().texts()[0].clone();
    let point = Point::new(
        original.bounds.x + 1.0,
        original.bounds.y + original.bounds.height / 2.0,
    );
    let _ =
        update(&mut state, Message::DrawPressed(Pointer::Mouse, point, 1.0));
    assert_eq!(state.annotations().text_preview().unwrap().id, original.id);
    assert_eq!(state.text_content.cursor().position.column, 0);
    let _ = update(&mut state, Message::CancelInteraction);

    send(&mut state, Event::Toggle);
    for pending in [false, true] {
        if pending {
            state.smart_snap.cache.clear();
            send(&mut state, Event::Toggle);
        }
        click(&mut state, Pointer::Mouse, 600.0, 350.0, 1.0);
        let draft = state.annotations().text_preview().unwrap();
        assert_ne!(draft.id, original.id);
        assert_eq!(draft.bounds.x, 600.0 * 612.0 / 800.0);
        if pending {
            settle(&mut state);
            assert_eq!(state.annotations().text_preview(), Some(draft));
        }
        let _ = update(&mut state, Message::CancelInteraction);
        assert_eq!(
            state.annotations().texts(),
            std::slice::from_ref(&original)
        );
    }
}
