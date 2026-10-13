use crate::geometry::{Point, Rect};

mod geometry;
mod interaction;
mod selection;
mod state;
mod style;

pub use geometry::arrowhead_points;
use geometry::{
    ink_bounds, linear_bounds, point_bounds, rotate_point, valid_scale,
};
use interaction::{
    Draft, EditDraft, GroupMoveDraft, SelectionDraft, ShapeDraft,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Navigate,
    Rectangle,
    Oval,
    Line,
    Arrow,
    Checkmark,
    Cross,
    Pen,
    Highlighter,
    Select,
}

impl Tool {
    const fn creation_kind(self) -> Option<CreationKind> {
        match self {
            Self::Rectangle => {
                Some(CreationKind::Bounded(ShapeKind::Rectangle))
            }
            Self::Oval => Some(CreationKind::Bounded(ShapeKind::Oval)),
            Self::Line => Some(CreationKind::Linear(LineKind::Line)),
            Self::Arrow => Some(CreationKind::Linear(LineKind::Arrow)),
            Self::Checkmark => {
                Some(CreationKind::Bounded(ShapeKind::Checkmark))
            }
            Self::Cross => Some(CreationKind::Bounded(ShapeKind::Cross)),
            Self::Pen => Some(CreationKind::Ink(InkKind::Pen)),
            Self::Highlighter => Some(CreationKind::Ink(InkKind::Highlighter)),
            Self::Navigate | Self::Select => None,
        }
    }

    const fn style_index(self) -> Option<usize> {
        match self {
            Self::Rectangle => Some(0),
            Self::Oval => Some(1),
            Self::Line => Some(2),
            Self::Arrow => Some(3),
            Self::Checkmark => Some(4),
            Self::Cross => Some(5),
            Self::Pen => Some(6),
            Self::Highlighter => Some(7),
            Self::Navigate | Self::Select => None,
        }
    }

    const fn supports_fill(self) -> bool {
        matches!(self, Self::Rectangle | Self::Oval | Self::Pen)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnnotationColor {
    #[default]
    Black,
    White,
    Red,
    Blue,
    Green,
    Yellow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationStyle {
    pub color: AnnotationColor,
    pub stroke_width: f32,
    pub filled: bool,
}

impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            color: AnnotationColor::Black,
            stroke_width: 3.0,
            filled: false,
        }
    }
}

#[derive(Debug)]
struct ToolStyles([AnnotationStyle; 8]);

impl Default for ToolStyles {
    fn default() -> Self {
        let mut styles = [AnnotationStyle::default(); 8];
        styles[4].stroke_width = 2.0;
        styles[5].stroke_width = 2.0;
        styles[7].color = AnnotationColor::Yellow;
        styles[7].stroke_width = 12.0;
        Self(styles)
    }
}

impl ToolStyles {
    fn get(&self, tool: Tool) -> Option<AnnotationStyle> {
        tool.style_index().map(|index| self.0[index])
    }

    fn get_mut(&mut self, tool: Tool) -> Option<&mut AnnotationStyle> {
        tool.style_index().map(|index| &mut self.0[index])
    }
}

#[derive(Clone, Copy, Debug)]
enum CreationKind {
    Bounded(ShapeKind),
    Linear(LineKind),
    Ink(InkKind),
}

impl CreationKind {
    fn matches(self, annotation: &ShapeAnnotation) -> bool {
        match (self, &annotation.geometry) {
            (
                Self::Bounded(expected),
                AnnotationGeometry::Bounded { kind, .. },
            ) => *kind == expected,
            (
                Self::Linear(expected),
                AnnotationGeometry::Linear { kind, .. },
            ) => *kind == expected,
            (Self::Ink(expected), AnnotationGeometry::Ink { kind, .. }) => {
                *kind == expected
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Oval,
    Checkmark,
    Cross,
}

impl ShapeKind {
    const fn is_mark(self) -> bool {
        matches!(self, Self::Checkmark | Self::Cross)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Line,
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkKind {
    Pen,
    Highlighter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Mouse,
    Touch(u64),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DragModifiers {
    pub square: bool,
    pub force_move: bool,
    pub force_resize: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl ResizeCorner {
    #[must_use]
    pub fn position(self, bounds: Rect) -> Point {
        let right = bounds.x + bounds.width;
        let bottom = bounds.y + bounds.height;
        match self {
            Self::TopLeft => Point::new(bounds.x, bounds.y),
            Self::TopRight => Point::new(right, bounds.y),
            Self::BottomRight => Point::new(right, bottom),
            Self::BottomLeft => Point::new(bounds.x, bottom),
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomRight => Self::TopLeft,
            Self::BottomLeft => Self::TopRight,
        }
    }
}

pub const RESIZE_CORNERS: [ResizeCorner; 4] = [
    ResizeCorner::TopLeft,
    ResizeCorner::TopRight,
    ResizeCorner::BottomRight,
    ResizeCorner::BottomLeft,
];

pub const ROTATION_HANDLE_OFFSET: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditHit {
    Move,
    Resize(ResizeCorner),
    Endpoint(LineEndpoint),
    Rotate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationGeometry {
    Bounded {
        kind: ShapeKind,
        bounds: Rect,
    },
    Linear {
        kind: LineKind,
        start: Point,
        end: Point,
    },
    Ink {
        kind: InkKind,
        points: Vec<Point>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEndpoint {
    Start,
    End,
}

impl LineEndpoint {
    fn opposite(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeAnnotation {
    pub geometry: AnnotationGeometry,
    pub stroke_width: f32,
    pub color: AnnotationColor,
    pub filled: bool,
    pub rotation: f32,
}

impl ShapeAnnotation {
    #[must_use]
    pub const fn style(&self) -> AnnotationStyle {
        AnnotationStyle {
            color: self.color,
            stroke_width: self.stroke_width,
            filled: self.filled,
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        match &self.geometry {
            AnnotationGeometry::Bounded { bounds, .. } => *bounds,
            AnnotationGeometry::Linear { kind, start, end } => {
                linear_bounds(*kind, *start, *end, self.stroke_width)
            }
            AnnotationGeometry::Ink { points, .. } => {
                ink_bounds(points, self.stroke_width)
            }
        }
    }

    #[must_use]
    pub fn supports_rotation(&self) -> bool {
        !matches!(self.geometry, AnnotationGeometry::Linear { .. })
    }

    #[must_use]
    pub fn frame_corners(&self) -> Option<[Point; 4]> {
        self.supports_rotation().then(|| {
            let bounds = self.bounds();
            RESIZE_CORNERS
                .map(|corner| self.rotated_point(corner.position(bounds)))
        })
    }

    #[must_use]
    pub fn rotation_handle(&self, scale: f32) -> Option<Point> {
        if !self.supports_rotation() || !valid_scale(scale) {
            return None;
        }
        let bounds = self.bounds();
        let center = bounds.center();
        Some(self.rotated_point(Point::new(
            center.x,
            bounds.y - ROTATION_HANDLE_OFFSET / scale,
        )))
    }

    #[must_use]
    pub fn rotated_point(&self, point: Point) -> Point {
        rotate_point(point, self.bounds().center(), self.rotation)
    }

    fn unrotated_point(&self, point: Point) -> Point {
        rotate_point(point, self.bounds().center(), -self.rotation)
    }

    fn visible_bounds(&self) -> Rect {
        self.frame_corners()
            .map_or_else(|| self.bounds(), |corners| point_bounds(&corners))
    }

    #[must_use]
    pub fn kind(&self) -> Option<ShapeKind> {
        match &self.geometry {
            AnnotationGeometry::Bounded { kind, .. } => Some(*kind),
            AnnotationGeometry::Linear { .. }
            | AnnotationGeometry::Ink { .. } => None,
        }
    }

    fn supports_fill(&self) -> bool {
        matches!(
            self.geometry,
            AnnotationGeometry::Bounded {
                kind: ShapeKind::Rectangle | ShapeKind::Oval,
                ..
            } | AnnotationGeometry::Ink {
                kind: InkKind::Pen,
                ..
            }
        )
    }

    fn translated(&self, dx: f32, dy: f32) -> Self {
        let geometry = match &self.geometry {
            AnnotationGeometry::Bounded { kind, bounds } => {
                AnnotationGeometry::Bounded {
                    kind: *kind,
                    bounds: Rect::new(
                        bounds.x + dx,
                        bounds.y + dy,
                        bounds.width,
                        bounds.height,
                    ),
                }
            }
            AnnotationGeometry::Linear { kind, start, end } => {
                AnnotationGeometry::Linear {
                    kind: *kind,
                    start: Point::new(start.x + dx, start.y + dy),
                    end: Point::new(end.x + dx, end.y + dy),
                }
            }
            AnnotationGeometry::Ink { kind, points } => {
                AnnotationGeometry::Ink {
                    kind: *kind,
                    points: points
                        .iter()
                        .map(|point| Point::new(point.x + dx, point.y + dy))
                        .collect(),
                }
            }
        };
        Self {
            geometry,
            stroke_width: self.stroke_width,
            color: self.color,
            filled: self.filled,
            rotation: self.rotation,
        }
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    shapes: Vec<ShapeAnnotation>,
    selected: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct Annotations {
    tool: Tool,
    styles: ToolStyles,
    shapes: Vec<ShapeAnnotation>,
    draft: Option<Draft>,
    selected: Vec<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}
