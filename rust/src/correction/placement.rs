//! Re-anchor through the source image without rotating/skewing annotations.

use super::perspective::Projection;
use super::{CorrectionError, PageCorrection, corrected_dimensions};
use crate::geometry::{Point, Size};

pub struct PageMapping {
    old: Projection,
    new: Projection,
    old_size: (f64, f64),
    new_size: (f64, f64),
    old_turns: i32,
    new_turns: i32,
    identity: bool,
}

impl PageMapping {
    /// Both coordinate systems have the editor's 612-point width.
    ///
    /// # Errors
    /// Rejects invalid source dimensions or correction settings.
    pub fn new(
        width: u32,
        height: u32,
        old: PageCorrection,
        new: PageCorrection,
    ) -> Result<Self, CorrectionError> {
        let page_size = |correction| {
            corrected_dimensions(width, height, correction)
                .map(|(w, h)| (612.0, 612.0 * f64::from(h) / f64::from(w)))
        };
        Ok(Self {
            old: Projection::new(old.crop)?,
            new: Projection::new(new.crop)?,
            old_size: page_size(old)?,
            new_size: page_size(new)?,
            old_turns: old.rotation_quarter_turns.rem_euclid(4),
            new_turns: new.rotation_quarter_turns.rem_euclid(4),
            identity: old.crop == new.crop
                && old.rotation_quarter_turns.rem_euclid(4)
                    == new.rotation_quarter_turns.rem_euclid(4),
        })
    }

    /// Map a center plus independent local X/Y length factors. No clamp:
    /// off-page image anchors must stay off-page until reachability handling.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn placement(&self, center: Point) -> Option<(Point, Size)> {
        if !center.x.is_finite() || !center.y.is_finite() {
            return None;
        }
        if self.identity {
            return Some((center, Size::new(1.0, 1.0)));
        }
        let (u, v) = rotate(
            f64::from(center.x) / self.old_size.0,
            f64::from(center.y) / self.old_size.1,
            -self.old_turns,
        );
        let source = self.old.map_unclamped(u, v);
        let mapped = to_page(&self.new, self.new_size, self.new_turns, source);
        // Compare magnification in fixed source axes, not the composed warp.
        // Ratios invert on a crop/reset round trip without size drift.
        let ratio = |dx, dy| {
            let length = |projection: &Projection, size, turns| {
                let a = to_page(
                    projection,
                    size,
                    turns,
                    (source.0 - dx, source.1 - dy),
                );
                let b = to_page(
                    projection,
                    size,
                    turns,
                    (source.0 + dx, source.1 + dy),
                );
                (b.0 - a.0).hypot(b.1 - a.1)
            };
            length(&self.new, self.new_size, self.new_turns)
                / length(&self.old, self.old_size, self.old_turns)
        };
        let sx = ratio(1e-6, 0.0);
        let sy = ratio(0.0, 1e-6);
        let values = [mapped.0, mapped.1, sx, sy];
        if values.iter().any(|v| !v.is_finite() || v.abs() > 1e18)
            || sx <= 0.0
            || sy <= 0.0
        {
            return None;
        }
        Some((
            Point::new(mapped.0 as f32, mapped.1 as f32),
            Size::new(sx as f32, sy as f32),
        ))
    }
}

fn to_page(
    projection: &Projection,
    size: (f64, f64),
    turns: i32,
    source: (f64, f64),
) -> (f64, f64) {
    let (u, v) = projection.inverse(source.0, source.1);
    let (u, v) = rotate(u, v, turns);
    (u * size.0, v * size.1)
}

fn rotate(u: f64, v: f64, turns: i32) -> (f64, f64) {
    match turns.rem_euclid(4) {
        1 => (1.0 - v, u),
        2 => (1.0 - u, 1.0 - v),
        3 => (v, 1.0 - u),
        _ => (u, v),
    }
}
