use super::geometry::{
    angle_from, constrain_line, distance, drag_bounds,
    keep_annotation_reachable, mark_bounds, normalized_rotation,
    resize_ink_points, rotate_point,
};
use super::{
    AnnotationGeometry, AnnotationStyle, CreationKind, EditHit, InkKind,
    LineEndpoint, Pointer, ShapeAnnotation,
};
use crate::geometry::{Point, Rect, Size};

#[derive(Debug)]
pub(super) struct ShapeDraft {
    pub(super) pointer: Pointer,
    pub(super) kind: CreationKind,
    pub(super) start: Point,
    pub(super) end: Point,
    pub(super) square: bool,
    pub(super) page: Size,
    pub(super) style: AnnotationStyle,
    pub(super) points: Vec<Point>,
}

#[derive(Debug)]
pub(super) struct EditDraft {
    pub(super) pointer: Pointer,
    pub(super) index: usize,
    pub(super) original: ShapeAnnotation,
    pub(super) start: Point,
    pub(super) end: Point,
    pub(super) square: bool,
    pub(super) hit: EditHit,
    pub(super) page: Size,
}

impl EditDraft {
    pub(super) fn annotation(&self) -> ShapeAnnotation {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let annotation = match self.hit {
            EditHit::Move => self.original.translated(dx, dy),
            EditHit::Resize(corner) => self.resized(corner, dx, dy),
            EditHit::Endpoint(endpoint) => {
                self.endpoint_moved(endpoint, dx, dy)
            }
            EditHit::Rotate => self.rotated(),
        };
        keep_annotation_reachable(&annotation, self.page)
    }

    fn resized(
        &self,
        corner: super::ResizeCorner,
        dx: f32,
        dy: f32,
    ) -> ShapeAnnotation {
        let controls = self.original.bounds();
        let moving = corner.position(controls);
        let square = self.square
            || matches!(
                &self.original.geometry,
                AnnotationGeometry::Bounded { kind, .. } if kind.is_mark()
            );
        let center = controls.center();
        let moving = self.original.rotated_point(moving);
        let moving = Point::new(moving.x + dx, moving.y + dy);
        let moving = rotate_point(moving, center, -self.original.rotation);
        let local =
            drag_bounds(corner.opposite().position(controls), moving, square);
        let center = self.original.rotated_point(local.center());
        let resized = Rect::new(
            center.x - local.width / 2.0,
            center.y - local.height / 2.0,
            local.width,
            local.height,
        );
        let geometry = match &self.original.geometry {
            AnnotationGeometry::Bounded { kind, .. } => {
                AnnotationGeometry::Bounded {
                    kind: *kind,
                    bounds: resized,
                }
            }
            AnnotationGeometry::Ink { kind, points } => {
                AnnotationGeometry::Ink {
                    kind: *kind,
                    points: resize_ink_points(
                        points,
                        resized,
                        self.original.stroke_width,
                    ),
                }
            }
            AnnotationGeometry::Linear { .. } => {
                return self.original.clone();
            }
        };
        ShapeAnnotation {
            geometry,
            stroke_width: self.original.stroke_width,
            color: self.original.color,
            filled: self.original.filled,
            rotation: self.original.rotation,
        }
    }

    fn endpoint_moved(
        &self,
        endpoint: LineEndpoint,
        dx: f32,
        dy: f32,
    ) -> ShapeAnnotation {
        let AnnotationGeometry::Linear { kind, start, end } =
            &self.original.geometry
        else {
            return self.original.clone();
        };
        let fixed = match endpoint.opposite() {
            LineEndpoint::Start => *start,
            LineEndpoint::End => *end,
        };
        let moving = match endpoint {
            LineEndpoint::Start => *start,
            LineEndpoint::End => *end,
        };
        let moving = Point::new(moving.x + dx, moving.y + dy);
        let moving = constrain_line(fixed, moving, self.square);
        let (start, end) = match endpoint {
            LineEndpoint::Start => (moving, fixed),
            LineEndpoint::End => (fixed, moving),
        };
        ShapeAnnotation {
            geometry: AnnotationGeometry::Linear {
                kind: *kind,
                start,
                end,
            },
            stroke_width: self.original.stroke_width,
            color: self.original.color,
            filled: self.original.filled,
            rotation: 0.0,
        }
    }

    fn rotated(&self) -> ShapeAnnotation {
        let center = self.original.bounds().center();
        let start = angle_from(center, self.start);
        let end = angle_from(center, self.end);
        let mut rotation = normalized_rotation(
            self.original.rotation + normalized_rotation(end - start),
        );
        if self.square {
            let step = std::f32::consts::FRAC_PI_4;
            rotation = normalized_rotation((rotation / step).round() * step);
        }
        let mut annotation = self.original.clone();
        annotation.rotation = rotation;
        annotation
    }
}

#[derive(Debug)]
pub(super) enum Draft {
    Create(ShapeDraft),
    Edit(EditDraft),
}

impl Draft {
    pub(super) fn pointer(&self) -> Pointer {
        match self {
            Self::Create(draft) => draft.pointer,
            Self::Edit(draft) => draft.pointer,
        }
    }

    pub(super) fn annotation(&self) -> ShapeAnnotation {
        match self {
            Self::Create(draft) => draft.annotation(),
            Self::Edit(draft) => draft.annotation(),
        }
    }

    pub(super) fn update(&mut self, point: Point, square: bool) {
        match self {
            Self::Create(draft) => {
                draft.end = point;
                draft.square = square;
                if matches!(draft.kind, CreationKind::Ink(_))
                    && draft
                        .points
                        .last()
                        .is_none_or(|last| distance(*last, point) >= 0.5)
                {
                    draft.points.push(point);
                }
            }
            Self::Edit(draft) => {
                draft.end = point;
                draft.square = square;
            }
        }
    }

    pub(super) fn constrain_square(&mut self, square: bool) {
        match self {
            Self::Create(draft) => draft.square = square,
            Self::Edit(draft) => draft.square = square,
        }
    }
}

impl ShapeDraft {
    fn annotation(&self) -> ShapeAnnotation {
        let is_mark = matches!(
            self.kind,
            CreationKind::Bounded(kind) if kind.is_mark()
        );
        let geometry = match self.kind {
            CreationKind::Bounded(kind) => AnnotationGeometry::Bounded {
                kind,
                bounds: if kind.is_mark() {
                    mark_bounds(self.start, self.end, self.page)
                } else {
                    drag_bounds(self.start, self.end, self.square)
                },
            },
            CreationKind::Linear(kind) => AnnotationGeometry::Linear {
                kind,
                start: self.start,
                end: constrain_line(self.start, self.end, self.square),
            },
            CreationKind::Ink(kind) => AnnotationGeometry::Ink {
                kind,
                points: self.points.clone(),
            },
        };
        ShapeAnnotation {
            geometry,
            stroke_width: self.style.stroke_width,
            color: self.style.color,
            filled: self.style.filled
                && !is_mark
                && !matches!(
                    self.kind,
                    CreationKind::Ink(InkKind::Highlighter)
                ),
            rotation: 0.0,
        }
    }
}
