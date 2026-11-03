//! One content-space layout for painting, hit-testing, scrolling and motion.

use super::{FastFill, Rail, SCROLLBAR_WIDTH, WIDTH};
use crate::app::pages::EditorPage;
use iced::{Point, Rectangle, Size, Vector};

const PADDING: f32 = 6.0;
const NUMBER_WIDTH: f32 = 22.0;
const GAP: f32 = 14.0;
const MAX_PAPER_HEIGHT: f32 = 138.0;

pub(super) struct Layout {
    pub(super) cards: Vec<Rectangle>,
    pub(super) order: Vec<usize>,
    pub(super) total: f32,
    hidden: Option<usize>,
}

pub(super) fn page_size(page: &EditorPage) -> Size {
    page.image.as_ref().map_or_else(
        || Size::new(page.size.width, page.size.height),
        |image| Size::new(image.size.width, image.size.height),
    )
}

fn paper_size(size: Size, width: f32) -> Size {
    let scale = ((width - NUMBER_WIDTH - 2.0 * PADDING).max(1.0) / size.width)
        .min(MAX_PAPER_HEIGHT / size.height);
    Size::new(size.width * scale, size.height * scale)
}

pub(super) fn paper_bounds(card: Rectangle, size: Size) -> Rectangle {
    let size = paper_size(size, card.width);
    Rectangle::new(
        Point::new(
            card.x
                + NUMBER_WIDTH
                + PADDING
                + (card.width - NUMBER_WIDTH - 2.0 * PADDING - size.width)
                    / 2.0,
            card.y + PADDING,
        ),
        size,
    )
}

pub(super) fn number_position(card: Rectangle) -> Point {
    Point::new(card.x + PADDING + NUMBER_WIDTH / 2.0, card.y + PADDING)
}

impl Layout {
    pub(super) fn new(app: &FastFill, width: f32) -> Self {
        // The thin scrollbar overlays cards only when scrolling is needed.
        let width = (width - 2.0).max(1.0);
        let mut cards = app
            .pages
            .iter()
            .map(|page| {
                let height = (paper_size(page_size(page), width).height
                    + 2.0 * PADDING)
                    .max(crate::appearance::CONTROL_HEIGHT);
                Rectangle::new(
                    Point::new(1.0, PADDING),
                    Size::new(width, height),
                )
            })
            .collect::<Vec<_>>();
        let mut order = (0..cards.len()).collect::<Vec<_>>();
        let hidden = app.dragged_page.filter(|drag| drag.moving).map(|drag| {
            let index = order.remove(drag.source);
            order.insert(drag.target, index);
            drag.source
        });
        let mut total = 0.0;
        for &index in &order {
            cards[index].y += total;
            total += cards[index].height + GAP;
        }
        Self {
            cards,
            order,
            total,
            hidden,
        }
    }

    pub(super) fn top(&self, index: usize) -> f32 {
        self.cards[index].y - PADDING
    }

    pub(super) fn original_top(&self, index: usize) -> f32 {
        self.cards
            .iter()
            .take(index)
            .map(|card| card.height + GAP)
            .sum()
    }

    pub(super) fn slot(&self, index: usize) -> usize {
        self.order.iter().position(|item| *item == index).unwrap()
    }

    pub(super) fn visible_slot(&self, index: usize) -> Option<usize> {
        (self.hidden != Some(index)).then(|| self.slot(index))
    }

    fn index_at(&self, y: f32) -> Option<usize> {
        self.order.iter().copied().find(|index| {
            let top = self.top(*index);
            (top..top + self.cards[*index].height + GAP).contains(&y)
        })
    }
}

pub(in crate::app) fn page_top(app: &FastFill, index: usize) -> f32 {
    let width = app
        .rail_motion
        .borrow()
        .bounds
        .map_or(WIDTH - 16.0, |bounds| bounds.width);
    Layout::new(app, width).original_top(index)
}

impl Rail<'_> {
    pub(super) fn layout(&self, bounds: Rectangle) -> Layout {
        Layout::new(self.app, bounds.width)
    }

    pub(super) fn scroll_thumb_height(&self, bounds: Rectangle) -> f32 {
        (bounds.height * bounds.height / self.layout(bounds).total)
            .max(32.0)
            .min(bounds.height)
    }

    pub(super) fn max_offset(&self, bounds: Rectangle) -> f32 {
        (self.layout(bounds).total - bounds.height).max(0.0)
    }

    pub(super) fn offset(&self, bounds: Rectangle) -> f32 {
        self.app.page_scroll.clamp(0.0, self.max_offset(bounds))
    }

    pub(super) fn card_bounds(
        &self,
        index: usize,
        bounds: Rectangle,
    ) -> Rectangle {
        self.layout(bounds).cards[index]
            + Vector::new(bounds.x, bounds.y - self.offset(bounds))
    }

    pub(super) fn index_at(
        &self,
        point: Point,
        bounds: Rectangle,
    ) -> Option<usize> {
        if !bounds.contains(point) {
            return None;
        }
        if let Some(ghost) = &self.app.rail_motion.borrow().ghost
            && Rectangle::new(ghost.position, ghost.size).contains(point)
        {
            return self
                .app
                .pages
                .iter()
                .position(|page| page.metadata.id == ghost.id);
        }
        for index in (0..self.app.pages.len()).rev() {
            if self.shown_card_bounds(index, bounds).contains(point) {
                return Some(index);
            }
        }
        let motion = self.app.rail_motion.borrow();
        if motion.animating() || motion.ghost.is_some() {
            return None;
        }
        self.layout(bounds)
            .index_at(point.y - bounds.y + self.offset(bounds))
    }

    pub(super) fn shown_card_bounds(
        &self,
        index: usize,
        bounds: Rectangle,
    ) -> Rectangle {
        let page = self.app.pages.iter().nth(index).unwrap();
        let motion = self.app.rail_motion.borrow();
        if let Some(ghost) = &motion.ghost
            && ghost.id == page.metadata.id
        {
            return Rectangle::new(ghost.position, ghost.size);
        }
        let mut card = self.card_bounds(index, bounds);
        let target = self.layout(bounds).top(index);
        card.y += motion.card_y(page.metadata.id, target) - target;
        card
    }

    pub(super) fn drop_index_at(
        &self,
        point: Point,
        bounds: Rectangle,
    ) -> Option<usize> {
        if !bounds.contains(point)
            || (self.max_offset(bounds) > 0.0
                && point.x >= bounds.x + bounds.width - SCROLLBAR_WIDTH)
        {
            return None;
        }
        let layout = self.layout(bounds);
        let y = point.y - bounds.y + self.offset(bounds);
        if let Some(drag) = self.app.dragged_page {
            let center = y - drag.grab.y + drag.origin.height / 2.0;
            // Cross neighbor centers, not edges: unequal heights stay stable.
            return Some(
                layout
                    .order
                    .iter()
                    .filter(|&&index| {
                        index != drag.source
                            && center >= layout.cards[index].center_y()
                    })
                    .count(),
            );
        }
        layout
            .index_at(y)
            .map(|index| layout.slot(index))
            .or_else(|| (y >= layout.total).then_some(layout.cards.len() - 1))
    }
}
