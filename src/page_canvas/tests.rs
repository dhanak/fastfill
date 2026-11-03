use fastfill::annotations::arrowhead_points;
use fastfill::geometry::Point as PagePoint;
use iced::{Point, Rectangle};

#[test]
fn select_cursor_distinguishes_marquees_from_selected_object_edits() {
    use super::{CanvasLayer, CanvasState, PageCanvas, page_viewport};
    use fastfill::annotations::{Annotations, DragModifiers, Pointer, Tool};
    use fastfill::geometry::{Navigation, Size};
    use iced::mouse;
    use iced::widget::canvas;

    let page_size = Size::new(800.0, 1100.0);
    let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(800.0, 800.0));
    let view = Navigation::default()
        .view(page_size, page_viewport(bounds.size()))
        .unwrap();
    let scale = view.page_rect().width / page_size.width;
    let plain = DragModifiers::default();
    let ctrl = DragModifiers {
        square: true,
        ..plain
    };
    let cursor = |annotations: &Annotations, modifiers, point| {
        let page = PageCanvas {
            image: None,
            navigation: Navigation::default(),
            page_size,
            annotations,
            input_epoch: 0,
            input_enabled: true,
            modifiers,
            layer: CanvasLayer::Overlay,
        };
        let point = view.page_to_view(point);
        canvas::Program::mouse_interaction(
            &page,
            &CanvasState::default(),
            bounds,
            mouse::Cursor::Available(Point::new(point.x, point.y)),
        )
    };
    let mut annotations = Annotations::default();
    let mouse = Pointer::Mouse;
    let corner = PagePoint::new(100.0, 100.0);
    annotations.set_tool(Tool::Rectangle);
    annotations.begin(mouse, corner, page_size);
    annotations.finish(mouse, PagePoint::new(250.0, 200.0), false);
    annotations.set_tool(Tool::Select);
    assert_eq!(
        cursor(&annotations, plain, corner),
        mouse::Interaction::ResizingDiagonallyDown
    );
    assert_eq!(
        cursor(&annotations, ctrl, corner),
        mouse::Interaction::Crosshair
    );
    annotations.begin_interaction(mouse, corner, page_size, scale, ctrl);
    assert_eq!(
        cursor(&annotations, plain, corner),
        mouse::Interaction::Crosshair
    );
    annotations.cancel();

    annotations.set_tool(Tool::Text);
    annotations.begin_text(PagePoint::new(300.0, 300.0), page_size, scale);
    annotations.update_text("Text", Size::new(80.0, 30.0));
    annotations.accept_text();
    annotations.set_tool(Tool::Select);
    let empty = PagePoint::new(700.0, 900.0);
    annotations.begin_interaction(mouse, empty, page_size, scale, plain);
    annotations.finish(mouse, empty, false);
    let text = annotations.texts().next().unwrap().bounds.center();
    assert_eq!(
        cursor(&annotations, plain, text),
        mouse::Interaction::Crosshair
    );
    annotations.begin_interaction(mouse, text, page_size, scale, plain);
    annotations.finish(mouse, text, false);
    assert_eq!(cursor(&annotations, plain, text), mouse::Interaction::Move);
    let member = PagePoint::new(175.0, 150.0);
    annotations.begin_interaction(mouse, member, page_size, scale, ctrl);
    annotations.finish(mouse, member, true);
    assert_eq!(annotations.selected_count(), 2);
    assert_eq!(
        cursor(&annotations, plain, member),
        mouse::Interaction::Move
    );
    assert_eq!(
        cursor(&annotations, plain, corner),
        mouse::Interaction::Move
    );
    assert_eq!(
        cursor(&annotations, ctrl, member),
        mouse::Interaction::Crosshair
    );
    annotations.begin_interaction(mouse, member, page_size, scale, plain);
    let moved = PagePoint::new(500.0, 500.0);
    annotations.drag(mouse, moved, false);
    assert_eq!(cursor(&annotations, plain, moved), mouse::Interaction::Move);
    assert_eq!(cursor(&annotations, ctrl, moved), mouse::Interaction::Move);
    annotations.cancel();
}

#[test]
fn arrowhead_dimensions_do_not_depend_on_direction_or_box_aspect() {
    let end = PagePoint::new(200.0, 200.0);
    let horizontal = arrowhead_points(PagePoint::new(0.0, 200.0), end).unwrap();
    let diagonal_offset = 200.0 / 2.0_f32.sqrt();
    let diagonal = arrowhead_points(
        PagePoint::new(end.x - diagonal_offset, end.y - diagonal_offset),
        end,
    )
    .unwrap();
    for (horizontal, diagonal) in [horizontal.0, horizontal.1]
        .into_iter()
        .zip([diagonal.0, diagonal.1])
    {
        let horizontal = (horizontal.x - end.x).hypot(horizontal.y - end.y);
        let diagonal = (diagonal.x - end.x).hypot(diagonal.y - end.y);
        assert!((horizontal - diagonal).abs() < 0.001);
    }
}
