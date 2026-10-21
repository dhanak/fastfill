//! Lucide SVG icons, pinned under assets/icons/lucide with upstream license.

use crate::appearance;
use fastfill_rust::annotations::Tool;
use iced::widget::{button, svg};
use iced::{Background, Color, Element, Theme};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Glyph {
    Tool(Tool),
    Fill,
    SmartSnap,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    Save,
    ExportPdf,
    Help,
    AddImage,
    ImportPdf,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    Camera,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    CameraSwitch,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    CameraMirror,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    CameraRotate,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    CameraPause,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    CameraPlay,
    #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
    CameraSettings,
    ReplaceImage,
    EditPage,
    Delete,
    DeleteAll,
    ResetAngle,
    ToBack,
    ToFront,
    More,
    ChevronDown,
    NewProject,
    OpenProject,
    SaveAs,
    Fit,
    ActualSize,
    MoveUp,
    MoveDown,
}

pub(crate) fn view<Message: 'static>(
    glyph: Glyph,
    selected: bool,
    enabled: bool,
) -> Element<'static, Message> {
    svg(svg::Handle::from_memory(source(glyph)))
        .width(appearance::TOOL_ICON_SIZE)
        .height(appearance::TOOL_ICON_SIZE)
        .style(move |theme: &Theme, _| {
            let status = if enabled {
                button::Status::Active
            } else {
                button::Status::Disabled
            };
            let style = if matches!(glyph, Glyph::Delete | Glyph::DeleteAll) {
                appearance::destructive_action(theme, status)
            } else {
                appearance::tool_button(theme, status, selected)
            };
            // SVG color filtering ignores alpha in the software renderer.
            let blend = |front: Color, back: Color| Color {
                r: front.r * front.a + back.r * (1.0 - front.a),
                g: front.g * front.a + back.g * (1.0 - front.a),
                b: front.b * front.a + back.b * (1.0 - front.a),
                a: 1.0,
            };
            let backdrop = theme.extended_palette().background.base.color;
            let background = match style.background {
                Some(Background::Color(background)) => {
                    blend(background, backdrop)
                }
                _ => backdrop,
            };
            let color = blend(style.text_color, background);
            svg::Style { color: Some(color) }
        })
        .into()
}

#[expect(clippy::too_many_lines, reason = "one exhaustive Lucide asset table")]
fn source(glyph: Glyph) -> &'static [u8] {
    match glyph {
        Glyph::Tool(tool) => match tool {
            Tool::Navigate => include_bytes!("../assets/icons/lucide/hand.svg"),
            Tool::Select => {
                include_bytes!("../assets/icons/lucide/mouse-pointer-2.svg")
            }
            Tool::Rectangle => include_bytes!(
                "../assets/icons/lucide/rectangle-horizontal.svg"
            ),
            Tool::Oval => include_bytes!("../assets/icons/lucide/ellipse.svg"),
            Tool::Line => include_bytes!("../assets/icons/lucide/slash.svg"),
            Tool::Arrow => {
                include_bytes!("../assets/icons/lucide/arrow-up-right.svg")
            }
            Tool::Checkmark => {
                include_bytes!("../assets/icons/lucide/check.svg")
            }
            Tool::Cross => include_bytes!("../assets/icons/lucide/x.svg"),
            Tool::Text => {
                include_bytes!("../assets/icons/lucide/case-sensitive.svg")
            }
            Tool::Pen => include_bytes!("../assets/icons/lucide/pencil.svg"),
            Tool::Highlighter => {
                include_bytes!("../assets/icons/lucide/highlighter.svg")
            }
        },
        Glyph::Fill => {
            include_bytes!("../assets/icons/lucide/paint-bucket.svg")
        }
        Glyph::SmartSnap => include_bytes!("../assets/icons/lucide/magnet.svg"),
        Glyph::Undo => include_bytes!("../assets/icons/lucide/undo-2.svg"),
        Glyph::Redo => include_bytes!("../assets/icons/lucide/redo-2.svg"),
        Glyph::ZoomIn => include_bytes!("../assets/icons/lucide/zoom-in.svg"),
        Glyph::ZoomOut => include_bytes!("../assets/icons/lucide/zoom-out.svg"),
        Glyph::Save => include_bytes!("../assets/icons/lucide/save.svg"),
        Glyph::ExportPdf => {
            include_bytes!("../assets/icons/lucide/file-down.svg")
        }
        Glyph::Help => {
            include_bytes!("../assets/icons/lucide/circle-question-mark.svg")
        }
        Glyph::AddImage => {
            include_bytes!("../assets/icons/lucide/image-plus.svg")
        }
        Glyph::ImportPdf => {
            include_bytes!("../assets/icons/lucide/file-up.svg")
        }
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::Camera => include_bytes!("../assets/icons/lucide/camera.svg"),
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::CameraSwitch => {
            include_bytes!("../assets/icons/lucide/switch-camera.svg")
        }
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::CameraMirror => {
            include_bytes!(
                "../assets/icons/lucide/triangles-centerline-dashed-vertical.svg"
            )
        }
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::CameraRotate => {
            include_bytes!("../assets/icons/lucide/rotate-cw.svg")
        }
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::CameraPause => {
            include_bytes!("../assets/icons/lucide/pause.svg")
        }
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::CameraPlay => {
            include_bytes!("../assets/icons/lucide/play.svg")
        }
        #[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
        Glyph::CameraSettings => {
            include_bytes!("../assets/icons/lucide/settings.svg")
        }
        Glyph::ReplaceImage => {
            include_bytes!("../assets/icons/lucide/image-up.svg")
        }
        Glyph::EditPage => include_bytes!("../assets/icons/lucide/crop.svg"),
        Glyph::Delete => include_bytes!("../assets/icons/lucide/trash.svg"),
        Glyph::DeleteAll => {
            include_bytes!("../assets/icons/lucide/shredder.svg")
        }
        Glyph::ResetAngle => {
            include_bytes!("../assets/icons/lucide/rotate-ccw.svg")
        }
        Glyph::ToBack => {
            include_bytes!("../assets/icons/lucide/send-to-back.svg")
        }
        Glyph::ToFront => {
            include_bytes!("../assets/icons/lucide/bring-to-front.svg")
        }
        Glyph::More => include_bytes!("../assets/icons/lucide/menu.svg"),
        Glyph::ChevronDown => {
            include_bytes!("../assets/icons/lucide/chevron-down.svg")
        }
        Glyph::NewProject => {
            include_bytes!("../assets/icons/lucide/file-plus.svg")
        }
        Glyph::OpenProject => {
            include_bytes!("../assets/icons/lucide/folder-open.svg")
        }
        Glyph::SaveAs => include_bytes!("../assets/icons/lucide/save-all.svg"),
        Glyph::Fit => {
            include_bytes!("../assets/icons/lucide/maximize-2.svg")
        }
        Glyph::ActualSize => include_bytes!("../assets/icons/lucide/scan.svg"),
        Glyph::MoveUp => include_bytes!("../assets/icons/lucide/arrow-up.svg"),
        Glyph::MoveDown => {
            include_bytes!("../assets/icons/lucide/arrow-down.svg")
        }
    }
}
