use crate::appearance::text_font;
use fastfill_rust::annotations::{TEXT_LINE_HEIGHT, TextFormat};
use fastfill_rust::geometry::Size as PageSize;
use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
use iced::advanced::text::{self, Paragraph as _, Text};
use iced::{Pixels, Size, alignment};

const MIN_CONTENT_WIDTH: f32 = 48.0;
const CARET_SPACE: f32 = 2.0;

pub(super) fn measure_text(
    content: &str,
    format: TextFormat,
    scale: f32,
) -> PageSize {
    if !format.font_size.is_finite()
        || !scale.is_finite()
        || format.font_size <= 0.0
        || scale <= 0.0
    {
        return PageSize::new(1.0, 1.0);
    }
    let size = Pixels(format.font_size * scale);
    let measured =
        paragraph(content, format, size, Size::INFINITE).min_bounds();
    let width =
        (measured.width + CARET_SPACE * scale).max(MIN_CONTENT_WIDTH * scale);
    let height = measured.height.max(size.0 * TEXT_LINE_HEIGHT);
    PageSize::new(width / scale, height / scale)
}

fn paragraph(
    content: &str,
    format: TextFormat,
    size: Pixels,
    bounds: Size,
) -> GraphicsParagraph {
    GraphicsParagraph::with_text(Text {
        content,
        bounds,
        size,
        line_height: text::LineHeight::Relative(TEXT_LINE_HEIGHT),
        font: text_font(format),
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_tracks_content_and_explicit_lines() {
        let format = TextFormat::default();
        let short = measure_text("Hi", format, 1.0);
        let long = measure_text("A substantially longer value", format, 1.0);
        let multiline = measure_text("First\nSecond", format, 1.0);

        assert!(long.width > short.width);
        assert!(multiline.height > short.height);
        assert_eq!(multiline.width, measure_text("Second", format, 1.0).width,);
    }

    #[test]
    fn measurement_does_not_soft_wrap_and_ignores_zoom() {
        let content = "A long line that must stay on one row";
        let format = TextFormat::default();
        let first = measure_text(content, format, 1.0);
        let second = measure_text(content, format, 2.0);

        assert!(first.width > 300.0);
        assert!((first.height - 24.0 * TEXT_LINE_HEIGHT).abs() < 0.01);
        assert!((first.width - second.width).abs() < 0.01);
        assert!((first.height - second.height).abs() < 0.01);
    }

    #[test]
    fn measurement_uses_selected_typeface_weight_and_size() {
        let plain = TextFormat::default();
        let formatted = TextFormat {
            typeface: fastfill_rust::annotations::TextTypeface::Serif,
            font_size: 40.0,
            bold: true,
            ..plain
        };

        let plain = measure_text("Wide text", plain, 1.0);
        let formatted = measure_text("Wide text", formatted, 1.0);

        assert!(formatted.height > plain.height);
        assert_ne!(formatted.width, plain.width);
    }
}
