//! Use touch coordinates for child widgets that rely on the mouse cursor.

use super::Message;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::advanced::{layout, overlay, renderer};
use iced::time::{Duration, Instant};
use iced::{Element, Event, Length, Rectangle, Size, Vector};
use iced::{Renderer, Theme, mouse, touch};

pub(super) fn wrap(content: Element<'_, Message>) -> Element<'_, Message> {
    Element::new(TouchCursor { content })
}

struct TouchCursor<'a> {
    content: Element<'a, Message>,
}

#[derive(Default)]
struct TouchState {
    fingers: u8,
    last_touch: Option<Instant>,
    touch_mode: bool,
}

impl Widget<Message, Theme, Renderer> for TouchCursor<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<TouchState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(TouchState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            limits,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<TouchState>();
        // iced buttons test the mouse cursor on touch release. Windows can
        // also send compatibility mouse events for that same contact. Keep
        // those from pressing another button until the tap has ended.
        let cursor = match event {
            Event::Touch(touch_event) => {
                state.touch_mode = true;
                state.last_touch = Some(Instant::now());
                let position = match touch_event {
                    touch::Event::FingerPressed { position, .. } => {
                        state.fingers = state.fingers.saturating_add(1);
                        position
                    }
                    touch::Event::FingerMoved { position, .. } => position,
                    touch::Event::FingerLifted { position, .. }
                    | touch::Event::FingerLost { position, .. } => {
                        state.fingers = state.fingers.saturating_sub(1);
                        position
                    }
                };
                mouse::Cursor::Available(*position)
            }
            Event::Mouse(_)
                if state.touch_mode
                    && (state.fingers > 0
                        || state.last_touch.is_some_and(|at| {
                            at.elapsed() < Duration::from_millis(250)
                        })) =>
            {
                return;
            }
            Event::Mouse(_) => {
                state.touch_mode = false;
                cursor
            }
            Event::Window(iced::window::Event::RedrawRequested(_))
                if state.touch_mode =>
            {
                mouse::Cursor::Unavailable
            }
            _ => cursor,
        };
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
