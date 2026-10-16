use fastfill_rust::document_detection::{
    AutoCaptureGate, CaptureState, DetectionStabilizer, DocumentDetection,
};
use fastfill_rust::project::{CropQuad, NormalizedPoint};
use std::time::Duration;

fn detection() -> DocumentDetection {
    DocumentDetection {
        corners: Some(CropQuad {
            top_left: NormalizedPoint { x: 0.2, y: 0.1 },
            top_right: NormalizedPoint { x: 0.8, y: 0.1 },
            bottom_right: NormalizedPoint { x: 0.8, y: 0.9 },
            bottom_left: NormalizedPoint { x: 0.2, y: 0.9 },
        }),
        candidate_corners: None,
        confidence: 0.85,
        area_ratio: 0.48,
        sharpness: 60.0,
        analysis_width: 1280,
        analysis_height: 720,
    }
}

fn shifted(mut detection: DocumentDetection, amount: f32) -> DocumentDetection {
    let quad = detection.corners.as_mut().unwrap();
    for point in [
        &mut quad.top_left,
        &mut quad.top_right,
        &mut quad.bottom_right,
        &mut quad.bottom_left,
    ] {
        point.x += amount;
    }
    detection
}

#[test]
fn consensus_delays_first_frame_ignores_minority_and_follows_new_majority() {
    let mut tracker = DetectionStabilizer::default();
    let base = detection();
    assert!(tracker.update(&base).corners.is_none());
    assert!(tracker.hint().is_none());
    let stable = tracker.update(&shifted(base.clone(), 0.01));
    let first = stable.corners.unwrap().top_left.x;
    assert!((first - 0.205).abs() < 0.0001);
    let other = shifted(base, -0.12);
    tracker.update(&other);
    let held = tracker.update(&other).corners.unwrap();
    assert!((held.top_left.x - first).abs() < 0.001);
    let switched = tracker.update(&other).corners.unwrap();
    assert!((switched.top_left.x - 0.08).abs() < 0.001);
    assert_eq!(tracker.hint(), Some(switched));
}

#[test]
fn consensus_uses_coordinate_medians_smoothing_and_current_sharpness() {
    let mut tracker = DetectionStabilizer::default();
    tracker.update(&detection());
    tracker.update(&detection());
    let mut nearby = shifted(detection(), 0.02);
    nearby.confidence = 0.95;
    nearby.sharpness = 10.0;
    let stable = tracker.update(&nearby);
    assert_eq!(stable.corners.unwrap().top_left.x, 0.2);
    assert_eq!(stable.sharpness, 10.0);
    let smooth = tracker.update(&nearby);
    assert!((smooth.corners.unwrap().top_left.x - 0.2035).abs() < 0.0001);
    assert!((smooth.confidence - 0.9).abs() < 0.0001);
    assert_eq!(smooth.analysis_width, 1280);
    assert_eq!(smooth.analysis_height, 720);
}

#[test]
fn missing_and_rejected_frames_age_out_consensus_and_reset_clears_hint() {
    let mut tracker = DetectionStabilizer::default();
    tracker.update(&detection());
    tracker.update(&detection());
    let mut missing = detection();
    missing.candidate_corners = missing.corners.take();
    missing.confidence = 0.7;
    tracker.update(&missing);
    assert!(tracker.update(&missing).corners.is_some());
    let lost = tracker.update(&missing);
    assert!(lost.corners.is_none());
    assert!(lost.candidate_corners.is_none());
    assert!(tracker.hint().is_none());
    assert!(tracker.update(&detection()).corners.is_none());
    tracker.reset();
    assert!(tracker.hint().is_none());
    assert!(tracker.update(&detection()).corners.is_none());
}

#[test]
fn countdown_is_one_second_with_one_ready_event_until_reset() {
    let mut gate = AutoCaptureGate::default();
    let frame = detection();
    assert_eq!(AutoCaptureGate::COUNTDOWN, Duration::from_secs(1));
    assert_eq!(
        gate.evaluate(&frame, Duration::ZERO),
        CaptureState::HoldSteady
    );
    assert_eq!(gate.progress(), 0.0);
    assert_eq!(
        gate.evaluate(&frame, Duration::from_millis(500)),
        CaptureState::HoldSteady
    );
    assert_eq!(gate.progress(), 0.5);
    assert_eq!(
        gate.evaluate(&frame, Duration::from_millis(999)),
        CaptureState::HoldSteady
    );
    assert_eq!(gate.progress(), 0.999);
    assert_eq!(
        gate.evaluate(&frame, Duration::from_secs(1)),
        CaptureState::Ready
    );
    assert_eq!(gate.progress(), 1.0);
    assert_eq!(
        gate.evaluate(&frame, Duration::from_millis(1100)),
        CaptureState::Captured
    );
    gate.reset();
    assert_eq!(gate.progress(), 0.0);
    assert_eq!(
        gate.evaluate(&frame, Duration::from_secs(2)),
        CaptureState::HoldSteady
    );
}

#[test]
fn drift_is_measured_from_countdown_anchor_not_previous_frame() {
    let mut gate = AutoCaptureGate::default();
    gate.evaluate(&detection(), Duration::ZERO);
    gate.evaluate(&shifted(detection(), 0.025), Duration::from_millis(500));
    assert_eq!(gate.progress(), 0.5);
    assert_eq!(
        gate.evaluate(&shifted(detection(), 0.05), Duration::from_secs(1)),
        CaptureState::HoldSteady
    );
    assert_eq!(gate.progress(), 0.0);
}

#[test]
fn invalid_weak_small_and_blurred_detections_reset_countdown() {
    let base = detection();
    let mut bad = Vec::new();
    for value in [f64::NAN, f64::INFINITY, -1.0, 0.71, 1.01] {
        bad.push(DocumentDetection {
            confidence: value,
            ..base.clone()
        });
    }
    for value in [f64::NAN, f64::INFINITY, -1.0, 0.09, 1.01] {
        bad.push(DocumentDetection {
            area_ratio: value,
            ..base.clone()
        });
    }
    for value in [f64::NAN, f64::INFINITY, -1.0, 24.9] {
        bad.push(DocumentDetection {
            sharpness: value,
            ..base.clone()
        });
    }
    let mut malformed = base.clone();
    malformed.corners.as_mut().unwrap().top_right.x = f32::NAN;
    bad.push(malformed);
    bad.push(DocumentDetection {
        corners: None,
        ..base.clone()
    });
    for frame in bad {
        let mut gate = AutoCaptureGate::default();
        gate.evaluate(&base, Duration::ZERO);
        assert_eq!(
            gate.evaluate(&frame, Duration::from_secs(2)),
            CaptureState::NoDocument,
            "{frame:?}"
        );
        assert_eq!(gate.progress(), 0.0);
        assert_eq!(
            gate.evaluate(&base, Duration::from_secs(3)),
            CaptureState::HoldSteady
        );
    }
}

#[test]
fn invalid_frames_do_not_vote_and_current_blur_cancels_a_stable_capture() {
    let mut tracker = DetectionStabilizer::default();
    let mut invalid = detection();
    invalid.confidence = f64::NAN;
    tracker.update(&invalid);
    assert!(tracker.update(&invalid).corners.is_none());
    assert!(tracker.hint().is_none());
    tracker.reset();
    let mut gate = AutoCaptureGate::default();
    tracker.update(&detection());
    let stable = tracker.update(&detection());
    gate.evaluate(&stable, Duration::ZERO);
    let mut blurry = detection();
    blurry.sharpness = 10.0;
    let stable = tracker.update(&blurry);
    assert!(stable.corners.is_some());
    assert_eq!(
        gate.evaluate(&stable, Duration::from_secs(2)),
        CaptureState::NoDocument
    );
    assert_eq!(gate.progress(), 0.0);
    let stable = tracker.update(&detection());
    assert_eq!(
        gate.evaluate(&stable, Duration::from_secs(3)),
        CaptureState::HoldSteady
    );
    assert_eq!(
        gate.evaluate(&stable, Duration::from_secs(5)),
        CaptureState::Ready
    );
    assert_eq!(
        gate.evaluate(&blurry, Duration::from_secs(6)),
        CaptureState::NoDocument
    );
    assert_eq!(
        gate.evaluate(&detection(), Duration::from_secs(7)),
        CaptureState::HoldSteady
    );
}

#[test]
fn repeated_or_backwards_timestamps_cannot_complete_the_wait() {
    let mut gate = AutoCaptureGate::default();
    let frame = detection();
    gate.evaluate(&frame, Duration::from_secs(10));
    for _ in 0..100 {
        assert_eq!(
            gate.evaluate(&frame, Duration::from_secs(10)),
            CaptureState::HoldSteady
        );
    }
    gate.evaluate(&frame, Duration::from_millis(10_500));
    assert_eq!(gate.progress(), 0.5);
    gate.evaluate(&frame, Duration::from_secs(5));
    assert_eq!(gate.progress(), 0.0);
    assert_eq!(
        gate.evaluate(&frame, Duration::from_millis(5500)),
        CaptureState::HoldSteady
    );
}
