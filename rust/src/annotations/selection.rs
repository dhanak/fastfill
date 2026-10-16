use super::geometry::{
    annotation_contains, contains, distance, line_hit_radius, union_bounds,
    valid, valid_scale,
};
use super::{
    AnnotationGeometry, Annotations, Draft, DragModifiers, EditDraft, EditHit,
    GroupMoveDraft, Layer, LineEndpoint, Pointer, RESIZE_CORNERS,
    SELECTION_STROKE, SelectionDraft, ShapeAnnotation, Tool,
};
use crate::geometry::{Point, Rect, Size};

impl Annotations {
    pub(super) fn hit_layer(
        &self,
        point: Point,
        pointer: Pointer,
        scale: f32,
    ) -> Option<Layer> {
        if !valid(point) || !valid_scale(scale) {
            return None;
        }
        let tolerance = line_hit_radius(pointer) / scale;
        self.layers.iter().rev().find_map(|layer| match *layer {
            Layer::Shape(index)
                if self.shapes.get(index).is_some_and(|annotation| {
                    annotation_contains(annotation, point, tolerance)
                }) =>
            {
                Some(*layer)
            }
            Layer::Text(index)
                if self.texts.get(index).is_some_and(|annotation| {
                    contains(
                        annotation.bounds,
                        annotation.unrotated_point(point),
                    )
                }) =>
            {
                Some(*layer)
            }
            Layer::Shape(_) | Layer::Text(_) => None,
        })
    }

    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        match (self.selected.as_slice(), self.selected_texts.as_slice()) {
            ([index], []) => Some(*index),
            _ => None,
        }
    }

    #[must_use]
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected
    }

    #[must_use]
    pub fn selected_text_indices(&self) -> &[usize] {
        &self.selected_texts
    }

    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selected.len() + self.selected_texts.len()
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selected_count() != 0
    }

    /// Selection is transient; clearing it never changes content or history.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.selected_texts.clear();
    }

    #[must_use]
    pub fn can_reset_rotation(&self) -> bool {
        self.draft.is_none()
            && self.text_draft.is_none()
            && self.text_move.is_none()
            && (self.selected.iter().any(|index| {
                self.shapes.get(*index).is_some_and(|annotation| {
                    annotation.supports_rotation()
                        && annotation.rotation.abs() > f32::EPSILON
                })
            }) || self.selected_texts.iter().any(|index| {
                self.texts
                    .get(*index)
                    .is_some_and(|text| text.rotation.abs() > f32::EPSILON)
            }))
    }

    pub fn reset_rotation(&mut self) -> bool {
        if !self.can_reset_rotation() {
            return false;
        }
        self.record_change();
        for index in &self.selected {
            if self.shapes[*index].supports_rotation() {
                self.shapes[*index].rotation = 0.0;
            }
        }
        for index in &self.selected_texts {
            self.texts[*index].rotation = 0.0;
        }
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
    pub fn is_edited(&self, index: usize) -> bool {
        match &self.draft {
            Some(Draft::Edit(draft)) => draft.index == index,
            Some(Draft::GroupMove(draft)) => {
                draft.originals.iter().any(|(edited, _)| *edited == index)
            }
            Some(Draft::Create(_) | Draft::Selection(_)) | None => false,
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
            self.shapes.get(index).cloned()
        }
    }

    #[must_use]
    pub fn selection_bounds(&self, scale: f32) -> Option<Rect> {
        if !valid_scale(scale) {
            return None;
        }
        self.selection_annotations(scale)
            .into_iter()
            .map(|annotation| annotation.visible_bounds())
            .chain(
                self.selection_texts()
                    .iter()
                    .map(super::TextAnnotation::visible_bounds),
            )
            .reduce(union_bounds)
    }

    #[must_use]
    pub fn selection_annotations(&self, scale: f32) -> Vec<ShapeAnnotation> {
        if !valid_scale(scale) || matches!(self.draft, Some(Draft::Create(_))) {
            return Vec::new();
        }
        let previews =
            self.draft.as_ref().map_or_else(Vec::new, Draft::previews);
        self.selected
            .iter()
            .filter_map(|index| {
                previews
                    .iter()
                    .find_map(|(preview_index, annotation)| {
                        (preview_index == &Some(*index))
                            .then(|| annotation.clone())
                    })
                    .or_else(|| self.shapes.get(*index).cloned())
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
            self.shapes.get(index)?.clone()
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

    fn hit_group(&self, point: Point, picked: Option<Layer>) -> bool {
        picked.map_or_else(
            || {
                self.selected.iter().any(|index| {
                    contains(self.shapes[*index].visible_bounds(), point)
                }) || self.selected_texts.iter().any(|index| {
                    let text = &self.texts[*index];
                    contains(text.bounds, text.unrotated_point(point))
                })
            },
            |layer| match layer {
                Layer::Shape(index) => {
                    self.selected.binary_search(&index).is_ok()
                }
                Layer::Text(index) => {
                    self.selected_texts.binary_search(&index).is_ok()
                }
            },
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
        if self.draft.is_some()
            || self.text_move.is_some()
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
            self.selected_texts.clear();
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
        let handle_hit = matches!(
            hit,
            Some(EditHit::Resize(_) | EditHit::Endpoint(_) | EditHit::Rotate)
        );
        if (handle_hit || modifiers.force_move && hit.is_some())
            && let (Some(index), Some(hit)) = (self.selected_index(), hit)
        {
            self.start_edit(pointer, index, point, page, modifiers, hit);
            return true;
        }

        if self.hit_selected_text(point, pointer, scale)
            == Some(super::TextEditHit::Rotate)
        {
            return self.begin_text_move(pointer, point, page, scale);
        }
        if self.selected_count() > 1 && hit == Some(EditHit::Move) {
            self.start_group_move(pointer, point, page);
            return true;
        }
        let picked = self.hit_layer(point, pointer, scale);
        if picked.is_none()
            && let (Some(index), Some(EditHit::Move)) =
                (self.selected_index(), hit)
        {
            self.start_edit(
                pointer,
                index,
                point,
                page,
                modifiers,
                EditHit::Move,
            );
            return true;
        }
        if let Some(Layer::Text(index)) = picked
            && (force_edit
                || self.selected_count() == 1
                    && self.selected_texts.binary_search(&index).is_ok())
        {
            return self.begin_text_move(pointer, point, page, scale);
        }
        if let Some(Layer::Shape(index)) = picked
            && (force_edit || self.selected_index() == Some(index))
        {
            self.selected_texts.clear();
            self.selected = vec![index];
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
        picked: Option<Layer>,
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
        picked: Option<Layer>,
        toggle: bool,
    ) {
        if !toggle {
            self.selected.clear();
            self.selected_texts.clear();
        }
        let Some(layer) = picked else { return };
        let (indices, index) = match layer {
            Layer::Shape(index) => (&mut self.selected, index),
            Layer::Text(index) => (&mut self.selected_texts, index),
        };
        match indices.binary_search(&index) {
            Ok(position) => {
                indices.remove(position);
            }
            Err(position) => indices.insert(position, index),
        }
    }

    fn start_group_move(&mut self, pointer: Pointer, point: Point, page: Size) {
        let originals = self
            .selected
            .iter()
            .map(|index| (*index, self.shapes[*index].clone()))
            .collect();
        self.draft = Some(Draft::GroupMove(GroupMoveDraft {
            pointer,
            originals,
            original_texts: self
                .selected_texts
                .iter()
                .map(|index| (*index, self.texts[*index].clone()))
                .collect(),
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
            original: self.shapes[index].clone(),
            start: point,
            end: point,
            square: modifiers.square,
            hit,
            page,
        }));
    }

    pub fn delete_selected(&mut self) -> bool {
        if self.draft.is_some()
            || self.text_draft.is_some()
            || self.text_move.is_some()
            || !self.has_selection()
        {
            return false;
        }
        self.record_change();
        self.remove_shape_layers(&self.selected.clone());
        for index in self.selected.iter().rev() {
            self.shapes.remove(*index);
        }
        self.selected.clear();
        for index in self.selected_texts.clone().into_iter().rev() {
            self.remove_text_layer(index);
            self.texts.remove(index);
        }
        self.selected_texts.clear();
        true
    }

    pub fn delete_all(&mut self) -> bool {
        if self.draft.is_some() || self.text_draft.is_some() || self.is_empty()
        {
            return false;
        }
        self.record_change();
        self.shapes.clear();
        self.texts.clear();
        self.layers.clear();
        self.selected.clear();
        self.selected_texts.clear();
        true
    }

    pub fn send_selected_to_back(&mut self) -> bool {
        self.reorder_selected(true)
    }

    pub fn bring_selected_to_front(&mut self) -> bool {
        self.reorder_selected(false)
    }

    fn reorder_selected(&mut self, selected_first: bool) -> bool {
        if self.draft.is_some()
            || self.text_draft.is_some()
            || self.text_move.is_some()
            || !self.has_selection()
        {
            return false;
        }
        let is_selected = |layer: &super::Layer| match layer {
            super::Layer::Shape(index) => {
                self.selected.binary_search(index).is_ok()
            }
            super::Layer::Text(index) => {
                self.selected_texts.binary_search(index).is_ok()
            }
        };
        let selected: Vec<_> =
            self.layers.iter().copied().filter(is_selected).collect();
        let unselected: Vec<_> = self
            .layers
            .iter()
            .copied()
            .filter(|layer| !is_selected(layer))
            .collect();
        let order: Vec<_> = if selected_first {
            selected.iter().chain(&unselected).copied().collect()
        } else {
            unselected.iter().chain(&selected).copied().collect()
        };
        if order == self.layers {
            return false;
        }
        self.record_change();
        self.layers = order;
        true
    }
}
