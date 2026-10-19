use super::geometry::{
    annotation_contains, contains, distance, line_hit_radius, union_bounds,
    valid, valid_scale,
};
use super::{
    AnnotationGeometry, Annotations, Draft, DragModifiers, EditDraft, EditHit,
    GroupMoveDraft, LineEndpoint, OwnedAnnotation, Pointer, RESIZE_CORNERS,
    SELECTION_STROKE, SelectionDraft, ShapeAnnotation, Tool,
};
use crate::geometry::{Point, Rect, Size};

impl Annotations {
    pub(super) fn hit_layer(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
    ) -> Option<uuid::Uuid> {
        if !valid(point) || !valid_scale(scale) {
            return None;
        }
        let tolerance = line_hit_radius(pointer) / scale;
        self.content.iter().rev().find_map(|annotation| {
            annotation
                .contains(point, tolerance)
                .then_some(annotation.id())
        })
    }

    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        let [id] = self.selected.as_slice() else {
            return None;
        };
        self.content
            .iter()
            .position(|a| a.id() == *id && a.shape().is_some())
    }

    #[must_use]
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected_content()
            .filter_map(|(index, a)| a.shape().map(|_| index))
            .collect()
    }

    #[must_use]
    pub fn selected_text_indices(&self) -> Vec<usize> {
        self.selected_content()
            .filter_map(|(index, a)| a.text().map(|_| index))
            .collect()
    }

    pub(super) fn selected_content(
        &self,
    ) -> impl Iterator<Item = (usize, &OwnedAnnotation)> {
        self.content
            .iter()
            .enumerate()
            .filter(|(_, a)| self.selected.contains(&a.id()))
    }

    #[must_use]
    pub fn selected_ids(&self) -> &[uuid::Uuid] {
        &self.selected
    }

    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selected_count() != 0
    }

    /// Selection is transient; clearing it never changes content or history.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    #[must_use]
    pub fn can_reset_rotation(&self) -> bool {
        !self.has_pending_edit()
            && self.selected_content().any(|(_, a)| match a {
                OwnedAnnotation::Shape(shape) => {
                    shape.supports_rotation()
                        && shape.rotation.abs() > f32::EPSILON
                }
                OwnedAnnotation::Text(text) => {
                    text.rotation.abs() > f32::EPSILON
                }
            })
    }

    pub fn reset_rotation(&mut self) -> bool {
        if !self.can_reset_rotation() {
            return false;
        }
        self.record_change();
        for annotation in &mut self.content {
            if self.selected.contains(&annotation.id()) {
                match annotation {
                    OwnedAnnotation::Shape(shape)
                        if shape.supports_rotation() =>
                    {
                        shape.rotation = 0.0;
                    }
                    OwnedAnnotation::Text(text) => text.rotation = 0.0,
                    OwnedAnnotation::Shape(_) => {}
                }
            }
        }
        true
    }

    #[must_use]
    pub fn edited_index(&self) -> Option<usize> {
        self.draft.as_ref().and_then(Draft::edited_index)
    }

    #[must_use]
    pub fn is_edited(&self, index: usize) -> bool {
        match &self.draft {
            Some(Draft::GroupMove(draft)) => {
                draft.originals.iter().any(|(edited, _)| *edited == index)
            }
            _ => self.edited_index() == Some(index),
        }
    }

    #[must_use]
    pub fn selection_annotation(&self, scale: f32) -> Option<ShapeAnnotation> {
        if !valid_scale(scale) || matches!(self.draft, Some(Draft::Create(_))) {
            return None;
        }
        let index = self.selected_index()?;
        if self.edited_index() == Some(index) {
            self.preview()
        } else {
            self.shape(index).cloned()
        }
    }

    #[must_use]
    pub fn selection_bounds(&self, scale: f32) -> Option<Rect> {
        if !valid_scale(scale) {
            return None;
        }
        self.selected_previews()
            .iter()
            .map(OwnedAnnotation::visible_bounds)
            .reduce(union_bounds)
    }

    #[must_use]
    pub fn selection_annotations(&self, scale: f32) -> Vec<ShapeAnnotation> {
        if !valid_scale(scale) || matches!(self.draft, Some(Draft::Create(_))) {
            return Vec::new();
        }
        self.selected_previews()
            .into_iter()
            .filter_map(|a| match a {
                OwnedAnnotation::Shape(shape) => Some(shape),
                OwnedAnnotation::Text(_) => None,
            })
            .collect()
    }

    pub(super) fn selected_previews(&self) -> Vec<OwnedAnnotation> {
        if matches!(self.draft, Some(Draft::Create(_))) {
            return Vec::new();
        }
        let previews = self.draft_previews();
        self.selected_content()
            .map(|(index, original)| {
                previews
                    .iter()
                    .find(|(edited, _)| *edited == Some(index))
                    .map_or(original, |(_, annotation)| annotation)
                    .clone()
            })
            .collect()
    }

    #[must_use]
    pub fn selection_box(&self) -> Option<Rect> {
        match &self.draft {
            Some(Draft::Selection(draft)) => draft.bounds(),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_selecting(&self) -> bool {
        matches!(self.draft, Some(Draft::Selection(_)))
    }

    #[must_use]
    pub fn hit_selected(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
        modifiers: DragModifiers,
    ) -> Option<EditHit> {
        if !valid(point) || !valid_scale(scale) {
            return None;
        }
        if self.tool == Tool::Select && self.selected_count() > 1 {
            let picked = self.hit_layer(point, pointer, scale);
            return (matches!(self.draft, Some(Draft::GroupMove(_)))
                || self.hit_group(point, picked))
            .then_some(EditHit::Move);
        }
        let index = self.selected_index()?;
        let annotation = if self.edited_index() == Some(index) {
            self.preview()?
        } else {
            self.shape(index)?.clone()
        };
        let controls = annotation.bounds();
        if !annotation.edit_handles_visible(scale) {
            // Include the visible frame stroke and screen/page rounding.
            let margin = SELECTION_STROKE / (2.0 * scale);
            let frame = Rect::new(
                controls.x - margin,
                controls.y - margin,
                controls.width + 2.0 * margin,
                controls.height + 2.0 * margin,
            );
            return (contains(frame, annotation.unrotated_point(point))
                || annotation_contains(
                    &annotation,
                    point,
                    line_hit_radius(pointer) / scale,
                ))
            .then_some(EditHit::Move);
        }
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

    fn hit_group(&self, point: Point, picked: Option<uuid::Uuid>) -> bool {
        picked.map_or_else(
            || {
                self.selected_content().any(|(_, a)| match a {
                    OwnedAnnotation::Shape(shape) => {
                        contains(shape.visible_bounds(), point)
                    }
                    OwnedAnnotation::Text(text) => {
                        contains(text.bounds, text.unrotated_point(point))
                    }
                })
            },
            |id| self.selected.contains(&id),
        )
    }

    pub fn begin_interaction(
        &mut self,
        pointer: Pointer,
        point: Point,
        page: Size,
        scale: f32,
        modifiers: DragModifiers,
    ) -> bool {
        if self.has_pending_edit()
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
        if self.tool == Tool::Select {
            return self.begin_select_interaction(
                pointer, point, page, scale, modifiers, hit,
            );
        }

        if let (Some(index), Some(hit)) = (self.selected_index(), hit) {
            self.start_edit(pointer, index, point, page, modifiers, hit);
            return true;
        }
        if self.tool.creation_kind().is_some() {
            return self.begin(pointer, point, page);
        }
        false
    }

    fn begin_select_interaction(
        &mut self,
        pointer: Pointer,
        point: Point,
        page: Size,
        scale: f32,
        modifiers: DragModifiers,
        hit: Option<EditHit>,
    ) -> bool {
        let force_edit = modifiers.force_move || modifiers.force_resize;
        if modifiers.square && !force_edit {
            let picked = self.hit_layer(point, pointer, scale);
            self.start_selection(pointer, point, scale, picked, true);
            return true;
        }
        // An existing single selection owns its controls/body, even behind
        // another layer. Ctrl above still selects/toggles the topmost layer.
        if let (Some(index), Some(hit)) = (self.selected_index(), hit) {
            self.start_edit(pointer, index, point, page, modifiers, hit);
            return true;
        }

        if self.hit_selected_text(point, pointer, scale).is_some() {
            return self.begin_text_move(pointer, point, page, scale);
        }
        if self.selected_count() > 1 && hit == Some(EditHit::Move) {
            self.start_group_move(pointer, point, page);
            return true;
        }
        let picked = self.hit_layer(point, pointer, scale);
        let index = picked
            .and_then(|id| self.content.iter().position(|a| a.id() == id));
        if let Some(index) = index
            && self.text(index).is_some()
            && (force_edit
                || self.selected_count() == 1
                    && self.selected.contains(&self.content[index].id()))
        {
            return self.begin_text_move(pointer, point, page, scale);
        }
        if let Some(index) = index
            && self.shape(index).is_some()
            && (force_edit || self.selected_index() == Some(index))
        {
            self.selected = vec![self.content[index].id()];
            let hit = self
                .hit_selected(point, pointer, scale, modifiers)
                .unwrap_or(EditHit::Move);
            self.start_edit(pointer, index, point, page, modifiers, hit);
            return true;
        }
        self.start_selection(pointer, point, scale, picked, false);
        true
    }

    fn start_selection(
        &mut self,
        pointer: Pointer,
        point: Point,
        scale: f32,
        picked: Option<uuid::Uuid>,
        toggle: bool,
    ) {
        // Defer clicks until release so movement can instead become a marquee.
        self.draft = Some(Draft::Selection(SelectionDraft {
            start: point,
            end: point,
            pointer,
            // Ignore click/tap jitter in screen pixels, independent of zoom.
            minimum_drag: match pointer {
                Pointer::Mouse => 4.0,
                Pointer::Touch(_) => 8.0,
            } / scale,
            picked,
            toggle,
            dragged: false,
        }));
    }

    pub(super) fn apply_selection_click(
        &mut self,
        picked: Option<uuid::Uuid>,
        toggle: bool,
    ) {
        if !toggle {
            self.selected.clear();
        }
        let Some(id) = picked else { return };
        if let Some(position) =
            self.selected.iter().position(|selected| *selected == id)
        {
            self.selected.remove(position);
        } else {
            self.selected.push(id);
        }
    }

    fn start_group_move(&mut self, pointer: Pointer, point: Point, page: Size) {
        let originals = self
            .selected_content()
            .map(|(index, a)| (index, a.clone()))
            .collect();
        self.draft = Some(Draft::GroupMove(GroupMoveDraft {
            pointer,
            originals,
            start: point,
            end: point,
            page,
        }));
    }

    fn start_edit(
        &mut self,
        pointer: Pointer,
        index: usize,
        point: Point,
        page: Size,
        modifiers: DragModifiers,
        hit: EditHit,
    ) {
        self.draft = Some(Draft::Edit(EditDraft {
            pointer,
            index,
            original: self.shape(index).expect("shape edit target").clone(),
            start: point,
            end: point,
            square: modifiers.square,
            hit,
            page,
        }));
    }

    pub fn delete_selected(&mut self) -> bool {
        if self.has_pending_edit() || !self.has_selection() {
            return false;
        }
        self.record_change();
        self.content.retain(|a| !self.selected.contains(&a.id()));
        self.selected.clear();
        true
    }

    pub fn delete_all(&mut self) -> bool {
        if self.has_pending_edit() || self.is_empty() {
            return false;
        }
        self.record_change();
        self.content.clear();
        self.selected.clear();
        true
    }

    pub fn send_selected_to_back(&mut self) -> bool {
        self.reorder_selected(true)
    }

    pub fn bring_selected_to_front(&mut self) -> bool {
        self.reorder_selected(false)
    }

    fn reorder_selected(&mut self, selected_first: bool) -> bool {
        if self.has_pending_edit() || !self.has_selection() {
            return false;
        }
        if self.content.is_sorted_by_key(|a| {
            self.selected.contains(&a.id()) != selected_first
        }) {
            return false;
        }
        self.record_change();
        // Stable boolean partition; no second clone of all annotation data.
        self.content
            .sort_by_key(|a| self.selected.contains(&a.id()) != selected_first);
        true
    }
}
