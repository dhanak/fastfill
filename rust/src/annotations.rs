use crate::geometry::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Navigate,
    Rectangle,
    Select,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectangleAnnotation {
    pub bounds: Rect,
    pub stroke_width: f32,
}

#[derive(Debug)]
struct RectangleDraft {
    pointer: Pointer,
    start: Point,
    end: Point,
    square: bool,
    page: Size,
}

#[derive(Debug)]
struct EditDraft {
    pointer: Pointer,
    index: usize,
    original: RectangleAnnotation,
    start: Point,
    end: Point,
    square: bool,
    hit: EditHit,
    page: Size,
}

impl EditDraft {
    fn annotation(&self) -> RectangleAnnotation {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let bounds = self.original.bounds;
        let bounds = match self.hit {
            EditHit::Move => Rect::new(
                bounds.x + dx,
                bounds.y + dy,
                bounds.width,
                bounds.height,
            ),
            EditHit::Resize(corner) => {
                let moving = corner.position(bounds);
                drag_bounds(
                    corner.opposite().position(bounds),
                    Point::new(moving.x + dx, moving.y + dy),
                    self.square,
                )
            }
        };
        RectangleAnnotation {
            bounds: keep_reachable(bounds, self.page),
            ..self.original
        }
    }
}

#[derive(Debug)]
enum Draft {
    Create(RectangleDraft),
    Edit(EditDraft),
}

impl Draft {
    fn pointer(&self) -> Pointer {
        match self {
            Self::Create(draft) => draft.pointer,
            Self::Edit(draft) => draft.pointer,
        }
    }

    fn annotation(&self) -> RectangleAnnotation {
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

impl RectangleDraft {
    fn annotation(&self) -> RectangleAnnotation {
        RectangleAnnotation {
            bounds: drag_bounds(self.start, self.end, self.square),
            // Keep the default outline proportional to image resolution.
            stroke_width: self.page.width / 300.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Annotations {
    tool: Tool,
    rectangles: Vec<RectangleAnnotation>,
    draft: Option<Draft>,
    selected: Option<usize>,
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
    pub fn rectangles(&self) -> &[RectangleAnnotation] {
        &self.rectangles
    }

    #[must_use]
    pub fn preview(&self) -> Option<RectangleAnnotation> {
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
    pub fn edited_index(&self) -> Option<usize> {
        match &self.draft {
            Some(Draft::Edit(draft)) => Some(draft.index),
            _ => None,
        }
    }

    #[must_use]
    pub fn selection_bounds(&self, scale: f32) -> Option<Rect> {
        if !valid_scale(scale) || matches!(self.draft, Some(Draft::Create(_))) {
            return None;
        }
        let index = self.selected?;
        let annotation = if self.edited_index() == Some(index) {
            self.preview()?
        } else {
            *self.rectangles.get(index)?
        };
        let bounds = annotation.bounds;
        Some(bounds)
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
        let controls = self.selection_bounds(scale)?;
        let radius = match pointer {
            Pointer::Mouse => 12.0,
            Pointer::Touch(_) => 22.0,
        } / scale;
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
        if self.tool == Tool::Select
            && !matches!(hit, Some(EditHit::Resize(_)))
            && !(modifiers.force_move && hit.is_some())
        {
            self.selected = self
                .rectangles
                .iter()
                .rposition(|object| contains(object.bounds, point));
        }
        let hit = self.hit_selected(point, pointer, scale, modifiers);
        if let (Some(index), Some(hit)) = (self.selected, hit) {
            self.draft = Some(Draft::Edit(EditDraft {
                pointer,
                index,
                original: self.rectangles[index],
                start: point,
                end: point,
                square: modifiers.square,
                hit,
                page,
            }));
            return true;
        }
        if self.tool == Tool::Rectangle {
            return self.begin(pointer, point, page);
        }
        false
    }

    pub fn delete_selected(&mut self) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let Some(index) = self.selected.take() else {
            return false;
        };
        self.rectangles.remove(index);
        true
    }

    pub fn begin(
        &mut self,
        pointer: Pointer,
        point: Point,
        page: Size,
    ) -> bool {
        if self.tool != Tool::Rectangle
            || self.draft.is_some()
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
        self.draft = Some(Draft::Create(RectangleDraft {
            pointer,
            start: point,
            end: point,
            square: false,
            page,
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
        let bounds = annotation.bounds;
        if !valid(point)
            || !valid(Point::new(bounds.x, bounds.y))
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width < 1.0
            || bounds.height < 1.0
            || bounds.x + bounds.width <= 0.0
            || bounds.y + bounds.height <= 0.0
        {
            return false;
        }
        match draft {
            Draft::Create(_) => {
                self.rectangles.push(annotation);
                self.selected = Some(self.rectangles.len() - 1);
            }
            Draft::Edit(draft) => self.rectangles[draft.index] = annotation,
        }
        true
    }

    pub fn cancel(&mut self) {
        self.draft = None;
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
