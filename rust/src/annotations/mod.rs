mod geometry;
mod interaction;
mod model;
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
    ROTATION_HANDLE_OFFSET, ResizeCorner, ShapeAnnotation, ShapeKind,
};
use text::TextDraft;
use text::TextMoveDraft;
pub use text::{
    TEXT_EDITOR_HORIZONTAL_PADDING, TEXT_EDITOR_VERTICAL_PADDING,
    TEXT_LINE_HEIGHT, TextAnnotation, TextEditHit, TextFormat, TextTypeface,
};
pub use tool::Tool;
use tool::{CreationKind, ToolStyles};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layer {
    Shape(usize),
    Text(usize),
}

#[derive(Clone, Copy, Debug)]
pub enum AnnotationRef<'a> {
    Shape(usize, &'a ShapeAnnotation),
    Text(usize, &'a TextAnnotation),
}

#[derive(Clone, Debug)]
pub enum OwnedAnnotation {
    Shape(ShapeAnnotation),
    Text(TextAnnotation),
}

impl OwnedAnnotation {
    /// Borrow content for rendering; the index is a placeholder, always zero.
    #[must_use]
    pub const fn as_ref(&self) -> AnnotationRef<'_> {
        match self {
            Self::Shape(shape) => AnnotationRef::Shape(0, shape),
            Self::Text(text) => AnnotationRef::Text(0, text),
        }
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    revision: u64,
    shapes: Vec<ShapeAnnotation>,
    texts: Vec<TextAnnotation>,
    layers: Vec<Layer>,
    selected: Vec<usize>,
    selected_texts: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct Annotations {
    revision: u64,
    next_revision: u64,
    tool: Tool,
    styles: ToolStyles,
    shapes: Vec<ShapeAnnotation>,
    texts: Vec<TextAnnotation>,
    layers: Vec<Layer>,
    text_format: TextFormat,
    draft: Option<Draft>,
    text_draft: Option<TextDraft>,
    text_move: Option<TextMoveDraft>,
    selected: Vec<usize>,
    selected_texts: Vec<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

#[derive(Clone, Debug)]
pub struct AnnotationSettings {
    tool: Tool,
    styles: ToolStyles,
    text_format: TextFormat,
}
