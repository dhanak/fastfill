use fastfill_rust::annotations::AnnotationColor;
use iced::Color;

pub(crate) const TOOLBAR_HEIGHT: f32 = 168.0;
pub(crate) const TOOLBAR_ROW_HEIGHT: f32 = TOOLBAR_HEIGHT / 3.0;
pub(crate) const CONTROL_HEIGHT: f32 = 40.0;
pub(crate) const CONTROL_GAP: f32 = 8.0;
pub(crate) const TOOLBAR_PADDING: [u16; 2] = [8, 12];
pub(crate) const PAGE_MARGIN: f32 = 36.0;
pub(crate) const PAGE_SHADOW_X: f32 = 5.0;
pub(crate) const PAGE_SHADOW_Y: f32 = 7.0;
pub(crate) const SELECTION_HANDLE_SIZE: f32 = 14.0;
pub(crate) const SELECTION_STROKE: f32 = 2.0;

pub(crate) fn accent() -> Color {
    Color::from_rgb8(0, 102, 204)
}

pub(crate) fn canvas_background() -> Color {
    Color::from_rgb8(67, 72, 78)
}

pub(crate) fn page_shadow() -> Color {
    Color::from_rgba8(0, 0, 0, 0.22)
}

pub(crate) fn annotation_color(color: AnnotationColor) -> Color {
    match color {
        AnnotationColor::Black => Color::BLACK,
        AnnotationColor::White => Color::WHITE,
        AnnotationColor::Red => Color::from_rgb8(220, 32, 32),
        AnnotationColor::Blue => accent(),
        AnnotationColor::Green => Color::from_rgb8(0, 160, 72),
        AnnotationColor::Yellow => Color::from_rgb8(245, 196, 0),
    }
}
