use super::archive::ProjectLoadError;
use super::model::{
    AffineTransform, NormalizedPoint, NormalizedRect, Project,
    ProjectAnnotation, ProjectPage, ProjectShapeKind,
};
use std::collections::HashSet;

pub(super) const MAXIMUM_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
pub(super) const MAXIMUM_IMAGE_PIXELS: i64 = 50_000_000;
const MAXIMUM_PAGES: usize = 20;
const MAXIMUM_ANNOTATIONS_PER_PAGE: usize = 10_000;
const MAXIMUM_INK_POINTS: usize = 100_000;
const MAXIMUM_TEXT_LENGTH: usize = 10_000;

pub(super) fn validate_project(
    project: &Project,
) -> Result<(), ProjectLoadError> {
    if project.schema_version != 1 {
        return invalid(if project.schema_version > 1 {
            "Project needs a newer FastFill version."
        } else {
            "Project schema is unsupported."
        });
    }
    if project.pages.len() > MAXIMUM_PAGES {
        return invalid("A project may contain at most 20 pages.");
    }
    if utf16_len(&project.title) > 200 {
        return invalid("Project title is too long.");
    }

    let mut page_ids = HashSet::new();
    for page in &project.pages {
        if !page_ids.insert(page.id) {
            return invalid("Page IDs must be unique.");
        }
        validate_page(page)?;
    }
    Ok(())
}

pub(super) fn validate_page(
    page: &ProjectPage,
) -> Result<(), ProjectLoadError> {
    validate_asset_name(&page.source_asset)?;
    if page.source_pixel_width <= 0
        || page.source_pixel_height <= 0
        || i64::from(page.source_pixel_width)
            * i64::from(page.source_pixel_height)
            > MAXIMUM_IMAGE_PIXELS
    {
        return invalid("Image dimensions exceed the 50-megapixel limit.");
    }
    validate_crop(page)?;
    if !(0..=3).contains(&page.rotation_quarter_turns) {
        return invalid("Page rotation is invalid.");
    }
    if !(-100..=100).contains(&page.filter.contrast) {
        return invalid("Page filter is invalid.");
    }
    if page.annotations.len() > MAXIMUM_ANNOTATIONS_PER_PAGE {
        return invalid("Page annotations are too numerous.");
    }

    let mut annotation_ids = HashSet::new();
    for annotation in &page.annotations {
        let (id, _, _) = annotation.common();
        if !annotation_ids.insert(id) {
            return invalid("Annotation IDs must be unique per page.");
        }
        validate_annotation(annotation, page)?;
    }
    Ok(())
}

fn validate_crop(page: &ProjectPage) -> Result<(), ProjectLoadError> {
    if !page.crop.is_valid() {
        return invalid(
            "Crop corners must be finite, inside the page, and convex.",
        );
    }
    Ok(())
}

fn validate_annotation(
    annotation: &ProjectAnnotation,
    page: &ProjectPage,
) -> Result<(), ProjectLoadError> {
    let (_, stroke_width, transform) = annotation.common();
    if !stroke_width.is_finite() || !(0.25..=40.0).contains(&stroke_width) {
        return invalid("Annotation stroke width is invalid.");
    }
    validate_transform(transform)?;

    match annotation {
        ProjectAnnotation::Freehand { points, .. } => {
            if points.is_empty()
                || points.len() > MAXIMUM_INK_POINTS
                || points.iter().any(|point| !finite_point(*point))
            {
                return invalid("Freehand annotation points are invalid.");
            }
        }
        ProjectAnnotation::Text {
            text,
            bounds,
            font_size,
            font_family,
            maximum_width,
            ..
        } => {
            if utf16_len(text) > MAXIMUM_TEXT_LENGTH {
                return invalid("Text annotation is too long.");
            }
            if !font_size.is_finite() || !(6.0..=144.0).contains(font_size) {
                return invalid("Text annotation font size is invalid.");
            }
            if font_family.trim().is_empty() || utf16_len(font_family) > 100 {
                return invalid("Text annotation font family is invalid.");
            }
            if !maximum_width.is_finite()
                || *maximum_width <= 0.0
                || *maximum_width > 1.0
            {
                return invalid("Text annotation maximum width is invalid.");
            }
            if !finite_rect(*bounds)
                || bounds.width <= 0.0
                || bounds.height <= 0.0
            {
                return invalid("Text annotation bounds are invalid.");
            }
        }
        ProjectAnnotation::Shape { start, end, .. } => {
            if !finite_point(*start) || !finite_point(*end) {
                return invalid("Shape annotation geometry is invalid.");
            }
        }
    }

    let bounds = annotation.bounds();
    if !finite_rect(bounds) {
        return invalid("Annotation geometry is invalid.");
    }
    validate_render_range(annotation, page, bounds, transform)?;

    let transformed = transformed_bounds(bounds, transform)?;
    if transformed.right() < 0.0
        || transformed.bottom() < 0.0
        || transformed.x > 1.0
        || transformed.y > 1.0
    {
        return invalid("Annotation must be at least partly on the page.");
    }
    Ok(())
}

fn validate_transform(
    transform: AffineTransform,
) -> Result<(), ProjectLoadError> {
    let values = [
        transform.m11,
        transform.m12,
        transform.m21,
        transform.m22,
        transform.m31,
        transform.m32,
    ];
    let determinant =
        transform.m11 * transform.m22 - transform.m12 * transform.m21;
    if values.iter().any(|value| !value.is_finite())
        || !determinant.is_finite()
        || determinant == 0.0
        || !inverse_is_finite(transform, determinant)
    {
        return invalid("Annotation transform is invalid.");
    }
    Ok(())
}

fn validate_render_range(
    annotation: &ProjectAnnotation,
    page: &ProjectPage,
    bounds: NormalizedRect,
    transform: AffineTransform,
) -> Result<(), ProjectLoadError> {
    let width = f64::from(page.source_pixel_width);
    let height = f64::from(page.source_pixel_height);
    let mut points = bounds.corners().to_vec();
    match annotation {
        ProjectAnnotation::Freehand {
            points: ink_points, ..
        } => points.extend_from_slice(ink_points),
        ProjectAnnotation::Shape { start, end, .. } => {
            points.extend([*start, *end]);
        }
        ProjectAnnotation::Text { .. } => {}
    }
    if points.iter().any(|point| {
        !renderable(*point, width, height)
            || !renderable(transform.apply(*point), width, height)
    }) {
        return invalid("Annotation exceeds the renderer's numeric range.");
    }
    if let ProjectAnnotation::Shape {
        shape: ProjectShapeKind::Arrow,
        start,
        end,
        ..
    } = annotation
    {
        let delta_x = f64::from(end.x - start.x) * width;
        let delta_y = f64::from(end.y - start.y) * height;
        let squared = delta_x * delta_x + delta_y * delta_y;
        if !delta_x.is_finite()
            || !delta_y.is_finite()
            || !squared.is_finite()
            || squared > f64::from(f32::MAX)
        {
            return invalid("Arrow exceeds the renderer's numeric range.");
        }
    }
    Ok(())
}

fn inverse_is_finite(transform: AffineTransform, determinant: f32) -> bool {
    [
        transform.m22 / determinant,
        -transform.m12 / determinant,
        -transform.m21 / determinant,
        transform.m11 / determinant,
        (transform.m21 * transform.m32 - transform.m31 * transform.m22)
            / determinant,
        (transform.m31 * transform.m12 - transform.m11 * transform.m32)
            / determinant,
    ]
    .iter()
    .all(|value| value.is_finite())
}

fn transformed_bounds(
    bounds: NormalizedRect,
    transform: AffineTransform,
) -> Result<NormalizedRect, ProjectLoadError> {
    let corners = bounds.corners().map(|point| transform.apply(point));
    if corners.iter().any(|point| !finite_point(*point)) {
        return invalid("Annotation transform is invalid.");
    }
    let min_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    Ok(NormalizedRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

fn renderable(point: NormalizedPoint, width: f64, height: f64) -> bool {
    let x = f64::from(point.x) * width;
    let y = f64::from(point.y) * height;
    point.x.is_finite()
        && point.y.is_finite()
        && x.is_finite()
        && y.is_finite()
        && x.abs() <= f64::from(f32::MAX)
        && y.abs() <= f64::from(f32::MAX)
}

fn finite_point(point: NormalizedPoint) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn finite_rect(rectangle: NormalizedRect) -> bool {
    rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
        && rectangle.right().is_finite()
        && rectangle.bottom().is_finite()
}

pub(super) fn validate_asset_name(name: &str) -> Result<(), ProjectLoadError> {
    if name.trim().is_empty()
        || name.ends_with(['.', ' '])
        || name.contains(['/', '\\'])
        || name.contains("..")
        || name.chars().any(|character| {
            character.is_control()
                || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        })
    {
        return invalid("Page asset name is invalid.");
    }
    let Some((_, extension)) = name.rsplit_once('.') else {
        return invalid("Page asset type is unsupported.");
    };
    let stem = name
        .split_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem.len() == 4
            && matches!(&stem[..3], "COM" | "LPT")
            && matches!(stem.as_bytes()[3], b'1'..=b'9')
    {
        return invalid("Page asset name is invalid.");
    }
    if !matches!(
        extension.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png"
    ) {
        return invalid("Page asset type is unsupported.");
    }
    Ok(())
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn invalid<T>(message: &str) -> Result<T, ProjectLoadError> {
    Err(ProjectLoadError::Invalid(message.into()))
}
