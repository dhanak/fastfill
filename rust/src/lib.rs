//! Platform-independent `FastFill` document and editor logic.

pub mod annotations;
pub mod correction;
pub mod geometry;
pub mod image_input;
pub mod interaction;
#[cfg(feature = "pdf-import")]
pub mod pdf_import;
#[cfg(feature = "pdf")]
pub mod pdf_probe;
pub mod project;
pub mod rendering;
