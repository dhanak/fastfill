//! Stage page-bound changes; share history without depending on image types.

use super::{AnnotationGeometry, Annotations};
use crate::geometry::{Point, Rect, Size};

pub struct PageChange {
    annotations: Annotations,
    moved: usize,
}

impl PageChange {
    #[must_use]
    pub const fn annotations(&self) -> &Annotations {
        &self.annotations
    }
}

impl Annotations {
    /// Opaque page-version key restored together with annotation history.
    #[must_use]
    pub const fn page_context(&self) -> u64 {
        self.page_context
    }

    #[must_use]
    pub fn uses_page_context(&self, context: u64) -> bool {
        self.page_context == context
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .any(|snapshot| snapshot.page_context == context)
    }

    /// Keep partly visible geometry unchanged. Move only fully off-page items.
    /// The caller can validate/encode this staged content before committing.
    #[must_use]
    pub fn prepare_page_change(&self, page: Size) -> Option<PageChange> {
        self.prepare_mapped_page_change(page, |point| {
            Some((point, Size::new(1.0, 1.0)))
        })
    }

    /// Re-anchor centers and resize X/Y independently; keep strokes/rotation.
    /// Text follows vertical font scaling, never horizontal glyph stretching.
    #[must_use]
    pub fn prepare_mapped_page_change(
        &self,
        page: Size,
        placement: impl Fn(Point) -> Option<(Point, Size)>,
    ) -> Option<PageChange> {
        if self.active_pointer().is_some()
            || self.has_text_edit()
            || !page.width.is_finite()
            || !page.height.is_finite()
            || page.width <= 0.0
            || page.height <= 0.0
        {
            return None;
        }
        // Clone committed content, not the entire undo/redo history.
        let mut annotations = Self::default();
        annotations.restore(self.snapshot());
        let mut moved = 0;
        for shape in &mut annotations.shapes {
            let bounds = shape.bounds();
            let center = bounds.center();
            let (target, scale) = checked_placement(placement(center)?)?;
            let point = |p: Point| {
                Point::new(
                    target.x + (p.x - center.x) * scale.width,
                    target.y + (p.y - center.y) * scale.height,
                )
            };
            if target != center || scale != Size::new(1.0, 1.0) {
                shape.geometry = match &shape.geometry {
                    AnnotationGeometry::Bounded { kind, .. } => {
                        let width = bounds.width * scale.width;
                        let height = bounds.height * scale.height;
                        AnnotationGeometry::Bounded {
                            kind: *kind,
                            bounds: Rect::new(
                                target.x - width / 2.0,
                                target.y - height / 2.0,
                                width,
                                height,
                            ),
                        }
                    }
                    AnnotationGeometry::Linear { kind, start, end } => {
                        AnnotationGeometry::Linear {
                            kind: *kind,
                            start: point(*start),
                            end: point(*end),
                        }
                    }
                    AnnotationGeometry::Ink { kind, points } => {
                        AnnotationGeometry::Ink {
                            kind: *kind,
                            points: points.iter().copied().map(point).collect(),
                        }
                    }
                };
            }
            let (dx, dy) = displacement(shape.visible_bounds(), page);
            if dx != 0.0 || dy != 0.0 {
                *shape = shape.translated(dx, dy);
                moved += 1;
            }
        }
        for text in &mut annotations.texts {
            let center = text.bounds.center();
            let (target, scale) = checked_placement(placement(center)?)?;
            let font_size = text.format.font_size * scale.height;
            if !(6.0..=144.0).contains(&font_size) {
                return None;
            }
            let width = text.bounds.width * scale.height;
            let height = text.bounds.height * scale.height;
            if target != center || scale.height.to_bits() != 1.0_f32.to_bits() {
                text.bounds = Rect::new(
                    target.x - width / 2.0,
                    target.y - height / 2.0,
                    width,
                    height,
                );
                text.format.font_size = font_size;
            }
            let (dx, dy) = displacement(text.visible_bounds(), page);
            if dx != 0.0 || dy != 0.0 {
                *text = text.translated(dx, dy);
                moved += 1;
            }
        }
        Some(PageChange { annotations, moved })
    }

    /// Commit validated staged content and its page key as one undo step.
    // Consume the staged change so callers cannot accidentally reuse it.
    #[allow(clippy::needless_pass_by_value)]
    pub fn commit_page_change(
        &mut self,
        change: PageChange,
        context: u64,
    ) -> Option<usize> {
        if self.active_pointer().is_some()
            || self.has_text_edit()
            || change.annotations.revision != self.revision
            || context == self.page_context
        {
            return None;
        }
        let mut snapshot = change.annotations.snapshot();
        self.record_change();
        snapshot.revision = self.revision;
        snapshot.page_context = context;
        self.restore(snapshot);
        Some(change.moved)
    }
}

fn checked_placement((point, scale): (Point, Size)) -> Option<(Point, Size)> {
    (point.x.is_finite()
        && point.y.is_finite()
        && scale.width.is_finite()
        && scale.height.is_finite()
        && scale.width > 0.0
        && scale.height > 0.0)
        .then_some((point, scale))
}

fn displacement(bounds: Rect, page: Size) -> (f32, f32) {
    let axis = |start: f32, length: f32, limit: f32| {
        let visible = 12.0_f32.min(length / 2.0).min(limit / 2.0);
        if start + length <= 0.0 {
            visible - (start + length)
        } else if start >= limit {
            limit - visible - start
        } else {
            0.0
        }
    };
    (
        axis(bounds.x, bounds.width, page.width),
        axis(bounds.y, bounds.height, page.height),
    )
}
