//! Transient navigation for the two page-correction views.

use fastfill_rust::geometry::{Navigation, Point, Rect, Size};
use fastfill_rust::interaction::GestureDelta;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Frame,
    Preview,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ViewState {
    navigation: Navigation,
    actual_size: bool,
}

impl ViewState {
    pub(super) fn navigation(self, image: Size, viewport: Rect) -> Navigation {
        let mut navigation = self.navigation;
        if self.actual_size {
            navigation.one_to_one(image, viewport);
        }
        navigation
    }

    fn fit(&mut self) {
        *self = Self::default();
    }

    fn actual_size(&mut self) {
        self.fit();
        self.actual_size = true;
    }

    fn move_by(&mut self, delta: GestureDelta, image: Size, viewport: Rect) {
        let mut navigation = self.navigation(image, viewport);
        navigation.zoom_at(
            delta.zoom_factor,
            delta.zoom_anchor,
            image,
            viewport,
        );
        navigation.pan_by(delta.pan);
        self.navigation = navigation;
        self.actual_size = false;
    }
}

#[derive(Debug, Default)]
pub(super) struct Viewer {
    pub(super) mode: Mode,
    pub(super) comparing: bool,
    pub(super) epoch: u64,
    frame: ViewState,
    preview: ViewState,
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Mode(Mode),
    Compare(bool),
    Fit,
    ActualSize,
    Navigate {
        mode: Mode,
        delta: GestureDelta,
        image: Size,
        viewport: Rect,
    },
}

impl Viewer {
    pub(super) fn current(&self) -> ViewState {
        match self.mode {
            Mode::Frame => self.frame,
            Mode::Preview => self.preview,
        }
    }

    pub(super) fn select(&mut self, mode: Mode) {
        if self.mode != mode {
            self.mode = mode;
            self.epoch = self.epoch.wrapping_add(1);
        }
        self.comparing = false;
    }

    pub(super) fn geometry_changed(&mut self) {
        self.frame.fit();
        self.preview.fit();
        self.epoch = self.epoch.wrapping_add(1);
        self.comparing = false;
    }

    pub(super) fn crop_dragged(&mut self) {
        // Preserve the active frame gesture while invalidating its preview.
        self.preview.fit();
        self.comparing = false;
    }

    pub(super) fn update(&mut self, event: &Event) {
        match *event {
            Event::Mode(mode) => self.select(mode),
            Event::Compare(pressed) => {
                self.comparing = pressed && self.mode == Mode::Preview;
            }
            Event::Fit => {
                self.current_mut().fit();
                self.epoch = self.epoch.wrapping_add(1);
            }
            Event::ActualSize => {
                self.current_mut().actual_size();
                self.epoch = self.epoch.wrapping_add(1);
            }
            Event::Navigate {
                mode,
                delta,
                image,
                viewport,
            } if mode == self.mode => {
                self.current_mut().move_by(delta, image, viewport);
            }
            Event::Navigate { .. } => {}
        }
    }

    fn current_mut(&mut self) -> &mut ViewState {
        match self.mode {
            Mode::Frame => &mut self.frame,
            Mode::Preview => &mut self.preview,
        }
    }
}

pub(super) fn pan(x: f32, y: f32) -> GestureDelta {
    GestureDelta {
        pan: Point::new(x, y),
        zoom_factor: 1.0,
        zoom_anchor: Point::new(0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_shows_original_only_while_pressed() {
        let mut viewer = Viewer::default();
        viewer.update(&Event::Compare(true));
        assert!(!viewer.comparing);
        viewer.select(Mode::Preview);
        viewer.update(&Event::Compare(true));
        assert!(viewer.comparing);
        viewer.update(&Event::Compare(false));
        assert!(!viewer.comparing);
    }

    #[test]
    fn actual_size_and_view_navigation_stay_independent() {
        let mut viewer = Viewer::default();
        let image = Size::new(100.0, 100.0);
        let viewport = Rect::new(0.0, 0.0, 400.0, 400.0);
        viewer.update(&Event::ActualSize);
        let rect = viewer
            .current()
            .navigation(image, viewport)
            .view(image, viewport)
            .unwrap()
            .page_rect();
        assert!((rect.width - 100.0).abs() < 0.01);
        viewer.update(&Event::Navigate {
            mode: Mode::Frame,
            delta: GestureDelta {
                pan: Point::new(10.0, 0.0),
                zoom_factor: 2.0,
                zoom_anchor: Point::new(200.0, 200.0),
            },
            image,
            viewport,
        });
        let frame = viewer
            .current()
            .navigation(image, viewport)
            .view(image, viewport)
            .unwrap()
            .page_rect();
        assert!((frame.width - 200.0).abs() < 0.01);
        viewer.select(Mode::Preview);
        let preview = viewer
            .current()
            .navigation(image, viewport)
            .view(image, viewport)
            .unwrap()
            .page_rect();
        assert!((preview.width - 400.0).abs() < 0.01);
        viewer.select(Mode::Frame);
        assert!(
            (viewer
                .current()
                .navigation(image, viewport)
                .view(image, viewport)
                .unwrap()
                .page_rect()
                .x
                - frame.x)
                .abs()
                < 0.01
        );
    }
}
