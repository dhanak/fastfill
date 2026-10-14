use crate::appearance::PAGE_MARGIN;
use crate::picker::ImportedImage;
use fastfill_rust::annotations::{AnnotationRef, Annotations, DragModifiers};
use fastfill_rust::geometry::{Navigation, Rect as PageRect, Size as PageSize};
use fastfill_rust::interaction::TouchNavigation;
use iced::{Point, mouse};

mod clipping;
mod program;
mod render;
mod selection;

use render::{draw_annotation, draw_shape, draw_text, draw_text_underline};
use selection::{
    draw_group_member, draw_group_selection, draw_marquee, draw_selection,
    draw_text_selection,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PageCanvas<'a> {
    pub(crate) image: Option<&'a ImportedImage>,
    pub(crate) navigation: Navigation,
    pub(crate) page_size: PageSize,
    pub(crate) annotations: &'a Annotations,
    pub(crate) input_epoch: u64,
    pub(crate) input_enabled: bool,
    pub(crate) modifiers: DragModifiers,
    pub(crate) layer: CanvasLayer<'a>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CanvasLayer<'a> {
    Background,
    Annotation(AnnotationRef<'a>),
    Overlay,
}

#[derive(Default)]
pub(crate) struct CanvasState {
    dragging: Option<mouse::Button>,
    last_position: Option<Point>,
    touch: TouchNavigation,
    input_epoch: u64,
}

pub(crate) fn page_viewport(size: iced::Size) -> PageRect {
    PageRect::new(
        PAGE_MARGIN,
        PAGE_MARGIN,
        (size.width - PAGE_MARGIN * 2.0).max(0.0),
        (size.height - PAGE_MARGIN * 2.0).max(0.0),
    )
}

#[cfg(test)]
mod tests;
