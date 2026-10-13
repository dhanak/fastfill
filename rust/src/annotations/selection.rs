use super::geometry::{
    annotation_contains, contains, distance, line_hit_radius, union_bounds,
    valid, valid_scale,
};
use super::{
    AnnotationGeometry, Annotations, Draft, DragModifiers, EditDraft, EditHit,
    GroupMoveDraft, LineEndpoint, Pointer, RESIZE_CORNERS, SelectionDraft,
    ShapeAnnotation, Tool,
};
use crate::geometry::{Point, Rect, Size};

impl Annotations {
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        match self.selected.as_slice() {
            [index] => Some(*index),
            _ => None,
        }
    }

    #[must_use]
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected
    }

    #[must_use]
    pub fn can_reset_rotation(&self) -> bool {
        self.draft.is_none()
            && self.selected.iter().any(|index| {
                self.shapes.get(*index).is_some_and(|annotation| {
                    annotation.supports_rotation()
                        && annotation.rotation.abs() > f32::EPSILON
                })
            })
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
            Some(Draft::Selection(draft)) => Some(draft.bounds()),
            _ => None,
        }
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
        let index = self.selected_index()?;
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
        let handle_hit = matches!(
            hit,
            Some(EditHit::Resize(_) | EditHit::Endpoint(_) | EditHit::Rotate)
        );
        let forced_move = modifiers.force_move && hit.is_some();
        if self.tool == Tool::Select {
            if (handle_hit || forced_move)
                && let (Some(index), Some(hit)) = (self.selected_index(), hit)
            {
                self.start_edit(pointer, index, point, page, modifiers, hit);
                return true;
            }

            let tolerance = line_hit_radius(pointer) / scale;
            let picked = self.shapes.iter().rposition(|annotation| {
                annotation_contains(annotation, point, tolerance)
            });
            if picked.is_none()
                && let (Some(index), Some(EditHit::Move)) =
                    (self.selected_index(), hit)
            {
                if modifiers.square {
                    self.selected.clear();
                } else {
                    self.start_edit(
                        pointer,
                        index,
                        point,
                        page,
                        modifiers,
                        EditHit::Move,
                    );
                }
                return true;
            }
            if modifiers.square
                && let Some(index) = picked
            {
                match self.selected.binary_search(&index) {
                    Ok(position) => {
                        self.selected.remove(position);
                    }
                    Err(position) => self.selected.insert(position, index),
                }
                return true;
            }
            let group_hit = self.selected.len() > 1
                && picked.map_or_else(
                    || {
                        self.selected.iter().any(|index| {
                            contains(
                                self.shapes[*index].visible_bounds(),
                                point,
                            )
                        })
                    },
                    |index| self.selected.binary_search(&index).is_ok(),
                );
            if group_hit {
                self.start_group_move(pointer, point, page);
                return true;
            }
            if let Some(index) = picked {
                self.selected = vec![index];
                let hit = self
                    .hit_selected(point, pointer, scale, modifiers)
                    .unwrap_or(EditHit::Move);
                self.start_edit(pointer, index, point, page, modifiers, hit);
                return true;
            }
            self.selected.clear();
            self.draft = Some(Draft::Selection(SelectionDraft {
                start: point,
                end: point,
                pointer,
            }));
            return true;
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

    fn start_group_move(&mut self, pointer: Pointer, point: Point, page: Size) {
        let originals = self
            .selected
            .iter()
            .map(|index| (*index, self.shapes[*index].clone()))
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
            original: self.shapes[index].clone(),
            start: point,
            end: point,
            square: modifiers.square,
            hit,
            page,
        }));
    }

    pub fn delete_selected(&mut self) -> bool {
        if self.draft.is_some() || self.selected.is_empty() {
            return false;
        }
        self.record_change();
        for index in self.selected.iter().rev() {
            self.shapes.remove(*index);
        }
        self.selected.clear();
        true
    }

    pub fn delete_all(&mut self) -> bool {
        if self.draft.is_some() || self.shapes.is_empty() {
            return false;
        }
        self.record_change();
        self.shapes.clear();
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
        if self.draft.is_some() || self.selected.is_empty() {
            return false;
        }
        let selected: Vec<_> = (0..self.shapes.len())
            .filter(|index| self.selected.binary_search(index).is_ok())
            .collect();
        let unselected: Vec<_> = (0..self.shapes.len())
            .filter(|index| self.selected.binary_search(index).is_err())
            .collect();
        let order: Vec<_> = if selected_first {
            selected.iter().chain(&unselected).copied().collect()
        } else {
            unselected.iter().chain(&selected).copied().collect()
        };
        if order.iter().copied().eq(0..self.shapes.len()) {
            return false;
        }
        self.record_change();
        self.shapes = order
            .into_iter()
            .map(|index| self.shapes[index].clone())
            .collect();
        let start = if selected_first { 0 } else { unselected.len() };
        self.selected = (start..start + selected.len()).collect();
        true
    }
}
