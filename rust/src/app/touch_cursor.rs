//! Use touch coordinates for child widgets that rely on the mouse cursor.

use super::Message;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::advanced::{layout, overlay, renderer};
use iced::time::{Duration, Instant};
use iced::{Element, Event, Length, Rectangle, Size, Vector};
use iced::{Renderer, Theme, mouse, touch};

#[cfg(not(target_arch = "wasm32"))]
use std::fmt;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::OpenOptions;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

pub(super) fn wrap(content: Element<'_, Message>) -> Element<'_, Message> {
    Element::new(TouchCursor { content })
}

struct TouchCursor<'a> {
    content: Element<'a, Message>,
}

#[derive(Default)]
struct TouchState {
    last_touch: Option<Instant>,
    gesture: Option<Gesture>,
}

struct Gesture {
    finger: touch::Finger,
    start: iced::Point,
    dragging: bool,
}

const MOUSE_ECHO_WINDOW: Duration = Duration::from_millis(250);
const SCROLL_SLOP: f32 = 8.0;

fn recent_touch(last_touch: Option<Instant>) -> bool {
    last_touch.is_some_and(|at| at.elapsed() < MOUSE_ECHO_WINDOW)
}

fn route_touch(
    state: &mut TouchState,
    event: &Event,
    touch_event: touch::Event,
    forward: &mut impl FnMut(&Event, mouse::Cursor),
) {
    state.last_touch = Some(Instant::now());
    match touch_event {
        touch::Event::FingerPressed { id, position } => {
            state.gesture = Some(Gesture {
                finger: id,
                start: position,
                dragging: false,
            });
            trace_input(event, "sidebar", format_args!("deferred"));
        }
        touch::Event::FingerMoved { id, position } => {
            let Some(gesture) = state.gesture.as_mut() else {
                return;
            };
            if gesture.finger != id {
                return;
            }
            if !gesture.dragging
                && (position.x - gesture.start.x)
                    .hypot(position.y - gesture.start.y)
                    > SCROLL_SLOP
            {
                forward(
                    &Event::Touch(touch::Event::FingerPressed {
                        id,
                        position: gesture.start,
                    }),
                    mouse::Cursor::Available(gesture.start),
                );
                gesture.dragging = true;
            }
            if gesture.dragging {
                forward(event, mouse::Cursor::Available(position));
            } else {
                trace_input(event, "sidebar", format_args!("deferred"));
            }
        }
        touch::Event::FingerLifted { id, position }
        | touch::Event::FingerLost { id, position } => {
            let Some(gesture) = state.gesture.take() else {
                return;
            };
            if gesture.finger != id {
                state.gesture = Some(gesture);
                return;
            }
            if gesture.dragging {
                forward(event, mouse::Cursor::Available(position));
            } else if matches!(touch_event, touch::Event::FingerLifted { .. }) {
                // A tap follows the mouse path; drags keep native touch
                // scrolling and never activate a control.
                forward(
                    &Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    )),
                    mouse::Cursor::Available(gesture.start),
                );
                forward(
                    &Event::Mouse(mouse::Event::ButtonReleased(
                        mouse::Button::Left,
                    )),
                    mouse::Cursor::Available(position),
                );
            }
        }
    }
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
        let mut forward = |routed: &Event, routed_cursor| {
            let mut messages = Vec::new();
            let mut child_shell = Shell::new(&mut messages);
            self.content.as_widget_mut().update(
                &mut tree.children[0],
                routed,
                layout,
                routed_cursor,
                renderer,
                clipboard,
                &mut child_shell,
                viewport,
            );
            trace_input(
                event,
                "sidebar",
                format_args!(
                    "routed={routed:?} captured={}",
                    child_shell.is_event_captured()
                ),
            );
            shell.merge(child_shell, |message| message);
        };
        if let Event::Touch(touch_event) = event {
            route_touch(state, event, *touch_event, &mut forward);
            return;
        }
        // Windows can echo a touch as mouse input. Bound the suppression so
        // a missing lift event cannot disable the real mouse indefinitely.
        if matches!(event, Event::Mouse(_)) && recent_touch(state.last_touch) {
            trace_input(event, "sidebar", format_args!("suppressed"));
            return;
        }
        if matches!(event, Event::Mouse(_)) {
            state.last_touch = None;
        }
        let cursor = if matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        ) && recent_touch(state.last_touch)
        {
            mouse::Cursor::Unavailable
        } else {
            cursor
        };
        forward(event, cursor);
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

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn trace_input(
    event: &Event,
    source: &str,
    detail: fmt::Arguments<'_>,
) {
    if !matches!(event, Event::Touch(_) | Event::Mouse(_)) {
        return;
    }
    trace(format_args!("{source} {event:?} {detail}"));
}

#[cfg(target_arch = "wasm32")]
fn trace_input(_: &Event, _: &str, _: std::fmt::Arguments<'_>) {}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn trace(detail: fmt::Arguments<'_>) {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    static START: OnceLock<Instant> = OnceLock::new();
    let Some(path) = PATH
        .get_or_init(|| std::env::var_os("FASTFILL_TOUCH_LOG").map(Into::into))
    else {
        return;
    };
    if let Ok(mut file) =
        OpenOptions::new().create(true).append(true).open(path)
    {
        let elapsed = START.get_or_init(Instant::now).elapsed().as_millis();
        let _ = writeln!(file, "{elapsed:>8} ms {detail}");
    }
}

#[cfg(test)]
#[test]
fn missing_lift_cannot_suppress_mouse_forever() {
    let old_press = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    assert!(!recent_touch(Some(old_press)));
    assert!(!recent_touch(None));
}
