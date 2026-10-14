use crate::geometry::{Point, Rect};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnnotationColor {
    #[default]
    Black,
    White,
    Red,
    Blue,
    Green,
    Yellow,
    Argb(u32),
}

impl AnnotationColor {
    #[must_use]
    pub const fn rgb(self) -> [u8; 3] {
        match self {
            Self::Black => [0, 0, 0],
            Self::White => [255, 255, 255],
            Self::Red => [220, 32, 32],
            Self::Blue => [0, 102, 204],
            Self::Green => [0, 160, 72],
            Self::Yellow => [245, 196, 0],
            Self::Argb(value) => {
                let [_, red, green, blue] = value.to_be_bytes();
                [red, green, blue]
            }
        }
    }

    #[must_use]
    pub const fn to_argb(self) -> u32 {
        if let Self::Argb(value) = self {
            value
        } else {
            let [red, green, blue] = self.rgb();
            u32::from_be_bytes([255, red, green, blue])
        }
    }

    #[must_use]
    pub const fn from_argb(value: u32) -> Self {
        match value {
            0xff00_0000 => Self::Black,
            0xffff_ffff => Self::White,
            0xffdc_2020 => Self::Red,
            0xff00_66cc => Self::Blue,
            0xff00_a048 => Self::Green,
            0xfff5_c400 => Self::Yellow,
            _ => Self::Argb(value),
        }
    }

    #[must_use]
    pub fn opacity(self) -> f32 {
        f32::from(self.to_argb().to_be_bytes()[0]) / 255.0
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Oval,
    Checkmark,
    Cross,
}

impl ShapeKind {
    pub(super) const fn is_mark(self) -> bool {
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

    pub(super) fn opposite(self) -> Self {
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
    pub(super) fn opposite(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeAnnotation {
    pub id: Uuid,
    pub geometry: AnnotationGeometry,
    pub stroke_width: f32,
    pub color: AnnotationColor,
    pub filled: bool,
    pub rotation: f32,
}

impl ShapeAnnotation {
    #[must_use]
    pub fn kind(&self) -> Option<ShapeKind> {
        match &self.geometry {
            AnnotationGeometry::Bounded { kind, .. } => Some(*kind),
            AnnotationGeometry::Linear { .. }
            | AnnotationGeometry::Ink { .. } => None,
        }
    }
}
