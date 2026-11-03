//! Calibrated four-frame consensus and virtual-time capture gating.

use super::{ACCEPTANCE, DocumentDetection, quad_distance};
use crate::project::{CropQuad, NormalizedPoint};
use serde::Serialize;
use std::collections::VecDeque;
use std::time::Duration;

const AGREEMENT: f32 = 0.08;

#[derive(Debug, Default)]
pub struct DetectionStabilizer {
    history: VecDeque<DocumentDetection>,
    hint: Option<CropQuad>,
}

impl DetectionStabilizer {
    #[must_use]
    pub const fn hint(&self) -> Option<CropQuad> {
        self.hint
    }

    /// Keep at most four scalar results, never source pixels. Reset between
    /// camera sessions, pauses, device/orientation changes, and input errors.
    pub fn reset(&mut self) {
        self.history.clear();
        self.hint = None;
    }

    /// Two agreeing accepted results establish a frame. Retain the current
    /// cluster on a tied vote, then use medians and 0.35 exponential smoothing.
    pub fn update(
        &mut self,
        detection: &DocumentDetection,
    ) -> DocumentDetection {
        let mut current = detection.clone();
        current.corners = accepted_corners(detection);
        self.history.push_back(current);
        if self.history.len() > 4 {
            self.history.pop_front();
        }
        let found = self
            .history
            .iter()
            .filter_map(|d| d.corners.map(|quad| (d, quad)))
            .collect::<Vec<_>>();
        let matching = |quad| {
            found
                .iter()
                .copied()
                .filter(|(_, corners)| {
                    quad_distance(*corners, quad) <= AGREEMENT
                })
                .collect::<Vec<_>>()
        };
        let mut cluster = self.hint.map_or_else(Vec::new, matching);
        if cluster.len() < 2 {
            cluster.clear();
            for (_, seed) in &found {
                let next = matching(*seed);
                if next.len() > cluster.len()
                    || (next.len() == cluster.len()
                        && next.iter().map(|(d, _)| d.confidence).sum::<f64>()
                            > cluster.iter().map(|(d, _)| d.confidence).sum())
                {
                    cluster = next;
                }
            }
        }
        let mut result = DocumentDetection {
            corners: None,
            candidate_corners: None,
            confidence: 0.0,
            area_ratio: 0.0,
            sharpness: if detection.sharpness.is_finite() {
                detection.sharpness.max(0.0)
            } else {
                0.0
            },
            analysis_width: detection.analysis_width,
            analysis_height: detection.analysis_height,
        };
        if cluster.len() < 2 {
            if self.history.len() == 4 {
                self.hint = None;
            }
            result.candidate_corners = self.hint;
            return result;
        }
        let points = std::array::from_fn::<_, 4, _>(|i| {
            let coordinate = |x| {
                median(cluster.iter().map(|(_, quad)| {
                    let p = quad.points()[i];
                    f64::from(if x { p.x } else { p.y })
                }))
            };
            #[allow(clippy::cast_possible_truncation)]
            NormalizedPoint {
                x: coordinate(true) as f32,
                y: coordinate(false) as f32,
            }
        });
        let target = quad(points);
        // Coordinate-wise medians need not preserve convexity in general.
        if !target.is_valid() {
            self.hint = None;
            return result;
        }
        let corners = self.hint.map_or(target, |old| {
            if quad_distance(old, target) > AGREEMENT {
                return target;
            }
            let points = std::array::from_fn(|i| {
                let (a, b) = (old.points()[i], target.points()[i]);
                NormalizedPoint {
                    x: a.x + (b.x - a.x) * 0.35,
                    y: a.y + (b.y - a.y) * 0.35,
                }
            });
            let smooth = quad(points);
            if smooth.is_valid() { smooth } else { target }
        });
        self.hint = Some(corners);
        result.corners = Some(corners);
        result.confidence = median(cluster.iter().map(|(d, _)| d.confidence));
        result.area_ratio = median(cluster.iter().map(|(d, _)| d.area_ratio));
        result
    }
}

fn quad(points: [NormalizedPoint; 4]) -> CropQuad {
    CropQuad {
        top_left: points[0],
        top_right: points[1],
        bottom_right: points[2],
        bottom_left: points[3],
    }
}

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        values[middle - 1].midpoint(values[middle])
    } else {
        values[middle]
    }
}

fn accepted_corners(detection: &DocumentDetection) -> Option<CropQuad> {
    (detection.confidence.is_finite()
        && (ACCEPTANCE..=1.0).contains(&detection.confidence)
        && detection.area_ratio.is_finite()
        && (0.0..=1.0).contains(&detection.area_ratio)
        && detection.sharpness.is_finite()
        && detection.sharpness >= 0.0)
        .then_some(detection.corners)
        .flatten()
        .filter(|quad| quad.is_valid())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureState {
    NoDocument,
    HoldSteady,
    Ready,
    Captured,
}

#[derive(Debug, Default)]
pub struct AutoCaptureGate {
    anchor: Option<CropQuad>,
    stable_since: Option<Duration>,
    last_time: Option<Duration>,
    captured: bool,
    progress: f64,
}

impl AutoCaptureGate {
    // One second: user-calibrated for practical handheld capture.
    pub const COUNTDOWN: Duration = Duration::from_secs(1);

    #[must_use]
    pub const fn progress(&self) -> f64 {
        self.progress
    }

    /// Reset on capture restart, pause, device switch, or input failure.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Time is monotonic elapsed time within one session, not wall-clock time.
    /// Sharpness 25 is calibrated against the recorded laptop-camera frames.
    /// Ready fires once; missing/weak/small/blurred input resets the gate.
    pub fn evaluate(
        &mut self,
        detection: &DocumentDetection,
        timestamp: Duration,
    ) -> CaptureState {
        if self.last_time.is_some_and(|last| timestamp < last) {
            self.reset();
        }
        self.last_time = Some(timestamp);
        let Some(corners) = accepted_corners(detection).filter(|_| {
            detection.area_ratio >= 0.1 && detection.sharpness >= 25.0
        }) else {
            self.reset();
            return CaptureState::NoDocument;
        };
        if self.captured {
            return CaptureState::Captured;
        }
        if self
            .anchor
            .is_none_or(|old| quad_distance(old, corners) > 0.04)
        {
            self.anchor = Some(corners);
            self.stable_since = Some(timestamp);
            self.progress = 0.0;
            return CaptureState::HoldSteady;
        }
        let elapsed =
            timestamp.saturating_sub(self.stable_since.unwrap_or(timestamp));
        self.progress =
            (elapsed.as_secs_f64() / Self::COUNTDOWN.as_secs_f64()).min(1.0);
        if elapsed < Self::COUNTDOWN {
            CaptureState::HoldSteady
        } else {
            self.captured = true;
            CaptureState::Ready
        }
    }
}
