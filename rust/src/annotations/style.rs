use super::{
    AnnotationColor, AnnotationStyle, Annotations, OwnedAnnotation,
    ShapeAnnotation, Tool,
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
}

impl Annotations {
    #[must_use]
    pub fn current_style(&self) -> Option<AnnotationStyle> {
        self.style_targets()
            .filter_map(|(_, a)| a.shape())
            .last()
            .map(ShapeAnnotation::style)
            .or_else(|| {
                (self.tool == Tool::Select && self.selected_count() > 1)
                    .then(|| {
                        self.style_targets().last().map(|(_, a)| a.style())
                    })
                    .flatten()
            })
            .or_else(|| self.styles.get(self.tool))
    }

    #[must_use]
    pub fn current_fill(&self) -> Option<bool> {
        self.style_targets()
            .filter(|(_, a)| a.tool().supports_fill())
            .last()
            .map(|(_, a)| a.style().filled)
            .or_else(|| self.styles.get(self.tool).map(|style| style.filled))
    }

    #[must_use]
    pub fn fill_available(&self) -> bool {
        self.tool.supports_fill()
            || self.style_targets().any(|(_, a)| a.tool().supports_fill())
    }

    #[must_use]
    pub fn stroke_available(&self) -> bool {
        self.tool.supports_stroke_width()
            || self
                .style_targets()
                .any(|(_, a)| a.tool().supports_stroke_width())
    }

    pub fn set_color(&mut self, color: AnnotationColor) -> bool {
        self.update_style(|_| true, |style| style.color = color)
    }

    pub fn set_stroke_width(&mut self, stroke_width: f32) -> bool {
        if !stroke_width.is_finite() || !(1.0..=20.0).contains(&stroke_width) {
            return false;
        }
        self.update_style(Tool::supports_stroke_width, |style| {
            if (style.stroke_width - stroke_width).abs() > f32::EPSILON {
                style.stroke_width = stroke_width;
            }
        })
    }

    pub fn set_filled(&mut self, filled: bool) -> bool {
        self.update_style(Tool::supports_fill, |style| style.filled = filled)
    }

    fn update_style(
        &mut self,
        supports: fn(Tool) -> bool,
        update: impl Fn(&mut AnnotationStyle),
    ) -> bool {
        if self.has_pending_edit() {
            return false;
        }
        let updates = self
            .style_targets()
            .filter(|(_, a)| supports(a.tool()))
            .filter_map(|(index, original)| {
                let original_style = original.style();
                let mut style = original_style;
                update(&mut style);
                if style == original_style {
                    return None;
                }
                let mut annotation = original.clone();
                annotation.set_style(style);
                Some((Some(index), annotation))
            })
            .collect();
        let content_changed = self.commit_content(updates);
        let default_changed = self
            .styles
            .get_mut(self.tool)
            .filter(|_| supports(self.tool))
            .is_some_and(|style| {
                let original = *style;
                update(style);
                *style != original
            });
        content_changed || default_changed
    }

    fn style_targets(&self) -> impl Iterator<Item = (usize, &OwnedAnnotation)> {
        let mut shapes =
            self.selected_content().filter(|(_, a)| a.shape().is_some());
        let single_shape = shapes
            .next()
            .filter(|_| shapes.next().is_none())
            .map(|(index, _)| index);
        self.selected_content().filter(move |(index, a)| {
            self.tool == Tool::Select
                || single_shape == Some(*index) && self.tool == a.tool()
        })
    }
}
