//! Platform-independent `FastFill` document and editor logic.

pub mod annotations;
#[cfg(all(feature = "native-camera", not(target_arch = "wasm32")))]
pub mod camera;
pub mod correction;
pub mod document_detection;
pub mod form_snap;
pub mod geometry;
mod image_color;
pub mod image_input;
pub mod interaction;
#[cfg(feature = "pdf-import")]
pub mod pdf_import;
#[cfg(feature = "pdf")]
pub mod pdf_probe;
pub mod project;
pub mod rendering;
