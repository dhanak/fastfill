//! Browser camera draft. Only a confirmed photo changes the project.

use super::{FastFill, ImagePicker, Message};
use crate::picker::ImportedImage;
use fastfill_rust::document_detection::{
    AutoCaptureGate, CaptureState, DetectionStabilizer, detect_document_preview,
};
use fastfill_rust::image_input::{MAX_IMAGE_BYTES, inspect_image};
use fastfill_rust::project::CropQuad;
use iced::Task;
use image::RgbaImage;
use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

#[wasm_bindgen(module = "/web/camera.mjs")]
extern "C" {
    #[wasm_bindgen(catch, js_name = capturePhoto)]
    async fn capture_photo(
        pixel_budget: u32,
        analyze: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;
}

type Captured = (ImportedImage, Option<CropQuad>);

#[derive(Default)]
struct BrowserTracking {
    stabilizer: DetectionStabilizer,
    gate: AutoCaptureGate,
    session: u32,
    first: Option<f64>,
    previous: Option<f64>,
    dimensions: Option<(u32, u32)>,
}

impl BrowserTracking {
    fn reset(&mut self) {
        self.stabilizer.reset();
        self.gate.reset();
        self.first = None;
        self.previous = None;
        self.dimensions = None;
    }

    fn result(
        &self,
        corners: Option<CropQuad>,
        capture: Option<CropQuad>,
        status: &str,
    ) -> String {
        serde_json::json!({
            "corners": corners,
            "capture": capture,
            "progress": self.gate.progress(),
            "status": status,
        })
        .to_string()
    }

    fn analyze(
        &mut self,
        bytes: &js_sys::Uint8Array,
        width: u32,
        height: u32,
        stamp: f64,
        age: f64,
        session: u32,
    ) -> String {
        if self.session != session {
            self.reset();
            self.session = session;
        }
        if !stamp.is_finite()
            || !age.is_finite()
            || stamp < 0.0
            || age < 0.0
            || age > 1_000.0
            || self
                .previous
                .is_some_and(|old| stamp < old || stamp - old > 1_000.0)
            || self.dimensions.is_some_and(|old| old != (width, height))
        {
            self.reset();
            return self.result(
                None,
                None,
                "Auto-framing is catching up; capture manually if needed.",
            );
        }
        let size = u64::from(width) * u64::from(height);
        if width.max(height) > 640
            || width < 16
            || height < 16
            || u64::from(bytes.length()) != size * 4
        {
            self.reset();
            return self.result(None, None, "Invalid camera analysis frame.");
        }
        self.previous = Some(stamp);
        self.dimensions = Some((width, height));
        let first = *self.first.get_or_insert(stamp);
        let pixels = RgbaImage::from_raw(width, height, bytes.to_vec())
            .expect("validated RGBA frame size");
        let raw = detect_document_preview(&pixels, self.stabilizer.hint());
        let Ok(raw) = raw else {
            self.reset();
            return self.result(
                None,
                None,
                "Auto-framing failed; capture manually.",
            );
        };
        let stable = self.stabilizer.update(&raw);
        let elapsed = Duration::from_secs_f64(
            ((stamp - first) / 1_000.0).clamp(0.0, 86_400.0),
        );
        let state = self.gate.evaluate(&stable, elapsed);
        let agrees = raw.corners.zip(stable.corners).is_some_and(|(a, b)| {
            a.points()
                .iter()
                .zip(b.points())
                .all(|(a, b)| (a.x - b.x).hypot(a.y - b.y) <= 0.04)
        });
        if state == CaptureState::Ready && !agrees {
            self.gate.reset();
        }
        let capture = (state == CaptureState::Ready && agrees)
            .then_some(raw.corners)
            .flatten();
        let status = if state == CaptureState::HoldSteady {
            "Hold steady — automatic capture in one second."
        } else {
            "Looking for a document. Manual capture is always available."
        };
        self.result(stable.corners, capture, status)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Begin,
    Finished(u64, Result<Option<Captured>, String>),
}

pub(super) fn can_open(state: &FastFill) -> bool {
    state.browser_camera.is_none()
        && state.project.is_idle()
        && state.image_picker == ImagePicker::Idle
        && !state.page_editor.is_open()
        && !state.confirm_delete_all
        && !state.confirm_delete_page
        && !state.show_shortcuts
        && !state.palette.open
        && state.dragged_page.is_none()
        && state.pages.can_add()
}

pub(super) fn update(
    state: &mut FastFill,
    message: &Message,
) -> Option<Task<Message>> {
    if matches!(message, Message::BrowserCamera(Event::Begin)) {
        if !can_open(state) {
            return Some(Task::none());
        }
        state.accept_text();
        state.cancel_interaction();
        let id = state.project.request_id();
        state.browser_camera = Some(id);
        state.status = "Opening camera…".into();
        state.status_is_error = false;
        let budget = state
            .pages
            .remaining_pixel_budget(state.pages.empty_starter());
        return Some(Task::perform(capture(budget), move |result| {
            Message::BrowserCamera(Event::Finished(id, result))
        }));
    }
    let Some(id) = state.browser_camera else {
        return matches!(message, Message::BrowserCamera(_)).then(Task::none);
    };
    if let Message::BrowserCamera(Event::Finished(received, result)) = message
        && *received == id
    {
        state.browser_camera = None;
        return Some(finish(state, result));
    }
    if matches!(message, Message::FocusLost) {
        state.modifiers = super::DragModifiers::default();
    }
    // The browser dialog owns input until it resolves.
    Some(Task::none())
}

async fn capture(budget: u64) -> Result<Option<Captured>, String> {
    let pixel_budget = u32::try_from(budget.min(50_000_000))
        .expect("image pixel limit fits u32");
    let mut tracking = BrowserTracking::default();
    let analyze = Closure::wrap(Box::new(
        move |pixels, width, height, stamp, age, session| {
            tracking.analyze(&pixels, width, height, stamp, age, session)
        },
    )
        as Box<
            dyn FnMut(js_sys::Uint8Array, u32, u32, f64, f64, u32) -> String,
        >);
    let value = capture_photo(pixel_budget, analyze.as_ref().unchecked_ref())
        .await
        .map_err(|error| {
            error
                .as_string()
                .unwrap_or_else(|| "Could not capture camera photo.".into())
        })?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    let field = |name: &str| {
        js_sys::Reflect::get(&value, &JsValue::from_str(name))
            .map_err(|_| "Camera returned an invalid photo.".to_owned())
    };
    let bytes = field("bytes")?
        .dyn_into::<js_sys::Uint8Array>()
        .map_err(|_| "Camera returned an invalid image.".to_owned())?;
    let limit = u32::try_from(MAX_IMAGE_BYTES).expect("100 MiB fits u32");
    if bytes.length() > limit {
        return Err("Camera photo is larger than 100 MiB.".into());
    }
    let bytes = bytes.to_vec();
    let info = inspect_image(&bytes).map_err(|error| error.to_string())?;
    if u64::from(info.width) * u64::from(info.height) > budget {
        return Err("Project images exceed the 200 MP memory limit.".into());
    }
    let crop = field("crop")?
        .as_string()
        .map(|json| {
            serde_json::from_str::<CropQuad>(&json)
                .map_err(|_| "Camera returned an invalid crop.".to_owned())
                .and_then(|crop| {
                    crop.is_valid().then_some(crop).ok_or_else(|| {
                        "Camera returned an invalid crop.".into()
                    })
                })
        })
        .transpose()?;
    ImportedImage::from_bytes("Camera photo.png".into(), bytes, None)
        .map(|image| Some((image, crop)))
}

fn finish(
    state: &mut FastFill,
    result: &Result<Option<Captured>, String>,
) -> Task<Message> {
    match result {
        Ok(None) => {
            state.status = "Capture canceled; project unchanged.".into();
            state.status_is_error = false;
        }
        Err(error) => {
            state.status.clone_from(error);
            state.status_is_error = true;
        }
        Ok(Some((image, crop))) if state.pages.add_image(image.clone()) => {
            state.navigation.fit();
            state.page_scroll =
                super::rail::page_top(state, state.pages.active_index());
            state.input_epoch = state.input_epoch.wrapping_add(1);
            state.status = "Photo added. Adjust framing if needed.".into();
            state.status_is_error = false;
            return super::page_edit::begin(state, *crop);
        }
        Ok(Some(_)) => {
            state.status = if state.pages.can_add() {
                "Project images exceed the 200 MP memory limit."
            } else {
                "A project may contain at most 20 pages."
            }
            .into();
            state.status_is_error = true;
        }
    }
    Task::none()
}
