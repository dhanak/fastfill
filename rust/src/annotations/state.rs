use super::geometry::{
    annotation_contains, contains, distance, line_hit_radius, valid,
    valid_annotation_size, valid_scale,
};
use super::{
    AnnotationColor, AnnotationGeometry, AnnotationStyle, Annotations, Draft,
    DragModifiers, EditDraft, EditHit, LineEndpoint, Pointer, RESIZE_CORNERS,
    ShapeAnnotation, ShapeDraft, Snapshot, Tool,
};
use crate::geometry::{Point, Rect, Size};

impl Annotations {
    #[must_use]
    pub const fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.cancel();
        self.tool = tool;
    }

    #[must_use]
    pub fn current_style(&self) -> Option<AnnotationStyle> {
        if let Some(index) = self.style_target_index() {
            let annotation = self.shapes.get(index)?;
            return Some(annotation.style());
        }
        self.styles.get(self.tool)
    }

    #[must_use]
    pub fn fill_available(&self) -> bool {
        if let Some(index) = self.style_target_index() {
            return self
                .shapes
                .get(index)
                .is_some_and(ShapeAnnotation::supports_fill);
        }
        self.tool.supports_fill()
    }

    pub fn set_color(&mut self, color: AnnotationColor) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let target = self.style_target_index();
        let object_changed =
            target.is_some_and(|index| self.shapes[index].color != color);
        let default_changed = self
            .styles
            .get(self.tool)
            .is_some_and(|style| style.color != color);
        if !object_changed && !default_changed {
            return false;
        }
        if let Some(index) = target.filter(|_| object_changed) {
            self.record_change();
            self.shapes[index].color = color;
        }
        if let Some(style) = self.styles.get_mut(self.tool) {
            style.color = color;
        }
        true
    }

    pub fn set_stroke_width(&mut self, stroke_width: f32) -> bool {
        if self.draft.is_some()
            || !stroke_width.is_finite()
            || !(1.0..=20.0).contains(&stroke_width)
        {
            return false;
        }
        let target = self.style_target_index();
        let object_changed = target.is_some_and(|index| {
            (self.shapes[index].stroke_width - stroke_width).abs()
                > f32::EPSILON
        });
        let default_changed = self.styles.get(self.tool).is_some_and(|style| {
            (style.stroke_width - stroke_width).abs() > f32::EPSILON
        });
        if !object_changed && !default_changed {
            return false;
        }
        if let Some(index) = target.filter(|_| object_changed) {
            self.record_change();
            self.shapes[index].stroke_width = stroke_width;
        }
        if let Some(style) = self.styles.get_mut(self.tool) {
            style.stroke_width = stroke_width;
        }
        true
    }

    pub fn set_filled(&mut self, filled: bool) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let target = self.style_target_index();
        if let Some(index) = target
            && !self.shapes[index].supports_fill()
        {
            return false;
        }
        if self.tool == Tool::Select {
            let Some(index) = target else {
                return false;
            };
            if self.shapes[index].filled == filled {
                return false;
            }
            self.record_change();
            self.shapes[index].filled = filled;
            return true;
        }
        if !self.tool.supports_fill() {
            return false;
        }
        let object_changed =
            target.is_some_and(|index| self.shapes[index].filled != filled);
        let default_changed = self
            .styles
            .get(self.tool)
            .is_some_and(|style| style.filled != filled);
        if !object_changed && !default_changed {
            return false;
        }
        if let Some(index) = target.filter(|_| object_changed) {
            self.record_change();
            self.shapes[index].filled = filled;
        }
        if let Some(style) = self.styles.get_mut(self.tool) {
            style.filled = filled;
        }
        true
    }

    fn style_target_index(&self) -> Option<usize> {
        let index = self.selected?;
        let annotation = self.shapes.get(index)?;
        (self.tool == Tool::Select
            || self
                .tool
                .creation_kind()
                .is_some_and(|kind| kind.matches(annotation)))
        .then_some(index)
    }

    #[must_use]
    pub fn shapes(&self) -> &[ShapeAnnotation] {
        &self.shapes
    }

    #[must_use]
    pub fn preview(&self) -> Option<ShapeAnnotation> {
        self.draft.as_ref().map(Draft::annotation)
    }

    #[must_use]
    pub fn active_pointer(&self) -> Option<Pointer> {
        self.draft.as_ref().map(Draft::pointer)
    }

    #[must_use]
    pub const fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.draft.is_none() && !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.draft.is_none() && !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.restore(snapshot);
        self.redo.push(current);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.restore(snapshot);
        self.undo.push(current);
        true
    }

    #[must_use]
    pub fn can_reset_rotation(&self) -> bool {
        self.draft.is_none()
            && self.selected.is_some_and(|index| {
                self.shapes.get(index).is_some_and(|annotation| {
                    annotation.supports_rotation()
                        && annotation.rotation.abs() > f32::EPSILON
                })
            })
    }

    pub fn reset_rotation(&mut self) -> bool {
        if !self.can_reset_rotation() {
            return false;
        }
        let Some(index) = self.selected else {
            return false;
        };
        self.record_change();
        self.shapes[index].rotation = 0.0;
        true
    }

    #[must_use]
    pub fn edited_index(&self) -> Option<usize> {
        match &self.draft {
            Some(Draft::Edit(draft)) => Some(draft.index),
            _ => None,
        }
    }

    #[must_use]
    pub fn selection_annotation(&self, scale: f32) -> Option<ShapeAnnotation> {
        if !valid_scale(scale) || matches!(self.draft, Some(Draft::Create(_))) {
            return None;
        }
        let index = self.selected?;
        let annotation = if self.edited_index() == Some(index) {
            self.preview()?
        } else {
            self.shapes.get(index)?.clone()
        };
        Some(annotation)
    }

    #[must_use]
    pub fn selection_bounds(&self, scale: f32) -> Option<Rect> {
        self.selection_annotation(scale)
            .map(|annotation| annotation.bounds())
    }

    #[must_use]
    pub fn hit_selected(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
        modifiers: DragModifiers,
    ) -> Option<EditHit> {
        if !valid(point) {
            return None;
        }
        let index = self.selected?;
        let annotation = if self.edited_index() == Some(index) {
            self.preview()?
        } else {
            self.shapes.get(index)?.clone()
        };
        let controls = annotation.bounds();
        let radius = match pointer {
            Pointer::Mouse => 12.0,
            Pointer::Touch(_) => 22.0,
        } / scale;
        if annotation
            .rotation_handle(scale)
            .is_some_and(|handle| distance(point, handle) <= radius)
        {
            return Some(EditHit::Rotate);
        }
        if let AnnotationGeometry::Linear { start, end, .. } =
            annotation.geometry
        {
            let (endpoint, endpoint_distance) =
                if distance(point, start) <= distance(point, end) {
                    (LineEndpoint::Start, distance(point, start))
                } else {
                    (LineEndpoint::End, distance(point, end))
                };
            let near_line = annotation_contains(
                &annotation,
                point,
                line_hit_radius(pointer) / scale,
            );
            let inside = contains(controls, point);
            if endpoint_distance > radius && !near_line && !inside {
                return None;
            }
            return if modifiers.force_move {
                Some(EditHit::Move)
            } else if modifiers.force_resize || endpoint_distance <= radius {
                Some(EditHit::Endpoint(endpoint))
            } else {
                Some(EditHit::Move)
            };
        }
        let nearest = RESIZE_CORNERS.into_iter().min_by(|a, b| {
            distance(point, annotation.rotated_point(a.position(controls)))
                .total_cmp(&distance(
                    point,
                    annotation.rotated_point(b.position(controls)),
                ))
        })?;
        let corner = annotation.rotated_point(nearest.position(controls));
        let corner_distance = distance(point, corner);
        let inside = contains(controls, annotation.unrotated_point(point));
        let near_handle = corner_distance <= radius;
        if !inside && !near_handle {
            return None;
        }
        if modifiers.force_move {
            Some(EditHit::Move)
        } else if modifiers.force_resize || !inside {
            Some(EditHit::Resize(nearest))
        } else {
            let inner_radius =
                (7.0 / scale).min(controls.width.min(controls.height) / 4.0);
            if corner_distance <= inner_radius {
                Some(EditHit::Resize(nearest))
            } else {
                Some(EditHit::Move)
            }
        }
    }

    pub fn begin_interaction(
        &mut self,
        pointer: Pointer,
        point: Point,
        page: Size,
        scale: f32,
        modifiers: DragModifiers,
    ) -> bool {
        if self.draft.is_some()
            || !valid(point)
            || !valid_scale(scale)
            || !page.width.is_finite()
            || !page.height.is_finite()
            || page.width <= 0.0
            || page.height <= 0.0
            || self.tool == Tool::Navigate
        {
            return false;
        }
        let hit = self.hit_selected(point, pointer, scale, modifiers);
        // Handles take priority; otherwise Select picks the topmost full box.
        let handle_hit = matches!(
            hit,
            Some(EditHit::Resize(_) | EditHit::Endpoint(_) | EditHit::Rotate)
        );
        let forced_move = modifiers.force_move && hit.is_some();
        if self.tool == Tool::Select && !handle_hit && !forced_move {
            let tolerance = line_hit_radius(pointer) / scale;
            let selected = self.shapes.iter().rposition(|object| {
                annotation_contains(object, point, tolerance)
            });
            if selected.is_some() || hit.is_none() {
                self.selected = selected;
            }
        }
        let hit = self.hit_selected(point, pointer, scale, modifiers);
        if let (Some(index), Some(hit)) = (self.selected, hit) {
            self.draft = Some(Draft::Edit(EditDraft {
                pointer,
                index,
                original: self.shapes[index].clone(),
                start: point,
                end: point,
                square: modifiers.square,
                hit,
                page,
            }));
            return true;
        }
        if self.tool.creation_kind().is_some() {
            return self.begin(pointer, point, page);
        }
        false
    }

    pub fn delete_selected(&mut self) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let Some(index) = self.selected else {
            return false;
        };
        self.record_change();
        self.shapes.remove(index);
        self.selected = None;
        true
    }

    pub fn begin(
        &mut self,
        pointer: Pointer,
        point: Point,
        page: Size,
    ) -> bool {
        let Some(kind) = self.tool.creation_kind() else {
            return false;
        };
        if self.draft.is_some()
            || !valid(point)
            || !page.width.is_finite()
            || !page.height.is_finite()
            || point.x < 0.0
            || point.y < 0.0
            || point.x >= page.width
            || point.y >= page.height
        {
            return false;
        }
        self.draft = Some(Draft::Create(ShapeDraft {
            pointer,
            kind,
            start: point,
            end: point,
            square: false,
            page,
            style: self.styles.get(self.tool).unwrap_or_default(),
            points: vec![point],
        }));
        true
    }

    pub fn drag(&mut self, pointer: Pointer, point: Point, square: bool) {
        if let Some(draft) = &mut self.draft
            && draft.pointer() == pointer
            && valid(point)
        {
            draft.update(point, square);
        }
    }

    pub fn constrain_square(&mut self, square: bool) {
        if let Some(draft) = &mut self.draft {
            draft.constrain_square(square);
        }
    }

    pub fn finish(
        &mut self,
        pointer: Pointer,
        point: Point,
        square: bool,
    ) -> bool {
        self.drag(pointer, point, square);
        let Some(draft) =
            self.draft.take_if(|draft| draft.pointer() == pointer)
        else {
            return false;
        };
        let annotation = draft.annotation();
        let bounds = annotation.bounds();
        if !valid(point)
            || !valid(Point::new(bounds.x, bounds.y))
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || !valid_annotation_size(&annotation)
            || bounds.x + bounds.width <= 0.0
            || bounds.y + bounds.height <= 0.0
        {
            return false;
        }
        match draft {
            Draft::Create(_) => {
                self.record_change();
                self.shapes.push(annotation);
                self.selected = Some(self.shapes.len() - 1);
            }
            Draft::Edit(draft) => {
                if self.shapes[draft.index] != annotation {
                    self.record_change();
                    self.shapes[draft.index] = annotation;
                }
            }
        }
        true
    }

    pub fn cancel(&mut self) {
        self.draft = None;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            shapes: self.shapes.clone(),
            selected: self.selected,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.shapes = snapshot.shapes;
        self.selected = snapshot.selected;
    }

    fn record_change(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
    }
}
