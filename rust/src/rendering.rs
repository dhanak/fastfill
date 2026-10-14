//! Backend-neutral annotation layout in page coordinates.

use crate::annotations::{
    AnnotationColor, AnnotationGeometry, AnnotationRef, InkKind, LineKind,
    ShapeAnnotation, ShapeKind, TextAnnotation, arrowhead_points,
};
use crate::geometry::{Point, Rect};

const ELLIPSE_SEGMENTS: u16 = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenePaint {
    pub color: AnnotationColor,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneLineCap {
    #[default]
    Butt,
    Round,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SceneLineJoin {
    #[default]
    Miter,
    Round,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneStroke {
    pub paint: ScenePaint,
    pub width: f32,
    pub cap: SceneLineCap,
    pub join: SceneLineJoin,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSubpath {
    pub points: Vec<Point>,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenePath {
    pub subpaths: Vec<SceneSubpath>,
    pub fill: Option<ScenePaint>,
    pub stroke: Option<SceneStroke>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderAnnotation<'a> {
    Paths(Vec<ScenePath>),
    Text(&'a TextAnnotation),
}

#[must_use]
pub fn layout_annotation(
    annotation: AnnotationRef<'_>,
) -> RenderAnnotation<'_> {
    match annotation {
        AnnotationRef::Shape(_, annotation) => {
            RenderAnnotation::Paths(layout_shape(annotation))
        }
        AnnotationRef::Text(_, annotation) => {
            RenderAnnotation::Text(annotation)
        }
    }
}

#[must_use]
pub fn layout_shape(annotation: &ShapeAnnotation) -> Vec<ScenePath> {
    match &annotation.geometry {
        AnnotationGeometry::Bounded { kind, bounds } => {
            layout_bounded(annotation, *kind, *bounds)
        }
        AnnotationGeometry::Linear { kind, start, end } => {
            layout_linear(annotation, *kind, *start, *end)
        }
        AnnotationGeometry::Ink { kind, points } => {
            layout_ink(annotation, *kind, points)
        }
    }
}

fn layout_bounded(
    annotation: &ShapeAnnotation,
    kind: ShapeKind,
    bounds: Rect,
) -> Vec<ScenePath> {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Vec::new();
    }
    let width = annotation.stroke_width.min(bounds.width).min(bounds.height);
    match kind {
        ShapeKind::Rectangle | ShapeKind::Oval => {
            let inset = if annotation.filled { 0.0 } else { width / 2.0 };
            let points = if kind == ShapeKind::Rectangle {
                rectangle_points(bounds, inset).to_vec()
            } else {
                ellipse_points(bounds, inset)
            };
            vec![ScenePath {
                subpaths: vec![subpath(annotation, points, true)],
                fill: annotation.filled.then(|| paint(annotation, 1.0)),
                stroke: (!annotation.filled).then(|| {
                    stroke(
                        annotation,
                        width,
                        SceneLineCap::Butt,
                        if kind == ShapeKind::Oval {
                            SceneLineJoin::Round
                        } else {
                            SceneLineJoin::Miter
                        },
                    )
                }),
            }]
        }
        ShapeKind::Checkmark | ShapeKind::Cross => {
            layout_mark(annotation, kind, bounds, width)
        }
    }
}

fn layout_mark(
    annotation: &ShapeAnnotation,
    kind: ShapeKind,
    bounds: Rect,
    width: f32,
) -> Vec<ScenePath> {
    if bounds.width <= width || bounds.height <= width {
        return Vec::new();
    }
    let mark = Rect::new(
        bounds.x + width / 2.0,
        bounds.y + width / 2.0,
        bounds.width - width,
        bounds.height - width,
    );
    let points = mark_points(kind, mark);
    let subpaths = if kind == ShapeKind::Checkmark {
        vec![subpath(annotation, points[..3].to_vec(), false)]
    } else {
        vec![
            subpath(annotation, points[..2].to_vec(), false),
            subpath(annotation, points[2..].to_vec(), false),
        ]
    };
    vec![ScenePath {
        subpaths,
        fill: None,
        stroke: Some(stroke(
            annotation,
            width,
            SceneLineCap::Butt,
            SceneLineJoin::Round,
        )),
    }]
}

fn layout_linear(
    annotation: &ShapeAnnotation,
    kind: LineKind,
    start: Point,
    end: Point,
) -> Vec<ScenePath> {
    let mut subpaths = vec![SceneSubpath {
        points: vec![start, end],
        closed: false,
    }];
    if kind == LineKind::Arrow
        && let Some((left, right)) = arrowhead_points(start, end)
    {
        subpaths.push(SceneSubpath {
            points: vec![left, end, right],
            closed: false,
        });
    }
    vec![ScenePath {
        subpaths,
        fill: None,
        stroke: Some(stroke(
            annotation,
            annotation.stroke_width,
            SceneLineCap::Butt,
            SceneLineJoin::Round,
        )),
    }]
}

fn layout_ink(
    annotation: &ShapeAnnotation,
    kind: InkKind,
    points: &[Point],
) -> Vec<ScenePath> {
    if points.len() < 2 {
        return Vec::new();
    }
    let opacity = if kind == InkKind::Highlighter {
        0.35
    } else {
        1.0
    };
    let fill = (annotation.filled && points.len() >= 3)
        .then(|| paint(annotation, opacity));
    vec![ScenePath {
        subpaths: vec![subpath(annotation, points.to_vec(), annotation.filled)],
        fill,
        stroke: Some(stroke(
            annotation,
            annotation.stroke_width,
            SceneLineCap::Round,
            SceneLineJoin::Round,
        )),
    }]
}

fn subpath(
    annotation: &ShapeAnnotation,
    points: Vec<Point>,
    closed: bool,
) -> SceneSubpath {
    SceneSubpath {
        points: points
            .into_iter()
            .map(|point| annotation.rotated_point(point))
            .collect(),
        closed,
    }
}

fn paint(annotation: &ShapeAnnotation, opacity: f32) -> ScenePaint {
    ScenePaint {
        color: annotation.color,
        opacity,
    }
}

fn stroke(
    annotation: &ShapeAnnotation,
    width: f32,
    cap: SceneLineCap,
    join: SceneLineJoin,
) -> SceneStroke {
    SceneStroke {
        paint: paint(
            annotation,
            if matches!(
                annotation.geometry,
                AnnotationGeometry::Ink {
                    kind: InkKind::Highlighter,
                    ..
                }
            ) {
                0.35
            } else {
                1.0
            },
        ),
        width,
        cap,
        join,
    }
}

fn rectangle_points(bounds: Rect, inset: f32) -> [Point; 4] {
    let left = bounds.x + inset;
    let top = bounds.y + inset;
    let right = bounds.x + bounds.width - inset;
    let bottom = bounds.y + bounds.height - inset;
    [
        Point::new(left, top),
        Point::new(right, top),
        Point::new(right, bottom),
        Point::new(left, bottom),
    ]
}

fn ellipse_points(bounds: Rect, inset: f32) -> Vec<Point> {
    let center = bounds.center();
    let width = (bounds.width - inset * 2.0).max(0.0);
    let height = (bounds.height - inset * 2.0).max(0.0);
    let step = std::f32::consts::TAU / f32::from(ELLIPSE_SEGMENTS);
    (0..ELLIPSE_SEGMENTS)
        .map(|index| {
            let angle = step * f32::from(index);
            Point::new(
                center.x + width / 2.0 * angle.cos(),
                center.y + height / 2.0 * angle.sin(),
            )
        })
        .collect()
}

fn mark_points(kind: ShapeKind, bounds: Rect) -> [Point; 4] {
    let point = |x: f32, y: f32| {
        Point::new(bounds.x + x * bounds.width, bounds.y + y * bounds.height)
    };
    match kind {
        ShapeKind::Checkmark => [
            point(0.0, 0.45),
            point(0.36, 1.0),
            point(1.0, 0.0),
            point(1.0, 0.0),
        ],
        ShapeKind::Cross => [
            point(0.0, 0.0),
            point(1.0, 1.0),
            point(1.0, 0.0),
            point(0.0, 1.0),
        ],
        ShapeKind::Rectangle | ShapeKind::Oval => {
            unreachable!("mark layout requires a mark kind")
        }
    }
}
