use crate::appearance::text_font;
use fastfill_rust::annotations::{
    TEXT_LINE_HEIGHT, TextAnnotation, TextFormat,
};
use fastfill_rust::geometry::{Point as PagePoint, Size as PageSize};
use iced::advanced::graphics::text::{
    Editor as GraphicsEditor, Paragraph as GraphicsParagraph,
};
use iced::advanced::text::{self, Editor as _, Paragraph as _, Text};
use iced::{Pixels, Size, alignment};

const MIN_CONTENT_WIDTH: f32 = 48.0;
const CARET_SPACE: f32 = 2.0;

pub(super) fn cursor_at(
    annotation: &TextAnnotation,
    point: PagePoint,
    scale: f32,
) -> text::editor::Cursor {
    // Reuse iced's shaped-text hit testing, including Unicode and newlines.
    let mut editor = GraphicsEditor::with_text(&annotation.text);
    editor.update(
        Size::INFINITE,
        text_font(annotation.format),
        Pixels(annotation.format.font_size * scale),
        text::LineHeight::Relative(TEXT_LINE_HEIGHT),
        text::Wrapping::None,
        &mut text::highlighter::PlainText,
    );
    let point = annotation.unrotated_point(point);
    editor.perform(text::editor::Action::Click(iced::Point::new(
        (point.x - annotation.bounds.x) * scale,
        (point.y - annotation.bounds.y) * scale,
    )));
    editor.cursor()
}

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

    #[test]
    fn caret_hit_testing_uses_local_glyphs_at_every_rotation_and_zoom() {
        use fastfill_rust::annotations::TextTypeface;
        use fastfill_rust::geometry::Rect;

        for scale in [0.5, 1.0, 3.0] {
            for rotation in [0.0, std::f32::consts::FRAC_PI_4, -1.2] {
                for typeface in TextTypeface::ALL {
                    let format = TextFormat {
                        typeface,
                        bold: true,
                        italic: true,
                        ..TextFormat::default()
                    };
                    let content = "First line\nárvíztűrő 👋🏽 text";
                    let measured = measure_text(content, format, scale);
                    let text = TextAnnotation {
                        id: uuid::Uuid::new_v4(),
                        text: content.into(),
                        bounds: Rect::new(
                            200.0,
                            200.0,
                            measured.width,
                            measured.height,
                        ),
                        format,
                        rotation,
                    };
                    let point = PagePoint::new(
                        282.0,
                        200.0 + format.font_size * TEXT_LINE_HEIGHT * 1.5,
                    );
                    let cursor =
                        cursor_at(&text, text.rotated_point(point), scale);
                    assert_eq!(cursor.position.line, 1);
                    assert!(cursor.position.column > 0);
                    assert!(cursor.position.column < "árvíztűrő 👋🏽 text".len());
                    assert!(
                        "árvíztűrő 👋🏽 text"
                            .is_char_boundary(cursor.position.column)
                    );
                    assert!(cursor.selection.is_none());
                }
            }
        }
    }
}
