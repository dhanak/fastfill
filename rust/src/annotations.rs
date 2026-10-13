use crate::geometry::{Point, Rect, Size};

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
    const fn creation_kind(self) -> Option<CreationKind> {
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

    const fn style_index(self) -> Option<usize> {
        match self {
            Self::Rectangle => Some(0),
            Self::Oval => Some(1),
            Self::Line => Some(2),
            Self::Arrow => Some(3),
            Self::Checkmark => Some(4),
            Self::Cross => Some(5),
            Self::Pen => Some(6),
            Self::Highlighter => Some(7),
            Self::Navigate | Self::Select => None,
        }
    }

    const fn supports_fill(self) -> bool {
        matches!(self, Self::Rectangle | Self::Oval | Self::Pen)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnnotationColor {
    #[default]
    Black,
    White,
    Red,
    Blue,
    Green,
    Yellow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationStyle {
    pub color: AnnotationColor,
    pub stroke_width: f32,
    pub filled: bool,
}

impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            color: AnnotationColor::Black,
            stroke_width: 3.0,
            filled: false,
        }
    }
}

#[derive(Debug)]
struct ToolStyles([AnnotationStyle; 8]);

impl Default for ToolStyles {
    fn default() -> Self {
        let mut styles = [AnnotationStyle::default(); 8];
        styles[4].stroke_width = 2.0;
        styles[5].stroke_width = 2.0;
        styles[7].color = AnnotationColor::Yellow;
        styles[7].stroke_width = 12.0;
        Self(styles)
    }
}

impl ToolStyles {
    fn get(&self, tool: Tool) -> Option<AnnotationStyle> {
        tool.style_index().map(|index| self.0[index])
    }

    fn get_mut(&mut self, tool: Tool) -> Option<&mut AnnotationStyle> {
        tool.style_index().map(|index| &mut self.0[index])
    }
}

#[derive(Clone, Copy, Debug)]
enum CreationKind {
    Bounded(ShapeKind),
    Linear(LineKind),
    Ink(InkKind),
}

impl CreationKind {
    fn matches(self, annotation: &ShapeAnnotation) -> bool {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Oval,
    Checkmark,
    Cross,
}

impl ShapeKind {
    const fn is_mark(self) -> bool {
        matches!(self, Self::Checkmark | Self::Cross)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Line,
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkKind {
    Pen,
    Highlighter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Mouse,
    Touch(u64),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DragModifiers {
    pub square: bool,
    pub force_move: bool,
    pub force_resize: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl ResizeCorner {
    #[must_use]
    pub fn position(self, bounds: Rect) -> Point {
        let right = bounds.x + bounds.width;
        let bottom = bounds.y + bounds.height;
        match self {
            Self::TopLeft => Point::new(bounds.x, bounds.y),
            Self::TopRight => Point::new(right, bounds.y),
            Self::BottomRight => Point::new(right, bottom),
            Self::BottomLeft => Point::new(bounds.x, bottom),
        }
    }

    fn opposite(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomRight => Self::TopLeft,
            Self::BottomLeft => Self::TopRight,
        }
    }
}

pub const RESIZE_CORNERS: [ResizeCorner; 4] = [
    ResizeCorner::TopLeft,
    ResizeCorner::TopRight,
    ResizeCorner::BottomRight,
    ResizeCorner::BottomLeft,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditHit {
    Move,
    Resize(ResizeCorner),
    Endpoint(LineEndpoint),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationGeometry {
    Bounded {
        kind: ShapeKind,
        bounds: Rect,
    },
    Linear {
        kind: LineKind,
        start: Point,
        end: Point,
    },
    Ink {
        kind: InkKind,
        points: Vec<Point>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEndpoint {
    Start,
    End,
}

impl LineEndpoint {
    fn opposite(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::Start,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeAnnotation {
    pub geometry: AnnotationGeometry,
    pub stroke_width: f32,
    pub color: AnnotationColor,
    pub filled: bool,
}

impl ShapeAnnotation {
    #[must_use]
    pub const fn style(&self) -> AnnotationStyle {
        AnnotationStyle {
            color: self.color,
            stroke_width: self.stroke_width,
            filled: self.filled,
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        match &self.geometry {
            AnnotationGeometry::Bounded { bounds, .. } => *bounds,
            AnnotationGeometry::Linear { kind, start, end } => {
                linear_bounds(*kind, *start, *end, self.stroke_width)
            }
            AnnotationGeometry::Ink { points, .. } => {
                ink_bounds(points, self.stroke_width)
            }
        }
    }

    #[must_use]
    pub fn kind(&self) -> Option<ShapeKind> {
        match &self.geometry {
            AnnotationGeometry::Bounded { kind, .. } => Some(*kind),
            AnnotationGeometry::Linear { .. }
            | AnnotationGeometry::Ink { .. } => None,
        }
    }

    fn supports_fill(&self) -> bool {
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

    fn translated(&self, dx: f32, dy: f32) -> Self {
        let geometry = match &self.geometry {
            AnnotationGeometry::Bounded { kind, bounds } => {
                AnnotationGeometry::Bounded {
                    kind: *kind,
                    bounds: Rect::new(
                        bounds.x + dx,
                        bounds.y + dy,
                        bounds.width,
                        bounds.height,
                    ),
                }
            }
            AnnotationGeometry::Linear { kind, start, end } => {
                AnnotationGeometry::Linear {
                    kind: *kind,
                    start: Point::new(start.x + dx, start.y + dy),
                    end: Point::new(end.x + dx, end.y + dy),
                }
            }
            AnnotationGeometry::Ink { kind, points } => {
                AnnotationGeometry::Ink {
                    kind: *kind,
                    points: points
                        .iter()
                        .map(|point| Point::new(point.x + dx, point.y + dy))
                        .collect(),
                }
            }
        };
        Self {
            geometry,
            stroke_width: self.stroke_width,
            color: self.color,
            filled: self.filled,
        }
    }
}

#[derive(Debug)]
struct ShapeDraft {
    pointer: Pointer,
    kind: CreationKind,
    start: Point,
    end: Point,
    square: bool,
    page: Size,
    style: AnnotationStyle,
    points: Vec<Point>,
}

#[derive(Debug)]
struct EditDraft {
    pointer: Pointer,
    index: usize,
    original: ShapeAnnotation,
    start: Point,
    end: Point,
    square: bool,
    hit: EditHit,
    page: Size,
}

impl EditDraft {
    fn annotation(&self) -> ShapeAnnotation {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let annotation = match self.hit {
            EditHit::Move => self.original.translated(dx, dy),
            EditHit::Resize(corner) => {
                let controls = self.original.bounds();
                let moving = corner.position(controls);
                let square = self.square
                    || matches!(
                        &self.original.geometry,
                        AnnotationGeometry::Bounded { kind, .. }
                            if kind.is_mark()
                    );
                let resized = drag_bounds(
                    corner.opposite().position(controls),
                    Point::new(moving.x + dx, moving.y + dy),
                    square,
                );
                let geometry = match &self.original.geometry {
                    AnnotationGeometry::Bounded { kind, .. } => {
                        AnnotationGeometry::Bounded {
                            kind: *kind,
                            bounds: resized,
                        }
                    }
                    AnnotationGeometry::Ink { kind, points } => {
                        AnnotationGeometry::Ink {
                            kind: *kind,
                            points: resize_ink_points(
                                points,
                                resized,
                                self.original.stroke_width,
                            ),
                        }
                    }
                    AnnotationGeometry::Linear { .. } => {
                        return self.original.clone();
                    }
                };
                ShapeAnnotation {
                    geometry,
                    stroke_width: self.original.stroke_width,
                    color: self.original.color,
                    filled: self.original.filled,
                }
            }
            EditHit::Endpoint(endpoint) => {
                let AnnotationGeometry::Linear { kind, start, end } =
                    &self.original.geometry
                else {
                    return self.original.clone();
                };
                let fixed = match endpoint.opposite() {
                    LineEndpoint::Start => *start,
                    LineEndpoint::End => *end,
                };
                let moving = match endpoint {
                    LineEndpoint::Start => *start,
                    LineEndpoint::End => *end,
                };
                let moving = Point::new(moving.x + dx, moving.y + dy);
                let moving = constrain_line(fixed, moving, self.square);
                let (start, end) = match endpoint {
                    LineEndpoint::Start => (moving, fixed),
                    LineEndpoint::End => (fixed, moving),
                };
                ShapeAnnotation {
                    geometry: AnnotationGeometry::Linear {
                        kind: *kind,
                        start,
                        end,
                    },
                    stroke_width: self.original.stroke_width,
                    color: self.original.color,
                    filled: self.original.filled,
                }
            }
        };
        keep_annotation_reachable(&annotation, self.page)
    }
}

#[derive(Debug)]
enum Draft {
    Create(ShapeDraft),
    Edit(EditDraft),
}

impl Draft {
    fn pointer(&self) -> Pointer {
        match self {
            Self::Create(draft) => draft.pointer,
            Self::Edit(draft) => draft.pointer,
        }
    }

    fn annotation(&self) -> ShapeAnnotation {
        match self {
            Self::Create(draft) => draft.annotation(),
            Self::Edit(draft) => draft.annotation(),
        }
    }

    fn update(&mut self, point: Point, square: bool) {
        match self {
            Self::Create(draft) => {
                draft.end = point;
                draft.square = square;
                if matches!(draft.kind, CreationKind::Ink(_))
                    && draft
                        .points
                        .last()
                        .is_none_or(|last| distance(*last, point) >= 0.5)
                {
                    draft.points.push(point);
                }
            }
            Self::Edit(draft) => {
                draft.end = point;
                draft.square = square;
            }
        }
    }

    fn constrain_square(&mut self, square: bool) {
        match self {
            Self::Create(draft) => draft.square = square,
            Self::Edit(draft) => draft.square = square,
        }
    }
}

impl ShapeDraft {
    fn annotation(&self) -> ShapeAnnotation {
        let is_mark = matches!(
            self.kind,
            CreationKind::Bounded(kind) if kind.is_mark()
        );
        let geometry = match self.kind {
            CreationKind::Bounded(kind) => AnnotationGeometry::Bounded {
                kind,
                bounds: if kind.is_mark() {
                    mark_bounds(self.start, self.end, self.page)
                } else {
                    drag_bounds(self.start, self.end, self.square)
                },
            },
            CreationKind::Linear(kind) => AnnotationGeometry::Linear {
                kind,
                start: self.start,
                end: constrain_line(self.start, self.end, self.square),
            },
            CreationKind::Ink(kind) => AnnotationGeometry::Ink {
                kind,
                points: self.points.clone(),
            },
        };
        ShapeAnnotation {
            geometry,
            stroke_width: self.style.stroke_width,
            color: self.style.color,
            filled: self.style.filled
                && !is_mark
                && !matches!(
                    self.kind,
                    CreationKind::Ink(InkKind::Highlighter)
                ),
        }
    }
}

#[derive(Clone, Debug)]
struct Snapshot {
    shapes: Vec<ShapeAnnotation>,
    selected: Option<usize>,
}

#[derive(Debug, Default)]
pub struct Annotations {
    tool: Tool,
    styles: ToolStyles,
    shapes: Vec<ShapeAnnotation>,
    draft: Option<Draft>,
    selected: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

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
            distance(point, a.position(controls))
                .total_cmp(&distance(point, b.position(controls)))
        })?;
        let corner_distance = distance(point, nearest.position(controls));
        let inside = contains(controls, point);
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
        let handle_hit =
            matches!(hit, Some(EditHit::Resize(_) | EditHit::Endpoint(_)));
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

fn valid(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn valid_scale(scale: f32) -> bool {
    scale.is_finite() && scale > 0.0
}

fn contains(bounds: Rect, point: Point) -> bool {
    point.x >= bounds.x
        && point.x <= bounds.x + bounds.width
        && point.y >= bounds.y
        && point.y <= bounds.y + bounds.height
}

fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn point_segment_distance(point: Point, start: Point, end: Point) -> f32 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f32::EPSILON {
        return distance(point, start);
    }
    let projection =
        ((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared;
    let projection = projection.clamp(0.0, 1.0);
    distance(
        point,
        Point::new(start.x + projection * dx, start.y + projection * dy),
    )
}

fn line_hit_radius(pointer: Pointer) -> f32 {
    match pointer {
        Pointer::Mouse => 7.0,
        Pointer::Touch(_) => 18.0,
    }
}

fn annotation_contains(
    annotation: &ShapeAnnotation,
    point: Point,
    line_tolerance: f32,
) -> bool {
    match &annotation.geometry {
        AnnotationGeometry::Bounded { bounds, .. } => contains(*bounds, point),
        AnnotationGeometry::Linear { kind, start, end } => {
            let shaft = point_segment_distance(point, *start, *end);
            if shaft <= line_tolerance {
                return true;
            }
            *kind == LineKind::Arrow
                && arrowhead_points(*start, *end).is_some_and(
                    |(left, right)| {
                        point_segment_distance(point, *end, left)
                            <= line_tolerance
                            || point_segment_distance(point, *end, right)
                                <= line_tolerance
                    },
                )
        }
        AnnotationGeometry::Ink { points, .. } => {
            (annotation.filled && point_in_polygon(point, points))
                || points.windows(2).any(|segment| {
                    point_segment_distance(point, segment[0], segment[1])
                        <= line_tolerance + annotation.stroke_width / 2.0
                })
        }
    }
}

fn point_in_polygon(point: Point, polygon: &[Point]) -> bool {
    let Some(mut previous) = polygon.last().copied() else {
        return false;
    };
    let mut inside = false;
    for current in polygon {
        let crosses = (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y)
                    / (previous.y - current.y)
                    + current.x;
        if crosses {
            inside = !inside;
        }
        previous = *current;
    }
    inside
}

fn valid_annotation_size(annotation: &ShapeAnnotation) -> bool {
    match &annotation.geometry {
        AnnotationGeometry::Bounded { bounds, .. } => {
            bounds.width >= 1.0 && bounds.height >= 1.0
        }
        AnnotationGeometry::Linear { start, end, .. } => {
            distance(*start, *end) >= 1.0
        }
        AnnotationGeometry::Ink { points, .. } => points
            .windows(2)
            .any(|segment| distance(segment[0], segment[1]) >= 1.0),
    }
}

fn constrain_line(start: Point, end: Point, enabled: bool) -> Point {
    if !enabled {
        return end;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return end;
    }
    let step = std::f32::consts::FRAC_PI_4;
    let angle = (dy.atan2(dx) / step).round() * step;
    Point::new(
        start.x + length * angle.cos(),
        start.y + length * angle.sin(),
    )
}

#[must_use]
pub fn arrowhead_points(start: Point, end: Point) -> Option<(Point, Point)> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return None;
    }
    // Keep arrowhead geometry independent from line thickness.
    let head = 24.0_f32.min(length * 0.8);
    let ux = dx / length;
    let uy = dy / length;
    let base = Point::new(end.x - ux * head, end.y - uy * head);
    let half_width = head * 0.45;
    let offset = Point::new(-uy * half_width, ux * half_width);
    Some((
        Point::new(base.x + offset.x, base.y + offset.y),
        Point::new(base.x - offset.x, base.y - offset.y),
    ))
}

fn linear_bounds(
    kind: LineKind,
    start: Point,
    end: Point,
    stroke_width: f32,
) -> Rect {
    let mut bounds = segment_stroke_bounds(start, end, stroke_width);
    if kind == LineKind::Arrow
        && let Some((first, second)) = arrowhead_points(start, end)
    {
        for wing in [first, second] {
            bounds = union_bounds(
                bounds,
                segment_stroke_bounds(end, wing, stroke_width),
            );
        }
        let half = stroke_width / 2.0;
        bounds = union_bounds(
            bounds,
            Rect::new(end.x - half, end.y - half, stroke_width, stroke_width),
        );
    }
    bounds
}

fn segment_stroke_bounds(start: Point, end: Point, stroke_width: f32) -> Rect {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return Rect::new(start.x, start.y, 0.0, 0.0);
    }
    let half = stroke_width / 2.0;
    let x_padding = half * (dy / length).abs();
    let y_padding = half * (dx / length).abs();
    Rect::new(
        start.x.min(end.x) - x_padding,
        start.y.min(end.y) - y_padding,
        (end.x - start.x).abs() + x_padding * 2.0,
        (end.y - start.y).abs() + y_padding * 2.0,
    )
}

fn point_bounds(points: &[Point]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    };
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x;
    let mut bottom = first.y;
    for point in &points[1..] {
        left = left.min(point.x);
        top = top.min(point.y);
        right = right.max(point.x);
        bottom = bottom.max(point.y);
    }
    Rect::new(left, top, right - left, bottom - top)
}

fn ink_bounds(points: &[Point], stroke_width: f32) -> Rect {
    let bounds = point_bounds(points);
    let half = stroke_width / 2.0;
    Rect::new(
        bounds.x - half,
        bounds.y - half,
        bounds.width + stroke_width,
        bounds.height + stroke_width,
    )
}

fn resize_ink_points(
    points: &[Point],
    target: Rect,
    stroke_width: f32,
) -> Vec<Point> {
    let source = point_bounds(points);
    let half = stroke_width / 2.0;
    let width = (target.width - stroke_width).max(1.0);
    let height = (target.height - stroke_width).max(1.0);
    let left = target.x + half;
    let top = target.y + half;
    points
        .iter()
        .map(|point| {
            let x = if source.width <= f32::EPSILON {
                left + width / 2.0
            } else {
                left + (point.x - source.x) / source.width * width
            };
            let y = if source.height <= f32::EPSILON {
                top + height / 2.0
            } else {
                top + (point.y - source.y) / source.height * height
            };
            Point::new(x, y)
        })
        .collect()
}

fn union_bounds(first: Rect, second: Rect) -> Rect {
    let left = first.x.min(second.x);
    let top = first.y.min(second.y);
    let right = (first.x + first.width).max(second.x + second.width);
    let bottom = (first.y + first.height).max(second.y + second.height);
    Rect::new(left, top, right - left, bottom - top)
}

fn drag_bounds(start: Point, end: Point, square: bool) -> Rect {
    let mut dx = end.x - start.x;
    let mut dy = end.y - start.y;
    if square {
        let side = dx.abs().max(dy.abs());
        dx = side.copysign(dx);
        dy = side.copysign(dy);
    }
    Rect::new(
        start.x.min(start.x + dx),
        start.y.min(start.y + dy),
        dx.abs(),
        dy.abs(),
    )
}

fn mark_bounds(center: Point, end: Point, page: Size) -> Rect {
    let default_half = page.width / 64.0;
    let half = (end.x - center.x)
        .abs()
        .max((end.y - center.y).abs())
        .max(default_half);
    Rect::new(center.x - half, center.y - half, half * 2.0, half * 2.0)
}

fn keep_reachable(mut bounds: Rect, page: Size) -> Rect {
    // Clamp continuously; correcting only after disappearance causes a jump.
    let visible_x = 1.0_f32.min(bounds.width).min(page.width);
    let visible_y = 1.0_f32.min(bounds.height).min(page.height);
    bounds.x = bounds
        .x
        .clamp(visible_x - bounds.width, page.width - visible_x);
    bounds.y = bounds
        .y
        .clamp(visible_y - bounds.height, page.height - visible_y);
    bounds
}

fn keep_annotation_reachable(
    annotation: &ShapeAnnotation,
    page: Size,
) -> ShapeAnnotation {
    let bounds = annotation.bounds();
    let kept = keep_reachable(bounds, page);
    annotation.translated(kept.x - bounds.x, kept.y - bounds.y)
}
