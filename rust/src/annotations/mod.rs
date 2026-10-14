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
struct Snapshot {
    shapes: Vec<ShapeAnnotation>,
    texts: Vec<TextAnnotation>,
    layers: Vec<Layer>,
    selected: Vec<usize>,
    selected_text: Option<usize>,
}

#[derive(Debug, Default)]
pub struct Annotations {
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
    selected_text: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}
