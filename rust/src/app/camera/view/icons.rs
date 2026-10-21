//! Camera controls use the same Lucide set as the main editor.

use crate::icons::{self, Glyph};
use fastfill_rust::annotations::Tool;
use iced::Element;

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Cancel,
    Rotate,
    Mirror,
    Capture,
    Pause,
    Play,
    Retake,
    Settings,
    Switch,
    UsePhoto,
}

pub(super) fn view<Message: 'static>(
    icon: Icon,
    selected: bool,
    enabled: bool,
) -> Element<'static, Message> {
    let glyph = match icon {
        Icon::Cancel => Glyph::Tool(Tool::Cross),
        Icon::Rotate => Glyph::CameraRotate,
        Icon::Mirror => Glyph::CameraMirror,
        Icon::Capture => Glyph::Camera,
        Icon::Pause => Glyph::CameraPause,
        Icon::Play => Glyph::CameraPlay,
        Icon::Retake => Glyph::Undo,
        Icon::Settings => Glyph::CameraSettings,
        Icon::Switch => Glyph::CameraSwitch,
        Icon::UsePhoto => Glyph::Tool(Tool::Checkmark),
    };
    icons::view(glyph, selected, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Message, tests::render_element};

    #[test]
    fn camera_actions_are_visible_and_distinct() {
        let render = |icon| {
            render_element(
                view::<Message>(icon, false, true),
                iced::Size::new(24, 24),
                1,
                1.0,
            )
        };
        for icon in [
            Icon::Cancel,
            Icon::Rotate,
            Icon::Mirror,
            Icon::Capture,
            Icon::Pause,
            Icon::Play,
            Icon::Retake,
            Icon::Settings,
            Icon::Switch,
            Icon::UsePhoto,
        ] {
            let pixels = render(icon);
            assert!(
                pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[0] < 100)
            );
        }
        assert_ne!(render(Icon::Retake), render(Icon::Rotate));
        assert_ne!(render(Icon::Switch), render(Icon::Capture));
    }
}
