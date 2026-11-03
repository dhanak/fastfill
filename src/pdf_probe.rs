//! Narrow PDF probe for backend-neutral vector annotation paths.

use crate::annotations::{AnnotationColor, AnnotationRef};
use crate::geometry::Size;
use crate::rendering::{
    RenderAnnotation, SceneLineCap, SceneLineJoin, ScenePaint, ScenePath,
    SceneStroke, layout_annotation,
};
use krilla::Document;
use krilla::color::rgb;
use krilla::geom::PathBuilder;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, LineCap, LineJoin, Stroke};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PdfProbeError {
    InvalidPageSize,
    TextRequiresFlattening,
    Serialization(String),
}

impl Display for PdfProbeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPageSize => formatter.write_str("invalid page size"),
            Self::TextRequiresFlattening => formatter.write_str(
                "exact text output requires the iced-shaped page to be \
                 flattened",
            ),
            Self::Serialization(message) => {
                write!(formatter, "PDF serialization failed: {message}")
            }
        }
    }
}

impl Error for PdfProbeError {}

/// Serialize one vector-only annotation page for backend evaluation.
///
/// # Errors
///
/// Returns [`PdfProbeError::InvalidPageSize`] for invalid dimensions,
/// [`PdfProbeError::TextRequiresFlattening`] when text would need a different
/// shaping engine, or [`PdfProbeError::Serialization`] when PDF generation
/// fails.
pub fn render_page<'a>(
    page_size: Size,
    annotations: impl IntoIterator<Item = AnnotationRef<'a>>,
) -> Result<Vec<u8>, PdfProbeError> {
    if !page_size.width.is_finite()
        || !page_size.height.is_finite()
        || page_size.width <= 0.0
        || page_size.height <= 0.0
    {
        return Err(PdfProbeError::InvalidPageSize);
    }

    let annotations: Vec<_> = annotations.into_iter().collect();
    if annotations
        .iter()
        .any(|annotation| matches!(annotation, AnnotationRef::Text(..)))
    {
        return Err(PdfProbeError::TextRequiresFlattening);
    }

    let settings = PageSettings::from_wh(page_size.width, page_size.height)
        .ok_or(PdfProbeError::InvalidPageSize)?;
    let mut document = Document::new();
    let mut page = document.start_page_with(settings);
    let mut surface = page.surface();

    for annotation in annotations {
        let RenderAnnotation::Paths(paths) = layout_annotation(annotation)
        else {
            unreachable!("text annotations were rejected before rendering")
        };
        for path in paths {
            draw_path(&mut surface, &path);
        }
    }

    surface.finish();
    page.finish();
    document
        .finish()
        .map_err(|error| PdfProbeError::Serialization(error.to_string()))
}

fn draw_path(surface: &mut krilla::surface::Surface<'_>, scene: &ScenePath) {
    let Some(path) = build_path(scene) else {
        return;
    };
    surface.set_fill(scene.fill.map(fill));
    surface.set_stroke(scene.stroke.map(stroke));
    surface.draw_path(&path);
}

fn build_path(scene: &ScenePath) -> Option<krilla::geom::Path> {
    let mut builder = PathBuilder::new();
    for subpath in &scene.subpaths {
        let (first, rest) = subpath.points.split_first()?;
        builder.move_to(first.x, first.y);
        for point in rest {
            builder.line_to(point.x, point.y);
        }
        if subpath.closed {
            builder.close();
        }
    }
    builder.finish()
}

fn fill(paint: ScenePaint) -> Fill {
    Fill {
        paint: color(paint.color).into(),
        opacity: opacity(paint.opacity),
        ..Fill::default()
    }
}

fn stroke(stroke: SceneStroke) -> Stroke {
    Stroke {
        paint: color(stroke.paint.color).into(),
        width: stroke.width,
        line_cap: match stroke.cap {
            SceneLineCap::Butt => LineCap::Butt,
            SceneLineCap::Round => LineCap::Round,
        },
        line_join: match stroke.join {
            SceneLineJoin::Miter => LineJoin::Miter,
            SceneLineJoin::Round => LineJoin::Round,
        },
        opacity: opacity(stroke.paint.opacity),
        ..Stroke::default()
    }
}

fn color(color: AnnotationColor) -> rgb::Color {
    let [red, green, blue] = color.rgb();
    rgb::Color::new(red, green, blue)
}

fn opacity(value: f32) -> NormalizedF32 {
    NormalizedF32::new(value)
        .expect("shared annotation layout only emits normalized opacity")
}
