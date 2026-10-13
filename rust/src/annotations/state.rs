use super::geometry::{valid, valid_annotation_size};
use super::{
    Annotations, Draft, Pointer, ShapeAnnotation, ShapeDraft, Snapshot, Tool,
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
    pub fn shapes(&self) -> &[ShapeAnnotation] {
        &self.shapes
    }

    #[must_use]
    pub fn preview(&self) -> Option<ShapeAnnotation> {
        self.draft.as_ref().and_then(Draft::annotation)
    }

    #[must_use]
    pub fn previews(&self) -> Vec<ShapeAnnotation> {
        self.draft.as_ref().map_or_else(Vec::new, |draft| {
            draft
                .previews()
                .into_iter()
                .map(|(_, annotation)| annotation)
                .collect()
        })
    }

    #[must_use]
    pub fn active_pointer(&self) -> Option<Pointer> {
        self.draft.as_ref().map(Draft::pointer)
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
        if !valid(point) {
            return false;
        }
        match draft {
            Draft::Create(draft) => {
                let annotation = draft.annotation();
                if !valid_finished_annotation(&annotation) {
                    return false;
                }
                self.record_change();
                self.shapes.push(annotation);
                self.selected = vec![self.shapes.len() - 1];
            }
            Draft::Edit(draft) => {
                let annotation = draft.annotation();
                if !valid_finished_annotation(&annotation) {
                    return false;
                }
                if self.shapes[draft.index] != annotation {
                    self.record_change();
                    self.shapes[draft.index] = annotation;
                }
            }
            Draft::GroupMove(draft) => {
                let moved = draft.annotations();
                if moved.iter().any(|(_, annotation)| {
                    !valid_finished_annotation(annotation)
                }) {
                    return false;
                }
                if moved.iter().any(|(index, annotation)| {
                    self.shapes[*index] != *annotation
                }) {
                    self.record_change();
                    for (index, annotation) in moved {
                        self.shapes[index] = annotation;
                    }
                }
            }
            Draft::Selection(draft) => {
                let bounds = draft.bounds();
                self.selected = self
                    .shapes
                    .iter()
                    .enumerate()
                    .filter_map(|(index, annotation)| {
                        intersects(bounds, annotation.visible_bounds())
                            .then_some(index)
                    })
                    .collect();
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
            selected: self.selected.clone(),
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.shapes = snapshot.shapes;
        self.selected = snapshot.selected;
    }

    pub(super) fn record_change(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
    }
}

fn valid_finished_annotation(annotation: &ShapeAnnotation) -> bool {
    let bounds = annotation.bounds();
    valid(Point::new(bounds.x, bounds.y))
        && bounds.width.is_finite()
        && bounds.height.is_finite()
        && valid_annotation_size(annotation)
        && bounds.x + bounds.width > 0.0
        && bounds.y + bounds.height > 0.0
}

fn intersects(first: Rect, second: Rect) -> bool {
    first.x <= second.x + second.width
        && first.x + first.width >= second.x
        && first.y <= second.y + second.height
        && first.y + first.height >= second.y
}
