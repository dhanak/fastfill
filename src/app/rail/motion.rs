//! Presentation only: redraws never change page order or document history.

use super::{FastFill, ImagePicker, Rail};
use iced::time::{Duration, Instant};
use iced::{Point, Rectangle, Size};
use uuid::Uuid;

pub(super) const DURATION: Duration = Duration::from_millis(180);

#[derive(Default)]
pub(in crate::app) struct Motion {
    rows: Vec<(Uuid, Slide)>,
    pub(super) ghost: Option<Ghost>,
    pub(super) bounds: Option<Rectangle>,
    time: Option<Instant>,
    duration: Duration,
}

pub(super) struct Ghost {
    pub(super) id: Uuid,
    pub(super) position: Point,
    pub(super) size: Size,
    slide: Option<Slide>,
}

struct Slide {
    from: Point,
    to: Point,
    started: Instant,
}

impl Slide {
    fn new(position: Point, time: Instant) -> Self {
        Self {
            from: position,
            to: position,
            started: time,
        }
    }

    fn progress(&self, time: Instant, duration: Duration) -> f32 {
        if duration.is_zero() {
            return 1.0;
        }
        (time.saturating_duration_since(self.started).as_secs_f32()
            / duration.as_secs_f32())
        .clamp(0.0, 1.0)
    }

    fn at(&self, time: Instant, duration: Duration) -> Point {
        let t = self.progress(time, duration);
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    fn retarget(&mut self, to: Point, time: Instant, duration: Duration) {
        if duration.is_zero() {
            self.from = to;
            self.to = to;
            self.started = time;
        } else if self.to != to {
            self.from = self.at(time, duration);
            self.to = to;
            self.started = time;
        }
    }

    fn running(&self, time: Instant, duration: Duration) -> bool {
        self.from != self.to && self.progress(time, duration) < 1.0
    }
}

impl Motion {
    pub(super) fn animating(&self) -> bool {
        self.time.is_some_and(|time| {
            self.rows
                .iter()
                .any(|(_, slide)| slide.running(time, self.duration))
                || self
                    .ghost
                    .as_ref()
                    .is_some_and(|ghost| ghost.slide.is_some())
        })
    }

    pub(super) fn card_y(&self, id: Uuid, fallback: f32) -> f32 {
        self.rows.iter().find(|(key, _)| *key == id).map_or(
            fallback,
            |(_, slide)| {
                slide
                    .at(self.time.unwrap_or(slide.started), self.duration)
                    .y
            },
        )
    }

    pub(super) fn sync(
        &mut self,
        app: &FastFill,
        bounds: Rectangle,
        time: Instant,
        duration: Duration,
    ) -> bool {
        let changed = self.bounds.is_some_and(|old| old != bounds);
        let available = app.image_picker == ImagePicker::Idle
            && app.project.is_idle()
            && !app.confirm_delete_all
            && !app.confirm_delete_page;
        if changed || !available {
            self.rows.clear();
            self.ghost = None;
        }
        let duration = if changed || !available {
            Duration::ZERO
        } else {
            duration
        };
        self.bounds = Some(bounds);
        self.time = Some(time);
        self.duration = duration;
        let rail = Rail { app };
        let layout = rail.layout(bounds);
        self.rows.retain(|(id, _)| {
            app.pages.iter().any(|page| page.metadata.id == *id)
        });
        for (index, page) in app.pages.iter().enumerate() {
            let id = page.metadata.id;
            let target = Point::new(0.0, layout.top(index));
            if let Some((_, slide)) =
                self.rows.iter_mut().find(|(key, _)| *key == id)
            {
                slide.retarget(target, time, duration);
            } else {
                let mut slide = Slide::new(
                    Point::new(0.0, layout.original_top(index)),
                    time,
                );
                slide.retarget(target, time, duration);
                self.rows.push((id, slide));
            }
        }
        if available && !changed {
            self.sync_ghost(&rail, bounds, time, duration);
        }
        self.animating()
    }

    fn sync_ghost(
        &mut self,
        rail: &Rail<'_>,
        bounds: Rectangle,
        time: Instant,
        duration: Duration,
    ) {
        if let Some(drag) = rail.app.dragged_page {
            let id =
                rail.app.pages.iter().nth(drag.source).unwrap().metadata.id;
            let floating = drag.moving
                || self.ghost.as_ref().is_some_and(|ghost| ghost.id == id);
            self.ghost = floating.then(|| Ghost {
                id,
                position: drag.position,
                size: drag.origin.size(),
                slide: None,
            });
            return;
        }
        let Some(ghost) = &mut self.ghost else {
            return;
        };
        let Some(index) = rail
            .app
            .pages
            .iter()
            .position(|page| page.metadata.id == ghost.id)
        else {
            self.ghost = None;
            return;
        };
        let to = rail.card_bounds(index, bounds).position();
        let slide = ghost.slide.get_or_insert_with(|| {
            let mut slide = Slide::new(ghost.position, time);
            slide.retarget(to, time, duration);
            slide
        });
        slide.retarget(to, time, duration);
        ghost.position = slide.at(time, duration);
        if !slide.running(time, duration) {
            self.ghost = None;
        }
    }
}

pub(super) fn duration() -> Duration {
    if reduced_motion() {
        Duration::ZERO
    } else {
        DURATION
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn reduced_motion() -> bool {
    // Explicit opt-out until native platform preferences have a shared API.
    std::env::var_os("FASTFILL_REDUCED_MOTION")
        .is_some_and(|value| value == "1")
}

#[cfg(target_arch = "wasm32")]
fn reduced_motion() -> bool {
    browser_reduced_motion()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
export function browser_reduced_motion() {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}
")]
extern "C" {
    fn browser_reduced_motion() -> bool;
}
