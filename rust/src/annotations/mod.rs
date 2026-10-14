mod geometry;
mod interaction;
mod model;
mod selection;
mod state;
mod style;
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
pub use tool::Tool;
use tool::{CreationKind, ToolStyles};

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
