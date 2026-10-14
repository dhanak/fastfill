use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct NormalizedPoint {
    pub x: f32,
    pub y: f32,
}

impl<'de> Deserialize<'de> for NormalizedPoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct PointData {
            x: f32,
            y: f32,
            #[serde(default)]
            vector: EmptyObject,
        }

        let data = PointData::deserialize(deserializer)?;
        let _ = data.vector;
        Ok(Self {
            x: data.x,
            y: data.y,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl NormalizedRect {
    pub(crate) fn right(self) -> f32 {
        self.x + self.width
    }

    pub(crate) fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub(crate) fn corners(self) -> [NormalizedPoint; 4] {
        [
            NormalizedPoint {
                x: self.x,
                y: self.y,
            },
            NormalizedPoint {
                x: self.right(),
                y: self.y,
            },
            NormalizedPoint {
                x: self.right(),
                y: self.bottom(),
            },
            NormalizedPoint {
                x: self.x,
                y: self.bottom(),
            },
        ]
    }
}

impl<'de> Deserialize<'de> for NormalizedRect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct RectData {
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            #[serde(default)]
            right: Option<f32>,
            #[serde(default)]
            bottom: Option<f32>,
        }

        let data = RectData::deserialize(deserializer)?;
        let _ = (data.right, data.bottom);
        Ok(Self {
            x: data.x,
            y: data.y,
            width: data.width,
            height: data.height,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CropQuad {
    pub top_left: NormalizedPoint,
    pub top_right: NormalizedPoint,
    pub bottom_right: NormalizedPoint,
    pub bottom_left: NormalizedPoint,
}

impl Default for CropQuad {
    fn default() -> Self {
        Self {
            top_left: NormalizedPoint { x: 0.0, y: 0.0 },
            top_right: NormalizedPoint { x: 1.0, y: 0.0 },
            bottom_right: NormalizedPoint { x: 1.0, y: 1.0 },
            bottom_left: NormalizedPoint { x: 0.0, y: 1.0 },
        }
    }
}

impl CropQuad {
    pub(crate) fn points(self) -> [NormalizedPoint; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum PageFilterMode {
    #[default]
    Original = 0,
    EnhancedColor = 1,
    Grayscale = 2,
    BlackAndWhite = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageFilterSettings {
    #[serde(default)]
    pub mode: PageFilterMode,
    #[serde(default)]
    pub contrast: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AffineTransform {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub m31: f32,
    pub m32: f32,
}

impl Default for AffineTransform {
    fn default() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            m31: 0.0,
            m32: 0.0,
        }
    }
}

impl AffineTransform {
    pub(crate) fn apply(self, point: NormalizedPoint) -> NormalizedPoint {
        NormalizedPoint {
            x: point.x * self.m11 + point.y * self.m21 + self.m31,
            y: point.x * self.m12 + point.y * self.m22 + self.m32,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Serialize_repr, Deserialize_repr,
)]
#[repr(u8)]
pub enum ProjectShapeKind {
    #[default]
    Line = 0,
    Arrow = 1,
    Rectangle = 2,
    Ellipse = 3,
    Checkmark = 4,
    Cross = 5,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectAnnotation {
    Freehand {
        #[serde(default)]
        points: Vec<NormalizedPoint>,
        #[serde(default)]
        is_highlighter: bool,
        #[serde(default)]
        filled: bool,
        id: Uuid,
        #[serde(default = "default_color_argb")]
        color_argb: u32,
        #[serde(default = "default_stroke_width")]
        stroke_width: f32,
        #[serde(default)]
        transform: AffineTransform,
    },
    Text {
        #[serde(default)]
        text: String,
        #[serde(default = "default_text_bounds")]
        bounds: NormalizedRect,
        #[serde(default = "default_font_size")]
        font_size: f32,
        #[serde(default = "default_font_family")]
        font_family: String,
        #[serde(default = "default_maximum_width")]
        maximum_width: f32,
        #[serde(default)]
        is_bold: bool,
        #[serde(default)]
        is_italic: bool,
        #[serde(default)]
        is_underlined: bool,
        id: Uuid,
        #[serde(default = "default_color_argb")]
        color_argb: u32,
        #[serde(default = "default_stroke_width")]
        stroke_width: f32,
        #[serde(default)]
        transform: AffineTransform,
    },
    Shape {
        #[serde(default)]
        shape: ProjectShapeKind,
        #[serde(default)]
        start: NormalizedPoint,
        #[serde(default)]
        end: NormalizedPoint,
        #[serde(default)]
        filled: bool,
        id: Uuid,
        #[serde(default = "default_color_argb")]
        color_argb: u32,
        #[serde(default = "default_stroke_width")]
        stroke_width: f32,
        #[serde(default)]
        transform: AffineTransform,
    },
}

impl ProjectAnnotation {
    pub(crate) fn common(&self) -> (Uuid, f32, AffineTransform) {
        match self {
            Self::Freehand {
                id,
                stroke_width,
                transform,
                ..
            }
            | Self::Text {
                id,
                stroke_width,
                transform,
                ..
            }
            | Self::Shape {
                id,
                stroke_width,
                transform,
                ..
            } => (*id, *stroke_width, *transform),
        }
    }

    pub(crate) fn bounds(&self) -> NormalizedRect {
        match self {
            Self::Freehand { points, .. } => point_bounds(points),
            Self::Text { bounds, .. } => *bounds,
            Self::Shape { start, end, .. } => NormalizedRect {
                x: start.x.min(end.x),
                y: start.y.min(end.y),
                width: (start.x - end.x).abs(),
                height: (start.y - end.y).abs(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPage {
    pub id: Uuid,
    pub source_asset: String,
    pub source_pixel_width: i32,
    pub source_pixel_height: i32,
    #[serde(default)]
    pub crop: CropQuad,
    #[serde(default)]
    pub rotation_quarter_turns: i32,
    #[serde(default)]
    pub filter: PageFilterSettings,
    #[serde(default)]
    pub annotations: Vec<ProjectAnnotation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    #[serde(default = "current_schema_version")]
    pub schema_version: i32,
    pub id: Uuid,
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_utc: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_utc: OffsetDateTime,
    #[serde(default)]
    pub pages: Vec<ProjectPage>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyObject {}

fn point_bounds(points: &[NormalizedPoint]) -> NormalizedRect {
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    NormalizedRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

const fn current_schema_version() -> i32 {
    1
}

const fn default_color_argb() -> u32 {
    0xff11_1827
}

const fn default_stroke_width() -> f32 {
    3.0
}

const fn default_text_bounds() -> NormalizedRect {
    NormalizedRect {
        x: 0.1,
        y: 0.1,
        width: 0.3,
        height: 0.1,
    }
}

const fn default_font_size() -> f32 {
    14.0
}

fn default_font_family() -> String {
    "Segoe UI".into()
}

const fn default_maximum_width() -> f32 {
    0.4
}

fn default_title() -> String {
    "Untitled scan".into()
}
