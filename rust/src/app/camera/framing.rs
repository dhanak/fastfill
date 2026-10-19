//! One analysis job across camera sessions; preview never waits for it.

use super::{Event, FastFill, Frame, Message};
use fastfill_rust::document_detection::{
    AutoCaptureGate, CaptureState, DetectionStabilizer, DocumentDetection,
    detect_document_preview,
};
use fastfill_rust::project::CropQuad;
use iced::Task;
use std::time::{Duration, Instant};

pub(super) const MAX_AGE: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub(super) struct Tracking {
    tracker: DetectionStabilizer,
    gate: AutoCaptureGate,
    analyzed: Option<Instant>,
    first: Option<Instant>,
    previous: Option<Instant>,
    dimensions: Option<(u32, u32)>,
    pub(super) corners: Option<CropQuad>,
    pub(super) state: CaptureState,
    notice: Option<String>,
}

impl Default for Tracking {
    fn default() -> Self {
        Self {
            tracker: DetectionStabilizer::default(),
            gate: AutoCaptureGate::default(),
            analyzed: None,
            first: None,
            previous: None,
            dimensions: None,
            corners: None,
            state: CaptureState::NoDocument,
            notice: None,
        }
    }
}

impl Tracking {
    fn reset(&mut self) {
        *self = Self {
            analyzed: self.analyzed,
            ..Self::default()
        };
    }

    pub(super) fn outline(&self, now: Instant) -> Option<CropQuad> {
        self.analyzed
            .filter(|at| now.saturating_duration_since(*at) <= MAX_AGE)
            .and(self.corners)
    }

    pub(super) fn progress(&self, now: Instant) -> f64 {
        if self.outline(now).is_some() {
            self.gate.progress()
        } else {
            0.0
        }
    }

    pub(super) fn status(&self, now: Instant) -> &str {
        if let Some(notice) = &self.notice {
            notice
        } else if self.outline(now).is_some()
            && self.state == CaptureState::HoldSteady
        {
            "Hold steady — automatic capture in one second."
        } else {
            "Looking for a document. Manual capture is always available."
        }
    }

    fn update(
        &mut self,
        raw: &DocumentDetection,
        at: Instant,
        now: Instant,
        dimensions: (u32, u32),
    ) -> bool {
        self.analyzed = Some(at);
        if now.saturating_duration_since(at) > MAX_AGE {
            self.reset();
            self.notice = Some(
                "Auto-framing is catching up; manual capture is available."
                    .into(),
            );
            return false;
        }
        if self.previous.is_some_and(|old| {
            at < old || at.saturating_duration_since(old) > MAX_AGE
        }) || self.dimensions.is_some_and(|old| old != dimensions)
        {
            self.reset();
        }
        self.previous = Some(at);
        self.dimensions = Some(dimensions);
        self.notice = None;
        let first = *self.first.get_or_insert(at);
        let stable = self.tracker.update(raw);
        self.corners = stable.corners;
        self.state = self
            .gate
            .evaluate(&stable, at.saturating_duration_since(first));
        if self.state != CaptureState::Ready {
            return false;
        }
        // Consensus can outlive a miss/jump. Validate the final source frame,
        // not only its history, before freezing pixels with this crop.
        let agrees = raw.corners.zip(self.corners).is_some_and(|(a, b)| {
            a.points()
                .iter()
                .zip(b.points())
                .all(|(a, b)| (a.x - b.x).hypot(a.y - b.y) <= 0.04)
        });
        if !agrees {
            self.gate.reset();
            self.state = CaptureState::HoldSteady;
        }
        agrees
    }
}

pub(super) struct Job {
    pub(super) id: u64,
    session: u64,
    frame: Frame,
}

pub(super) fn schedule(state: &mut FastFill) -> Task<Message> {
    let Some(draft) = state
        .camera
        .draft
        .as_ref()
        .filter(|draft| !draft.captured && !draft.saving)
    else {
        return Task::none();
    };
    let Some(frame) = draft.last.as_ref() else {
        return Task::none();
    };
    if state.camera.analysis.is_some()
        || draft.tracking.analyzed == Some(frame.arrived)
    {
        return Task::none();
    }
    let id = state.project.request_id();
    let pixels = frame.pixels.clone();
    let hint = draft.tracking.tracker.hint();
    state.camera.analysis = Some(Job {
        id,
        session: draft.id,
        frame: frame.clone(),
    });
    Task::perform(
        async move {
            let (sender, receiver) = iced::futures::channel::oneshot::channel();
            std::thread::Builder::new()
                .name("fastfill-camera-framing".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(
                        std::panic::AssertUnwindSafe(|| {
                            detect_document_preview(&pixels, hint)
                                .map_err(|error| error.to_string())
                        }),
                    )
                    .unwrap_or_else(|_| Err("Framing worker failed.".into()));
                    let _ = sender.send(result);
                })
                .map_err(|error| error.to_string())?;
            receiver.await.map_err(|error| error.to_string())?
        },
        move |result| Message::Camera(Event::Framed(id, result)),
    )
}

pub(super) fn ready(
    state: &mut FastFill,
    id: u64,
    result: &Result<DocumentDetection, String>,
    now: Instant,
) -> Task<Message> {
    if state
        .camera
        .analysis
        .as_ref()
        .is_none_or(|job| job.id != id)
    {
        return Task::none();
    }
    let job = state.camera.analysis.take().unwrap();
    if let Some(draft) = &mut state.camera.draft
        && draft.id == job.session
        && !draft.captured
        && !draft.saving
    {
        match result {
            Ok(_) if !job.frame.is_recent() => {
                // Linux monotonic time may stop during sleep; wall age won't.
                draft.tracking.reset();
                draft.tracking.analyzed = Some(job.frame.arrived);
                draft.tracking.notice = Some(
                    "Old frame discarded; waiting for fresh camera images."
                        .into(),
                );
            }
            Ok(raw) => {
                if draft.tracking.update(
                    raw,
                    job.frame.arrived,
                    now,
                    job.frame.pixels.dimensions(),
                ) {
                    // Smooth tracking, not the crop of this frozen image.
                    draft.crop = raw.corners;
                    draft.last = Some(job.frame);
                    draft.running = None;
                    draft.captured = true;
                }
            }
            Err(error) => {
                draft.tracking.reset();
                draft.tracking.analyzed = Some(job.frame.arrived);
                draft.tracking.notice = Some(format!(
                    "Auto-framing failed ({error}); capture manually."
                ));
            }
        }
    }
    schedule(state)
}

#[cfg(test)]
mod tests;
