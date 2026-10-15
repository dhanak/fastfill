use super::geometry::{
    angle_from, contains, distance, normalized_rotation, point_bounds,
    rotate_point, valid, valid_scale,
};
use super::{Annotations, Layer, Pointer, Tool};
use crate::geometry::{Point, Rect, Size};
use std::fmt;

const DEFAULT_FONT_SIZE: f32 = 24.0;
const MIN_CONTENT_WIDTH: f32 = 48.0;
pub const TEXT_LINE_HEIGHT: f32 = 1.3;
pub const TEXT_EDITOR_HORIZONTAL_PADDING: f32 = 6.0;
pub const TEXT_EDITOR_VERTICAL_PADDING: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextTypeface {
    #[default]
    Sans,
    Serif,
    Monospace,
}

impl TextTypeface {
    pub const ALL: [Self; 3] = [Self::Sans, Self::Serif, Self::Monospace];
}

impl fmt::Display for TextTypeface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Sans => "Sans",
            Self::Serif => "Serif",
            Self::Monospace => "Monospace",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFormat {
    pub typeface: TextTypeface,
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: super::AnnotationColor,
}

impl Default for TextFormat {
    fn default() -> Self {
        Self {
            typeface: TextTypeface::Sans,
            font_size: DEFAULT_FONT_SIZE,
            bold: false,
            italic: false,
            underline: false,
            color: super::AnnotationColor::Black,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditHit {
    Move,
    Rotate,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextAnnotation {
    pub id: uuid::Uuid,
    pub text: String,
    pub bounds: Rect,
    pub format: TextFormat,
    pub rotation: f32,
}

impl TextAnnotation {
    pub(super) fn translated(&self, dx: f32, dy: f32) -> Self {
        let mut annotation = self.clone();
        annotation.bounds.x += dx;
        annotation.bounds.y += dy;
        annotation
    }

    #[must_use]
    pub fn frame_corners(&self) -> [Point; 4] {
        let bounds = self.bounds;
        [
            Point::new(bounds.x, bounds.y),
            Point::new(bounds.x + bounds.width, bounds.y),
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
            Point::new(bounds.x, bounds.y + bounds.height),
        ]
        .map(|point| self.rotated_point(point))
    }

    #[must_use]
    pub fn rotation_handle(&self, scale: f32) -> Option<Point> {
        if !valid_scale(scale) {
            return None;
        }
        let center = self.bounds.center();
        Some(self.rotated_point(Point::new(
            center.x,
            self.bounds.y - super::ROTATION_HANDLE_OFFSET / scale,
        )))
    }

    #[must_use]
    pub fn rotated_point(&self, point: Point) -> Point {
        rotate_point(point, self.bounds.center(), self.rotation)
    }

    pub(super) fn unrotated_point(&self, point: Point) -> Point {
        rotate_point(point, self.bounds.center(), -self.rotation)
    }

    pub(super) fn visible_bounds(&self) -> Rect {
        point_bounds(&self.frame_corners())
    }
}

#[derive(Debug)]
pub(super) struct TextDraft {
    annotation: TextAnnotation,
    index: Option<usize>,
    scale: f32,
}

#[derive(Debug)]
pub(super) struct TextMoveDraft {
    pub(super) pointer: Pointer,
    index: usize,
    original: TextAnnotation,
    start: Point,
    end: Point,
    page: Size,
    hit: TextEditHit,
    pub(super) snap: bool,
}

impl TextMoveDraft {
    fn annotation(&self) -> TextAnnotation {
        if self.hit == TextEditHit::Rotate {
            return self.rotated();
        }
        let bounds = self.original.visible_bounds();
        let visible_x = 1.0_f32.min(bounds.width).min(self.page.width);
        let visible_y = 1.0_f32.min(bounds.height).min(self.page.height);
        let dx = (self.end.x - self.start.x).clamp(
            visible_x - bounds.width - bounds.x,
            self.page.width - visible_x - bounds.x,
        );
        let dy = (self.end.y - self.start.y).clamp(
            visible_y - bounds.height - bounds.y,
            self.page.height - visible_y - bounds.y,
        );
        self.original.translated(dx, dy)
    }

    fn rotated(&self) -> TextAnnotation {
        let center = self.original.bounds.center();
        let start = angle_from(center, self.start);
        let end = angle_from(center, self.end);
        let mut rotation = normalized_rotation(
            self.original.rotation + normalized_rotation(end - start),
        );
        if self.snap {
            let step = std::f32::consts::FRAC_PI_4;
            rotation = normalized_rotation((rotation / step).round() * step);
        }
        let mut annotation = self.original.clone();
        annotation.rotation = rotation;
        annotation
    }
}

impl Annotations {
    #[must_use]
    pub fn texts(&self) -> &[TextAnnotation] {
        &self.texts
    }

    #[must_use]
    pub fn text_preview(&self) -> Option<TextAnnotation> {
        self.text_draft
            .as_ref()
            .map(|draft| draft.annotation.clone())
    }

    #[must_use]
    pub fn text_move_preview(&self) -> Option<TextAnnotation> {
        self.text_move.as_ref().map(TextMoveDraft::annotation)
    }

    #[must_use]
    pub fn text_move_previews(&self) -> Vec<TextAnnotation> {
        match &self.draft {
            Some(super::Draft::GroupMove(draft)) => {
                draft.texts().into_iter().map(|(_, text)| text).collect()
            }
            _ => self.text_move_preview().into_iter().collect(),
        }
    }

    #[must_use]
    pub fn selection_texts(&self) -> Vec<TextAnnotation> {
        let previews = self.text_move_previews();
        self.selected_texts
            .iter()
            .filter_map(|index| {
                self.texts.get(*index).map(|text| {
                    previews
                        .iter()
                        .find(|preview| preview.id == text.id)
                        .unwrap_or(text)
                        .clone()
                })
            })
            .collect()
    }

    pub(super) fn selected_text_index(&self) -> Option<usize> {
        match (self.selected.as_slice(), self.selected_texts.as_slice()) {
            ([], [index]) => Some(*index),
            _ => None,
        }
    }

    #[must_use]
    pub fn text_edit(&self) -> Option<(&str, Rect, TextFormat, f32)> {
        self.text_draft.as_ref().map(|draft| {
            (
                draft.annotation.text.as_str(),
                draft.annotation.bounds,
                draft.annotation.format,
                draft.scale,
            )
        })
    }

    #[must_use]
    pub const fn has_text_edit(&self) -> bool {
        self.text_draft.is_some()
    }

    #[must_use]
    pub fn text_hit(&self, point: Point) -> Option<usize> {
        if !valid(point) {
            return None;
        }
        self.layers.iter().rev().find_map(|layer| match *layer {
            Layer::Text(index)
                if self.texts.get(index).is_some_and(|annotation| {
                    contains(
                        annotation.bounds,
                        annotation.unrotated_point(point),
                    )
                }) =>
            {
                Some(index)
            }
            Layer::Shape(_) | Layer::Text(_) => None,
        })
    }

    #[must_use]
    pub fn selected_text_annotation(&self) -> Option<TextAnnotation> {
        if let Some(draft) = &self.text_move {
            return Some(draft.annotation());
        }
        self.selected_text_index()
            .and_then(|index| self.texts.get(index))
            .cloned()
    }

    #[must_use]
    pub fn hit_selected_text(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
    ) -> Option<TextEditHit> {
        if !valid(point) || !valid_scale(scale) {
            return None;
        }
        let annotation = self.selected_text_annotation()?;
        let radius = match pointer {
            Pointer::Mouse => 12.0,
            Pointer::Touch(_) => 22.0,
        } / scale;
        if annotation
            .rotation_handle(scale)
            .is_some_and(|handle| distance(point, handle) <= radius)
        {
            return Some(TextEditHit::Rotate);
        }
        contains(annotation.bounds, annotation.unrotated_point(point))
            .then_some(TextEditHit::Move)
    }

    #[must_use]
    pub fn hit_text_interaction(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
    ) -> Option<TextEditHit> {
        self.text_interaction_target(point, pointer, scale)
            .map(|(_, hit)| hit)
    }

    #[must_use]
    pub fn current_text_format(&self) -> Option<TextFormat> {
        self.text_draft
            .as_ref()
            .map(|draft| draft.annotation.format)
            .or_else(|| {
                let index = self.selected_texts.first()?;
                let mut format = self.texts.get(*index)?.format;
                // A toggle is active only when every selected text has it.
                format.bold = self
                    .selected_texts
                    .iter()
                    .all(|index| self.texts[*index].format.bold);
                format.italic = self
                    .selected_texts
                    .iter()
                    .all(|index| self.texts[*index].format.italic);
                format.underline = self
                    .selected_texts
                    .iter()
                    .all(|index| self.texts[*index].format.underline);
                Some(format)
            })
            .or_else(|| (self.tool == Tool::Text).then_some(self.text_format))
    }

    pub fn set_text_format(&mut self, format: TextFormat) -> bool {
        if !valid_text_format(format) {
            return false;
        }
        if let Some(draft) = &mut self.text_draft {
            if draft.annotation.format == format {
                return false;
            }
            draft.annotation.format = format;
            return true;
        }
        if self.tool != Tool::Text || self.text_format == format {
            return false;
        }
        self.text_format = format;
        true
    }

    pub fn set_selected_text_format(
        &mut self,
        format: TextFormat,
        size: Size,
    ) -> bool {
        if self.selected_text_index().is_none() {
            return false;
        }
        self.update_selected_text_formats(|_| (format, size))
    }

    /// Apply measured text formats atomically, preserving every shape and
    /// unrelated text property. Invalid results or active drags change nothing.
    /// All selected texts form one undo step, not one step per object.
    pub fn update_selected_text_formats(
        &mut self,
        mut update: impl FnMut(&TextAnnotation) -> (TextFormat, Size),
    ) -> bool {
        if self.draft.is_some()
            || self.text_draft.is_some()
            || self.text_move.is_some()
            || self.selected_texts.is_empty()
        {
            return false;
        }
        let mut updates = Vec::with_capacity(self.selected_texts.len());
        for index in &self.selected_texts {
            let original = &self.texts[*index];
            let (format, size) = update(original);
            if !valid_text_format(format) || !valid_text_size(size) {
                return false;
            }
            let mut annotation = original.clone();
            annotation.format = format;
            annotation.bounds.width = size.width;
            annotation.bounds.height = size.height;
            updates.push((*index, annotation));
        }
        if updates
            .iter()
            .all(|(index, text)| *text == self.texts[*index])
        {
            return false;
        }
        self.record_change();
        for (index, annotation) in updates {
            self.text_format = annotation.format;
            self.texts[index] = annotation;
        }
        true
    }

    #[must_use]
    pub fn is_text_edited(&self, index: usize) -> bool {
        self.text_draft
            .as_ref()
            .and_then(|draft| draft.index)
            .is_some_and(|edited| edited == index)
            || self
                .text_move
                .as_ref()
                .is_some_and(|draft| draft.index == index)
            || matches!(&self.draft, Some(super::Draft::GroupMove(draft))
                if draft.original_texts.iter().any(|(i, _)| *i == index))
    }

    pub fn begin_text(&mut self, point: Point, page: Size, scale: f32) -> bool {
        if !self.can_begin_text(point, page, scale) {
            return false;
        }
        self.selected.clear();
        self.selected_texts.clear();
        self.text_draft = Some(TextDraft {
            annotation: TextAnnotation {
                id: uuid::Uuid::new_v4(),
                text: String::new(),
                bounds: text_placement(point, self.text_format.font_size),
                format: self.text_format,
                rotation: 0.0,
            },
            index: None,
            scale,
        });
        true
    }

    pub fn begin_text_edit(
        &mut self,
        point: Point,
        page: Size,
        scale: f32,
    ) -> bool {
        if !self.can_begin_text(point, page, scale) {
            return false;
        }
        let Some(index) = self.text_hit(point) else {
            return false;
        };
        self.begin_existing_text(index, scale)
    }

    pub fn begin_text_move(
        &mut self,
        pointer: Pointer,
        point: Point,
        page: Size,
        scale: f32,
    ) -> bool {
        if self.tool != Tool::Select
            || self.draft.is_some()
            || self.text_draft.is_some()
            || self.text_move.is_some()
            || !valid(point)
            || !valid_scale(scale)
        {
            return false;
        }
        let Some((index, hit)) =
            self.text_interaction_target(point, pointer, scale)
        else {
            return false;
        };
        self.selected.clear();
        self.selected_texts = vec![index];
        self.text_move = Some(TextMoveDraft {
            pointer,
            index,
            original: self.texts[index].clone(),
            start: point,
            end: point,
            page,
            hit,
            snap: false,
        });
        true
    }

    pub fn update_text(&mut self, text: impl Into<String>, size: Size) {
        if !size.width.is_finite()
            || !size.height.is_finite()
            || size.width <= 0.0
            || size.height <= 0.0
        {
            return;
        }
        if let Some(draft) = &mut self.text_draft {
            draft.annotation.text = text.into();
            draft.annotation.bounds.width = size.width;
            draft.annotation.bounds.height = size.height;
        }
    }

    pub fn accept_text(&mut self) -> bool {
        let Some(mut draft) = self.text_draft.take() else {
            return false;
        };
        let text = draft.annotation.text.trim().to_owned();
        draft.annotation.text = text;
        if draft.annotation.text.is_empty() {
            if let Some(index) = draft.index {
                self.record_change();
                self.remove_text_layer(index);
                self.texts.remove(index);
                self.selected_texts.clear();
            }
            return true;
        }
        let format = draft.annotation.format;
        match draft.index {
            Some(index) if self.texts[index] != draft.annotation => {
                self.record_change();
                self.texts[index] = draft.annotation;
                self.selected_texts = vec![index];
            }
            Some(index) => self.selected_texts = vec![index],
            None => {
                self.record_change();
                self.texts.push(draft.annotation);
                self.layers.push(super::Layer::Text(self.texts.len() - 1));
                self.selected_texts = vec![self.texts.len() - 1];
            }
        }
        self.text_format = format;
        true
    }

    pub(super) fn drag_text_move(
        &mut self,
        pointer: Pointer,
        point: Point,
        snap: bool,
    ) -> bool {
        let Some(draft) = &mut self.text_move else {
            return false;
        };
        if draft.pointer == pointer && valid(point) {
            draft.end = point;
            draft.snap = snap;
        }
        true
    }

    pub(super) fn finish_text_move(
        &mut self,
        pointer: Pointer,
        point: Point,
        snap: bool,
    ) -> Option<bool> {
        let draft = self.text_move.as_mut()?;
        if draft.pointer != pointer {
            return Some(false);
        }
        if valid(point) {
            draft.end = point;
            draft.snap = snap;
        }
        let draft = self.text_move.take().expect("text move exists");
        let annotation = draft.annotation();
        if annotation == draft.original {
            return Some(true);
        }
        self.record_change();
        self.texts[draft.index] = annotation;
        Some(true)
    }

    fn can_begin_text(&self, point: Point, page: Size, scale: f32) -> bool {
        matches!(self.tool, Tool::Text | Tool::Select)
            && self.draft.is_none()
            && self.text_draft.is_none()
            && self.text_move.is_none()
            && valid(point)
            && valid_scale(scale)
            && page.width.is_finite()
            && page.height.is_finite()
            && page.width > 0.0
            && page.height > 0.0
            && point.x >= 0.0
            && point.y >= 0.0
            && point.x < page.width
            && point.y < page.height
    }

    fn begin_existing_text(&mut self, index: usize, scale: f32) -> bool {
        let Some(annotation) = self.texts.get(index).cloned() else {
            return false;
        };
        self.selected.clear();
        self.selected_texts = vec![index];
        self.text_draft = Some(TextDraft {
            annotation,
            index: Some(index),
            scale,
        });
        true
    }

    fn text_interaction_target(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
    ) -> Option<(usize, TextEditHit)> {
        if let Some(index) = self.selected_text_index()
            && self.hit_selected_text(point, pointer, scale)
                == Some(TextEditHit::Rotate)
        {
            return Some((index, TextEditHit::Rotate));
        }
        self.topmost_text_hit(point, pointer, scale)
            .map(|index| (index, TextEditHit::Move))
    }

    fn topmost_text_hit(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
    ) -> Option<usize> {
        match self.hit_layer(point, pointer, scale) {
            Some(Layer::Text(index)) => Some(index),
            Some(Layer::Shape(_)) | None => None,
        }
    }
}

fn text_placement(point: Point, font_size: f32) -> Rect {
    let line_height = font_size * TEXT_LINE_HEIGHT;
    Rect::new(
        point.x,
        point.y - line_height / 2.0,
        MIN_CONTENT_WIDTH,
        line_height,
    )
}

fn valid_text_format(format: TextFormat) -> bool {
    format.font_size.is_finite() && (6.0..=144.0).contains(&format.font_size)
}

fn valid_text_size(size: Size) -> bool {
    size.width.is_finite()
        && size.height.is_finite()
        && size.width > 0.0
        && size.height > 0.0
}
