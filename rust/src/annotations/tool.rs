use super::{
    AnnotationColor, AnnotationGeometry, AnnotationStyle, InkKind, LineKind,
    ShapeAnnotation, ShapeKind,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Navigate,
    Rectangle,
    Oval,
    Line,
    Arrow,
    Checkmark,
    Cross,
    Pen,
    Highlighter,
    Select,
}

impl Tool {
    pub const ALL: [Self; 10] = [
        Self::Navigate,
        Self::Select,
        Self::Rectangle,
        Self::Oval,
        Self::Line,
        Self::Arrow,
        Self::Checkmark,
        Self::Cross,
        Self::Pen,
        Self::Highlighter,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Navigate => "Navigate",
            Self::Select => "Select",
            Self::Rectangle => "Rectangle",
            Self::Oval => "Oval",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
            Self::Checkmark => "Check",
            Self::Cross => "Cross",
            Self::Pen => "Pen",
            Self::Highlighter => "Highlight",
        }
    }

    #[must_use]
    pub const fn handles_annotations(self) -> bool {
        !matches!(self, Self::Navigate)
    }

    #[must_use]
    pub const fn creates_annotations(self) -> bool {
        self.creation_kind().is_some()
    }

    pub(super) const fn creation_kind(self) -> Option<CreationKind> {
        match self {
            Self::Rectangle => {
                Some(CreationKind::Bounded(ShapeKind::Rectangle))
            }
            Self::Oval => Some(CreationKind::Bounded(ShapeKind::Oval)),
            Self::Line => Some(CreationKind::Linear(LineKind::Line)),
            Self::Arrow => Some(CreationKind::Linear(LineKind::Arrow)),
            Self::Checkmark => {
                Some(CreationKind::Bounded(ShapeKind::Checkmark))
            }
            Self::Cross => Some(CreationKind::Bounded(ShapeKind::Cross)),
            Self::Pen => Some(CreationKind::Ink(InkKind::Pen)),
            Self::Highlighter => Some(CreationKind::Ink(InkKind::Highlighter)),
            Self::Navigate | Self::Select => None,
        }
    }

    pub(super) const fn supports_fill(self) -> bool {
        matches!(self, Self::Rectangle | Self::Oval | Self::Pen)
    }

    fn default_style(self) -> AnnotationStyle {
        let mut style = AnnotationStyle::default();
        match self {
            Self::Checkmark | Self::Cross => style.stroke_width = 2.0,
            Self::Highlighter => {
                style.color = AnnotationColor::Yellow;
                style.stroke_width = 12.0;
            }
            Self::Navigate
            | Self::Select
            | Self::Rectangle
            | Self::Oval
            | Self::Line
            | Self::Arrow
            | Self::Pen => {}
        }
        style
    }
}

#[derive(Debug)]
pub(super) struct ToolStyles(Vec<(Tool, AnnotationStyle)>);

impl Default for ToolStyles {
    fn default() -> Self {
        Self(
            Tool::ALL
                .into_iter()
                .filter(|tool| tool.creates_annotations())
                .map(|tool| (tool, tool.default_style()))
                .collect(),
        )
    }
}

impl ToolStyles {
    pub(super) fn get(&self, tool: Tool) -> Option<AnnotationStyle> {
        self.0.iter().find_map(|(candidate, style)| {
            (*candidate == tool).then_some(*style)
        })
    }

    pub(super) fn get_mut(
        &mut self,
        tool: Tool,
    ) -> Option<&mut AnnotationStyle> {
        self.0.iter_mut().find_map(|(candidate, style)| {
            (*candidate == tool).then_some(style)
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CreationKind {
    Bounded(ShapeKind),
    Linear(LineKind),
    Ink(InkKind),
}

impl CreationKind {
    pub(super) fn matches(self, annotation: &ShapeAnnotation) -> bool {
        match (self, &annotation.geometry) {
            (
                Self::Bounded(expected),
                AnnotationGeometry::Bounded { kind, .. },
            ) => *kind == expected,
            (
                Self::Linear(expected),
                AnnotationGeometry::Linear { kind, .. },
            ) => *kind == expected,
            (Self::Ink(expected), AnnotationGeometry::Ink { kind, .. }) => {
                *kind == expected
            }
            _ => false,
        }
    }
}
