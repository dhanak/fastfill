use super::geometry::{valid, valid_annotation_size};
use super::{
    AnnotationRef, AnnotationSettings, Annotations, Draft, OwnedAnnotation,
    Pointer, ShapeAnnotation, ShapeDraft, Snapshot, Tool,
};
use crate::geometry::{Point, Rect, Size};

impl Annotations {
    pub(crate) fn from_ordered(
        annotations: impl IntoIterator<Item = OwnedAnnotation>,
    ) -> Self {
        Self {
            content: annotations.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Committed content identity; undo restores the previous identity.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn settings(&self) -> AnnotationSettings {
        AnnotationSettings {
            tool: self.tool,
            styles: self.styles.clone(),
            text_format: self.text_format,
        }
    }

    pub fn apply_settings(&mut self, settings: &AnnotationSettings) {
        self.cancel();
        self.tool = settings.tool;
        self.styles = settings.styles.clone();
        self.text_format = settings.text_format;
    }

    #[must_use]
    pub const fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.cancel();
        self.tool = tool;
    }

    pub fn shapes(&self) -> impl DoubleEndedIterator<Item = &ShapeAnnotation> {
        self.content.iter().filter_map(OwnedAnnotation::shape)
    }

    #[must_use]
    /// Look up a shape at a layer index, not an index into `shapes()`.
    pub fn shape(&self, index: usize) -> Option<&ShapeAnnotation> {
        self.content.get(index)?.shape()
    }

    pub fn ordered_annotations(
        &self,
    ) -> impl Iterator<Item = AnnotationRef<'_>> {
        self.content.iter().enumerate().map(|(index, annotation)| {
            match annotation {
                OwnedAnnotation::Shape(shape) => {
                    AnnotationRef::Shape(index, shape)
                }
                OwnedAnnotation::Text(text) => AnnotationRef::Text(index, text),
            }
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    #[must_use]
    pub fn preview(&self) -> Option<ShapeAnnotation> {
        match self.draft.as_ref()?.annotation()? {
            OwnedAnnotation::Shape(shape) => Some(shape),
            OwnedAnnotation::Text(_) => None,
        }
    }

    /// Geometric previews in layer order; text-content editing is separate.
    pub fn geometric_previews(&self) -> impl Iterator<Item = OwnedAnnotation> {
        self.draft_previews()
            .into_iter()
            .map(|(_, annotation)| annotation)
    }

    pub(super) fn draft_previews(
        &self,
    ) -> Vec<(Option<usize>, OwnedAnnotation)> {
        self.draft.as_ref().map_or_else(Vec::new, Draft::previews)
    }

    #[must_use]
    pub fn active_pointer(&self) -> Option<Pointer> {
        self.draft.as_ref().map(Draft::pointer)
    }

    pub(super) fn has_pending_edit(&self) -> bool {
        self.draft.is_some() || self.text_draft.is_some()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.has_pending_edit() && !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.has_pending_edit() && !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if self.has_pending_edit() {
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
        if self.has_pending_edit() {
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
        if self.has_pending_edit()
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
        self.selected.retain(|id| {
            self.content
                .iter()
                .any(|a| a.id() == *id && a.shape().is_some())
        });
        self.draft = Some(Draft::Create(ShapeDraft {
            id: uuid::Uuid::new_v4(),
            pointer,
            kind,
            start: point,
            end: point,
            square: false,
            page,
            style: self.styles.get(self.tool).unwrap_or_default(),
            points: vec![point],
            mark_snap: None,
        }));
        true
    }

    /// Freeze a new mark's click target; dragging past the threshold is manual.
    pub fn set_mark_snap(
        &mut self,
        pointer: Pointer,
        bounds: Rect,
        threshold: f32,
    ) -> bool {
        if !valid(Point::new(bounds.x, bounds.y))
            || !valid(Point::new(
                bounds.x + bounds.width,
                bounds.y + bounds.height,
            ))
            || bounds.width < 1.0
            || (bounds.width - bounds.height).abs() > f32::EPSILON
            || !threshold.is_finite()
            || threshold <= 0.0
        {
            return false;
        }
        let Some(Draft::Create(draft)) = &mut self.draft else {
            return false;
        };
        if draft.pointer != pointer
            || !matches!(draft.kind,
                super::CreationKind::Bounded(kind) if kind.is_mark())
        {
            return false;
        }
        draft.mark_snap = Some((bounds, threshold));
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
        // Text drags historically commit their last valid position when the
        // final event has no finite point; keep that behavior during cleanup.
        if !valid(point) && !matches!(draft, Draft::TextMove(_)) {
            return false;
        }
        if let Draft::Selection(draft) = draft {
            let Some(bounds) = draft.bounds() else {
                self.apply_selection_click(draft.picked, draft.toggle);
                return true;
            };
            self.selected = self
                .content
                .iter()
                .filter_map(|annotation| {
                    annotation.hits(bounds).then_some(annotation.id())
                })
                .collect();
            return true;
        }
        let updates = draft.previews();
        if updates.iter().any(|(_, a)| {
            a.shape()
                .is_some_and(|shape| !valid_finished_annotation(shape))
        }) {
            return false;
        }
        let inserted = updates
            .first()
            .and_then(|(index, a)| index.is_none().then_some(a.id()));
        self.commit_content(updates);
        if let Some(id) = inserted {
            self.selected = vec![id];
        }
        true
    }

    pub fn cancel(&mut self) {
        self.draft = None;
        self.text_draft = None;
    }

    /// Apply staged replacements/insertions as one history step. All callers
    /// validate first, so a rejected edit never partially changes content.
    pub(super) fn commit_content(
        &mut self,
        updates: Vec<(Option<usize>, OwnedAnnotation)>,
    ) -> bool {
        if !updates.iter().any(|(index, annotation)| {
            index.is_none_or(|index| self.content[index] != *annotation)
        }) {
            return false;
        }
        self.record_change();
        for (index, annotation) in updates {
            match index {
                Some(index) => self.content[index] = annotation,
                None => self.content.push(annotation),
            }
        }
        true
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            revision: self.revision,
            page_context: self.page_context,
            content: self.content.clone(),
            selected: self.selected.clone(),
        }
    }

    pub(super) fn restore(&mut self, snapshot: Snapshot) {
        self.revision = snapshot.revision;
        self.page_context = snapshot.page_context;
        self.content = snapshot.content;
        self.selected = snapshot.selected;
    }

    pub(super) fn record_change(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
        self.next_revision += 1;
        self.revision = self.next_revision;
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
