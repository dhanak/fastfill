#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    fn is_valid(self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && Size::new(self.width, self.height).is_valid()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageView {
    page_rect: Rect,
    scale: f32,
}

impl PageView {
    #[must_use]
    pub const fn page_rect(self) -> Rect {
        self.page_rect
    }

    #[must_use]
    pub fn page_to_view(self, point: Point) -> Point {
        Point::new(
            self.page_rect.x + point.x * self.scale,
            self.page_rect.y + point.y * self.scale,
        )
    }

    #[must_use]
    pub fn view_to_page(self, point: Point) -> Point {
        Point::new(
            (point.x - self.page_rect.x) / self.scale,
            (point.y - self.page_rect.y) / self.scale,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Navigation {
    zoom: f32,
    pan: Point,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Point::new(0.0, 0.0),
        }
    }
}

impl Navigation {
    pub const MAX_ZOOM: f32 = 16.0;
    pub const MIN_ZOOM: f32 = 0.25;

    #[must_use]
    pub const fn zoom(self) -> f32 {
        self.zoom
    }

    #[must_use]
    pub fn view(self, page: Size, viewport: Rect) -> Option<PageView> {
        let fitted = fit_contain(page, viewport)?;
        let fitted_rect = fitted.page_rect();
        let width = fitted_rect.width * self.zoom;
        let height = fitted_rect.height * self.zoom;
        let center = viewport.center();
        let page_rect = Rect::new(
            center.x - width / 2.0 + self.pan.x,
            center.y - height / 2.0 + self.pan.y,
            width,
            height,
        );

        Some(PageView {
            page_rect,
            scale: fitted.scale * self.zoom,
        })
    }

    pub fn fit(&mut self) {
        *self = Self::default();
    }

    pub fn one_to_one(&mut self, page: Size, viewport: Rect) {
        let Some(fitted) = fit_contain(page, viewport) else {
            return;
        };
        self.zoom = (1.0 / fitted.scale).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.pan = Point::new(0.0, 0.0);
    }

    pub fn pan_by(&mut self, delta: Point) {
        if delta.x.is_finite() && delta.y.is_finite() {
            self.pan.x += delta.x;
            self.pan.y += delta.y;
        }
    }

    pub fn zoom_at(
        &mut self,
        factor: f32,
        anchor: Point,
        page: Size,
        viewport: Rect,
    ) {
        if !factor.is_finite()
            || factor <= 0.0
            || !anchor.x.is_finite()
            || !anchor.y.is_finite()
        {
            return;
        }
        let Some(before) = self.view(page, viewport) else {
            return;
        };
        let page_anchor = before.view_to_page(anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        let Some(after) = self.view(page, viewport) else {
            return;
        };
        let moved_anchor = after.page_to_view(page_anchor);
        self.pan_by(Point::new(
            anchor.x - moved_anchor.x,
            anchor.y - moved_anchor.y,
        ));
    }
}

#[must_use]
pub fn fit_contain(page: Size, viewport: Rect) -> Option<PageView> {
    if !page.is_valid() || !viewport.is_valid() {
        return None;
    }

    let scale =
        (viewport.width / page.width).min(viewport.height / page.height);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let width = page.width * scale;
    let height = page.height * scale;
    let page_rect = Rect::new(
        viewport.x + (viewport.width - width) / 2.0,
        viewport.y + (viewport.height - height) / 2.0,
        width,
        height,
    );

    Some(PageView { page_rect, scale })
}
