use super::{
    AnnotationColor, AnnotationGeometry, AnnotationStyle, Annotations, InkKind,
    ShapeAnnotation, ShapeKind, Tool,
};

impl ShapeAnnotation {
    #[must_use]
    pub const fn style(&self) -> AnnotationStyle {
        AnnotationStyle {
            color: self.color,
            stroke_width: self.stroke_width,
            filled: self.filled,
        }
    }

    pub(super) fn supports_fill(&self) -> bool {
        matches!(
            self.geometry,
            AnnotationGeometry::Bounded {
                kind: ShapeKind::Rectangle | ShapeKind::Oval,
                ..
            } | AnnotationGeometry::Ink {
                kind: InkKind::Pen,
                ..
            }
        )
    }
}

impl Annotations {
    #[must_use]
    pub fn current_style(&self) -> Option<AnnotationStyle> {
        if let Some(index) = self.style_target_indices().last().copied() {
            let annotation = self.shapes.get(index)?;
            return Some(annotation.style());
        }
        self.styles.get(self.tool)
    }

    #[must_use]
    pub fn current_fill(&self) -> Option<bool> {
        self.style_target_indices()
            .into_iter()
            .rev()
            .find_map(|index| {
                self.shapes
                    .get(index)
                    .filter(|annotation| annotation.supports_fill())
                    .map(|annotation| annotation.filled)
            })
            .or_else(|| self.styles.get(self.tool).map(|style| style.filled))
    }

    #[must_use]
    pub fn fill_available(&self) -> bool {
        let targets = self.style_target_indices();
        if !targets.is_empty() {
            return targets.into_iter().any(|index| {
                self.shapes
                    .get(index)
                    .is_some_and(ShapeAnnotation::supports_fill)
            });
        }
        self.tool.supports_fill()
    }

    pub fn set_color(&mut self, color: AnnotationColor) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let targets = self.style_target_indices();
        let object_changed = targets
            .iter()
            .any(|index| self.shapes[*index].color != color);
        let default_changed = self
            .styles
            .get(self.tool)
            .is_some_and(|style| style.color != color);
        if !object_changed && !default_changed {
            return false;
        }
        if object_changed {
            self.record_change();
            for index in targets {
                self.shapes[index].color = color;
            }
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
        let targets = self.style_target_indices();
        let object_changed = targets.iter().any(|index| {
            (self.shapes[*index].stroke_width - stroke_width).abs()
                > f32::EPSILON
        });
        let default_changed = self.styles.get(self.tool).is_some_and(|style| {
            (style.stroke_width - stroke_width).abs() > f32::EPSILON
        });
        if !object_changed && !default_changed {
            return false;
        }
        if object_changed {
            self.record_change();
            for index in targets {
                self.shapes[index].stroke_width = stroke_width;
            }
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
        let targets: Vec<_> = self
            .style_target_indices()
            .into_iter()
            .filter(|index| self.shapes[*index].supports_fill())
            .collect();
        if self.tool == Tool::Select {
            if targets.is_empty()
                || targets
                    .iter()
                    .all(|index| self.shapes[*index].filled == filled)
            {
                return false;
            }
            self.record_change();
            for index in targets {
                self.shapes[index].filled = filled;
            }
            return true;
        }
        if !self.tool.supports_fill() {
            return false;
        }
        let object_changed = targets
            .iter()
            .any(|index| self.shapes[*index].filled != filled);
        let default_changed = self
            .styles
            .get(self.tool)
            .is_some_and(|style| style.filled != filled);
        if !object_changed && !default_changed {
            return false;
        }
        if object_changed {
            self.record_change();
            for index in targets {
                self.shapes[index].filled = filled;
            }
        }
        if let Some(style) = self.styles.get_mut(self.tool) {
            style.filled = filled;
        }
        true
    }

    fn style_target_indices(&self) -> Vec<usize> {
        if self.tool == Tool::Select {
            return self.selected.clone();
        }
        let [index] = self.selected.as_slice() else {
            return Vec::new();
        };
        self.shapes
            .get(*index)
            .filter(|annotation| {
                self.tool
                    .creation_kind()
                    .is_some_and(|kind| kind.matches(annotation))
            })
            .map_or_else(Vec::new, |_| vec![*index])
    }
}
