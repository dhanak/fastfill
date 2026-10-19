mod geometry;
mod interaction;
mod marquee;
mod model;
mod page;
mod selection;
mod state;
mod style;
mod text;
mod tool;

pub use geometry::arrowhead_points;
use interaction::{
    Draft, EditDraft, GroupMoveDraft, SelectionDraft, ShapeDraft,
};
pub use model::{
    AnnotationColor, AnnotationGeometry, AnnotationStyle, DragModifiers,
    EditHit, InkKind, LineEndpoint, LineKind, Pointer, RESIZE_CORNERS,
    ROTATION_HANDLE_OFFSET, ResizeCorner, SELECTION_HANDLE_SIZE,
    SELECTION_STROKE, ShapeAnnotation, ShapeKind,
};
pub use page::PageChange;
use text::TextDraft;
use text::TextMoveDraft;
pub use text::{
    TEXT_EDITOR_HORIZONTAL_PADDING, TEXT_EDITOR_VERTICAL_PADDING,
    TEXT_LINE_HEIGHT, TextAnnotation, TextEditHit, TextFormat, TextTypeface,
};
pub use tool::Tool;
use tool::{CreationKind, ToolStyles};

#[derive(Clone, Copy, Debug)]
/// Borrowed content with its current layer index, never a persistent identity.
pub enum AnnotationRef<'a> {
    Shape(usize, &'a ShapeAnnotation),
    Text(usize, &'a TextAnnotation),
}

impl AnnotationRef<'_> {
    #[must_use]
    pub const fn id(self) -> uuid::Uuid {
        match self {
            Self::Shape(_, shape) => shape.id,
            Self::Text(_, text) => text.id,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OwnedAnnotation {
    Shape(ShapeAnnotation),
    Text(TextAnnotation),
}

impl OwnedAnnotation {
    #[must_use]
    pub const fn id(&self) -> uuid::Uuid {
        self.as_ref().id()
    }

    #[must_use]
    pub const fn shape(&self) -> Option<&ShapeAnnotation> {
        match self {
            Self::Shape(shape) => Some(shape),
            Self::Text(_) => None,
        }
    }

    #[must_use]
    pub const fn text(&self) -> Option<&TextAnnotation> {
        match self {
            Self::Text(text) => Some(text),
            Self::Shape(_) => None,
        }
    }

    fn shape_mut(&mut self) -> Option<&mut ShapeAnnotation> {
        match self {
            Self::Shape(shape) => Some(shape),
            Self::Text(_) => None,
        }
    }

    fn text_mut(&mut self) -> Option<&mut TextAnnotation> {
        match self {
            Self::Text(text) => Some(text),
            Self::Shape(_) => None,
        }
    }

    fn contains(&self, point: crate::geometry::Point, tolerance: f32) -> bool {
        match self {
            Self::Shape(shape) => {
                geometry::annotation_contains(shape, point, tolerance)
            }
            Self::Text(text) => {
                geometry::contains(text.bounds, text.unrotated_point(point))
            }
        }
    }

    fn hits(&self, bounds: crate::geometry::Rect) -> bool {
        match self {
            Self::Shape(shape) => marquee::hits_shape(bounds, shape),
            Self::Text(text) => {
                marquee::hits_frame(bounds, &text.frame_corners())
            }
        }
    }

    fn visible_bounds(&self) -> crate::geometry::Rect {
        match self {
            Self::Shape(shape) => shape.visible_bounds(),
            Self::Text(text) => text.visible_bounds(),
        }
    }

    fn translated(&self, dx: f32, dy: f32) -> Self {
        match self {
            Self::Shape(shape) => Self::Shape(shape.translated(dx, dy)),
            Self::Text(text) => Self::Text(text.translated(dx, dy)),
        }
    }

    fn tool(&self) -> Tool {
        match self {
            Self::Shape(shape) => shape.tool(),
            Self::Text(_) => Tool::Text,
        }
    }

    fn style(&self) -> AnnotationStyle {
        match self {
            Self::Shape(shape) => shape.style(),
            Self::Text(text) => AnnotationStyle {
                color: text.format.color,
                ..AnnotationStyle::default()
            },
        }
    }

    fn set_style(&mut self, style: AnnotationStyle) {
        match self {
            Self::Shape(shape) => {
                shape.color = style.color;
                shape.stroke_width = style.stroke_width;
                shape.filled = style.filled;
            }
            Self::Text(text) => text.format.color = style.color,
        }
    }

    /// Compare content, not transient shape/text indices in a borrowed view.
    #[must_use]
    pub fn matches(&self, annotation: AnnotationRef<'_>) -> bool {
        match (self, annotation) {
            (Self::Shape(old), AnnotationRef::Shape(_, shape)) => old == shape,
            (Self::Text(old), AnnotationRef::Text(_, text)) => old == text,
            _ => false,
        }
    }

    /// Borrow content for rendering; the index is a placeholder, always zero.
    #[must_use]
    pub const fn as_ref(&self) -> AnnotationRef<'_> {
        match self {
            Self::Shape(shape) => AnnotationRef::Shape(0, shape),
            Self::Text(text) => AnnotationRef::Text(0, text),
        }
    }
}

impl From<AnnotationRef<'_>> for OwnedAnnotation {
    fn from(annotation: AnnotationRef<'_>) -> Self {
        match annotation {
            AnnotationRef::Shape(_, shape) => Self::Shape(shape.clone()),
            AnnotationRef::Text(_, text) => Self::Text(text.clone()),
        }
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    revision: u64,
    page_context: u64,
    content: Vec<OwnedAnnotation>,
    selected: Vec<uuid::Uuid>,
}

#[derive(Debug, Default)]
pub struct Annotations {
    revision: u64,
    next_revision: u64,
    page_context: u64,
    tool: Tool,
    styles: ToolStyles,
    content: Vec<OwnedAnnotation>,
    text_format: TextFormat,
    draft: Option<Draft>,
    text_draft: Option<TextDraft>,
    selected: Vec<uuid::Uuid>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

#[derive(Clone, Debug)]
pub struct AnnotationSettings {
    tool: Tool,
    styles: ToolStyles,
    text_format: TextFormat,
}
