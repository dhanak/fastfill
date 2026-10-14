//! Lossless schema bridge for the geometry the editor currently supports.

use super::editor_transform as coordinates;
use super::validate::validate_page;
use super::{
    CropQuad, PageFilterSettings, ProjectAnnotation, ProjectPage,
    ProjectShapeKind,
};
use crate::annotations::{
    AnnotationColor, AnnotationGeometry, AnnotationRef, Annotations, InkKind,
    LineKind, OwnedAnnotation, ShapeAnnotation, ShapeKind, TextAnnotation,
    TextFormat, TextTypeface,
};
use crate::geometry::{Point, Rect, Size};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct ProjectEditError(String);

impl fmt::Display for ProjectEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProjectEditError {}

/// C# preview/export uses a 612-point page width, not source image pixels.
///
/// # Errors
/// Returns an error for invalid page metadata.
pub fn editor_page_size(page: &ProjectPage) -> Result<Size, ProjectEditError> {
    validate_page(page).map_err(|error| ProjectEditError(error.to_string()))?;
    #[allow(clippy::cast_precision_loss)]
    Ok(Size::new(
        612.0,
        612.0 * page.source_pixel_height as f32
            / page.source_pixel_width as f32,
    ))
}

/// Load committed annotations in layer order, with no selection or history.
///
/// # Errors
/// Rejects corrections and annotation layouts not yet supported by the editor.
pub fn decode_editor_page(
    page: &ProjectPage,
) -> Result<Annotations, ProjectEditError> {
    let size = editable_size(page)?;
    let annotations = page
        .annotations
        .iter()
        .map(|annotation| decode_annotation(annotation, size))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Annotations::from_ordered(annotations))
}

/// Update annotations, retaining page metadata and unchanged schema objects.
/// Annotations must use `editor_page_size` point units; existing pixel-space
/// editor state is not automatically rescaled by this function.
///
/// # Errors
/// Rejects pending edits, unsupported input, and invalid resulting geometry.
pub fn encode_editor_page(
    original: &ProjectPage,
    editor: &Annotations,
) -> Result<ProjectPage, ProjectEditError> {
    let size = editable_size(original)?;
    if editor.active_pointer().is_some() || editor.has_text_edit() {
        return Err(error("Accept or cancel pending edits before saving."));
    }
    // Validate the entire source before permitting any replacement/deletion.
    let previous = original
        .annotations
        .iter()
        .map(|annotation| {
            Ok((
                annotation.common().0,
                (annotation, decode_annotation(annotation, size)?),
            ))
        })
        .collect::<Result<HashMap<_, _>, ProjectEditError>>()?;
    let mut page = original.clone();
    page.annotations = editor
        .ordered_annotations()
        .map(|annotation| {
            let id = match annotation {
                AnnotationRef::Shape(_, shape) => shape.id,
                AnnotationRef::Text(_, text) => text.id,
            };
            if let Some((source, decoded)) = previous.get(&id) {
                let unchanged = match (annotation, decoded) {
                    (
                        AnnotationRef::Shape(_, shape),
                        OwnedAnnotation::Shape(old),
                    ) => shape == old,
                    (
                        AnnotationRef::Text(_, text),
                        OwnedAnnotation::Text(old),
                    ) => text == old,
                    _ => false,
                };
                if unchanged {
                    return (*source).clone();
                }
            }
            let old = previous.get(&id).map(|(source, _)| *source);
            encode_annotation(annotation, old, size)
        })
        .collect();
    validate_page(&page)
        .map_err(|error| ProjectEditError(error.to_string()))?;
    Ok(page)
}

fn editable_size(page: &ProjectPage) -> Result<Size, ProjectEditError> {
    let size = editor_page_size(page)?;
    if page.crop != CropQuad::default() {
        return Err(error("Page crop is not yet supported by the editor."));
    }
    if page.rotation_quarter_turns != 0 {
        return Err(error("Page rotation is not yet supported by the editor."));
    }
    if page.filter != PageFilterSettings::default() {
        return Err(error("Page filters are not yet supported by the editor."));
    }
    Ok(size)
}

fn decode_annotation(
    annotation: &ProjectAnnotation,
    page: Size,
) -> Result<OwnedAnnotation, ProjectEditError> {
    let (id, stroke_width, transform) = annotation.common();
    let rotation = coordinates::rotation(transform, page).ok_or_else(|| {
        error("Annotation transform includes unsupported scaling or shear.")
    })?;
    let (geometry, color, filled) = match annotation {
        ProjectAnnotation::Shape {
            shape,
            start,
            end,
            color_argb,
            filled,
            ..
        } => {
            let geometry = match shape {
                ProjectShapeKind::Line | ProjectShapeKind::Arrow => {
                    AnnotationGeometry::Linear {
                        kind: if *shape == ProjectShapeKind::Line {
                            LineKind::Line
                        } else {
                            LineKind::Arrow
                        },
                        start: coordinates::point(
                            transform.apply(*start),
                            page,
                        ),
                        end: coordinates::point(transform.apply(*end), page),
                    }
                }
                _ => AnnotationGeometry::Bounded {
                    kind: match shape {
                        ProjectShapeKind::Rectangle => ShapeKind::Rectangle,
                        ProjectShapeKind::Ellipse => ShapeKind::Oval,
                        ProjectShapeKind::Checkmark => ShapeKind::Checkmark,
                        ProjectShapeKind::Cross => ShapeKind::Cross,
                        _ => unreachable!("linear shapes handled above"),
                    },
                    bounds: coordinates::rect(annotation.bounds(), page),
                },
            };
            (geometry, *color_argb, *filled)
        }
        ProjectAnnotation::Freehand {
            points,
            is_highlighter,
            color_argb,
            filled,
            ..
        } => (
            AnnotationGeometry::Ink {
                kind: if *is_highlighter {
                    InkKind::Highlighter
                } else {
                    InkKind::Pen
                },
                points: points
                    .iter()
                    .map(|p| coordinates::point(*p, page))
                    .collect(),
            },
            *color_argb,
            *filled,
        ),
        ProjectAnnotation::Text { .. } => {
            return decode_text(annotation, page, rotation);
        }
    };
    let mut shape = ShapeAnnotation {
        id,
        geometry,
        stroke_width,
        color: AnnotationColor::from_argb(color),
        filled,
        rotation: 0.0,
    };
    if !matches!(shape.geometry, AnnotationGeometry::Linear { .. }) {
        let delta =
            coordinates::translation(shape.bounds().center(), transform, page);
        translate_geometry(&mut shape.geometry, delta);
        shape.rotation = rotation;
    }
    validate_editor_bounds(shape.bounds())?;
    Ok(OwnedAnnotation::Shape(shape))
}

fn encode_annotation(
    annotation: AnnotationRef<'_>,
    previous: Option<&ProjectAnnotation>,
    page: Size,
) -> ProjectAnnotation {
    match annotation {
        AnnotationRef::Text(_, text) => ProjectAnnotation::Text {
            id: text.id,
            text: text.text.clone(),
            bounds: coordinates::normalized_rect(text.bounds, page),
            font_size: text.format.font_size,
            font_family: match text.format.typeface {
                TextTypeface::Sans => "Segoe UI",
                TextTypeface::Serif => "Times New Roman",
                TextTypeface::Monospace => "Consolas",
            }
            .into(),
            maximum_width: 1.0,
            is_bold: text.format.bold,
            is_italic: text.format.italic,
            is_underlined: text.format.underline,
            color_argb: text.format.color.to_argb(),
            stroke_width: previous.map_or(3.0, |old| old.common().1),
            transform: coordinates::around(
                text.bounds.center(),
                text.rotation,
                page,
            ),
        },
        AnnotationRef::Shape(_, shape) => {
            let transform = coordinates::around(
                shape.bounds().center(),
                shape.rotation,
                page,
            );
            match &shape.geometry {
                AnnotationGeometry::Ink { kind, points } => {
                    ProjectAnnotation::Freehand {
                        id: shape.id,
                        points: points
                            .iter()
                            .map(|p| coordinates::normalized(*p, page))
                            .collect(),
                        is_highlighter: *kind == InkKind::Highlighter,
                        filled: shape.filled,
                        color_argb: shape.color.to_argb(),
                        stroke_width: shape.stroke_width,
                        transform,
                    }
                }
                geometry => {
                    let (kind, start, end) = match geometry {
                        AnnotationGeometry::Bounded { kind, bounds } => (
                            match kind {
                                ShapeKind::Rectangle => {
                                    ProjectShapeKind::Rectangle
                                }
                                ShapeKind::Oval => ProjectShapeKind::Ellipse,
                                ShapeKind::Checkmark => {
                                    ProjectShapeKind::Checkmark
                                }
                                ShapeKind::Cross => ProjectShapeKind::Cross,
                            },
                            Point::new(bounds.x, bounds.y),
                            Point::new(
                                bounds.x + bounds.width,
                                bounds.y + bounds.height,
                            ),
                        ),
                        AnnotationGeometry::Linear { kind, start, end } => (
                            if *kind == LineKind::Line {
                                ProjectShapeKind::Line
                            } else {
                                ProjectShapeKind::Arrow
                            },
                            *start,
                            *end,
                        ),
                        AnnotationGeometry::Ink { .. } => unreachable!(),
                    };
                    ProjectAnnotation::Shape {
                        id: shape.id,
                        shape: kind,
                        start: coordinates::normalized(start, page),
                        end: coordinates::normalized(end, page),
                        filled: shape.filled,
                        color_argb: shape.color.to_argb(),
                        stroke_width: shape.stroke_width,
                        transform,
                    }
                }
            }
        }
    }
}

fn decode_text(
    annotation: &ProjectAnnotation,
    page: Size,
    rotation: f32,
) -> Result<OwnedAnnotation, ProjectEditError> {
    let ProjectAnnotation::Text {
        id,
        text,
        bounds,
        font_size,
        font_family,
        maximum_width,
        is_bold,
        is_italic,
        is_underlined,
        color_argb,
        transform,
        ..
    } = annotation
    else {
        unreachable!("text decoder requires text");
    };
    // No silent reflow: legacy soft-wrapped text needs a later slice.
    if maximum_width.to_bits() != 1.0_f32.to_bits() {
        return Err(error("Soft-wrapped text layout is unsupported."));
    }
    let mut bounds = coordinates::rect(*bounds, page);
    let delta = coordinates::translation(bounds.center(), *transform, page);
    bounds.x += delta.x;
    bounds.y += delta.y;
    validate_editor_bounds(bounds)?;
    Ok(OwnedAnnotation::Text(TextAnnotation {
        id: *id,
        text: text.clone(),
        bounds,
        format: TextFormat {
            typeface: decode_typeface(font_family)?,
            font_size: *font_size,
            bold: *is_bold,
            italic: *is_italic,
            underline: *is_underlined,
            color: AnnotationColor::from_argb(*color_argb),
        },
        rotation,
    }))
}

fn translate_geometry(geometry: &mut AnnotationGeometry, delta: Point) {
    match geometry {
        AnnotationGeometry::Bounded { bounds, .. } => {
            bounds.x += delta.x;
            bounds.y += delta.y;
        }
        AnnotationGeometry::Ink { points, .. } => {
            for point in points {
                point.x += delta.x;
                point.y += delta.y;
            }
        }
        AnnotationGeometry::Linear { .. } => unreachable!(),
    }
}

fn decode_typeface(family: &str) -> Result<TextTypeface, ProjectEditError> {
    match family {
        "Segoe UI" => Ok(TextTypeface::Sans),
        "Times New Roman" => Ok(TextTypeface::Serif),
        "Consolas" => Ok(TextTypeface::Monospace),
        _ => Err(error("Text font family is not supported by the editor.")),
    }
}

fn validate_editor_bounds(bounds: Rect) -> Result<(), ProjectEditError> {
    // Editor hit-testing squares distances; schema pixel limits alone do not
    // protect point-space calculations when a source image is very small.
    let values = [
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        bounds.x + bounds.width,
        bounds.y + bounds.height,
        bounds.width * bounds.width + bounds.height * bounds.height,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(error("Annotation exceeds the editor's numeric range."));
    }
    Ok(())
}

fn error(message: &str) -> ProjectEditError {
    ProjectEditError(message.into())
}
