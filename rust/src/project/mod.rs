mod archive;
mod editor;
mod editor_transform;
mod model;
mod save;
mod validate;

pub use archive::{
    LoadedAsset, LoadedProject, MAXIMUM_ARCHIVE_CONTENT_BYTES,
    MAXIMUM_MANIFEST_BYTES, ProjectLoadError, load_archive, load_archive_file,
    load_archive_with_images,
};
pub(crate) use editor::EditorStorage;
pub use editor::{
    ProjectEditError, decode_editor_page, editor_page_size,
    encode_corrected_editor_page, encode_editor_page, manifest_size,
};
pub use model::{
    AffineTransform, CropQuad, NormalizedPoint, NormalizedRect, PageFilterMode,
    PageFilterSettings, Project, ProjectAnnotation, ProjectPage,
    ProjectShapeKind,
};
#[cfg(not(target_arch = "wasm32"))]
pub use save::save_archive_file;
pub use save::{ProjectSaveError, save_archive};
pub use validate::MAXIMUM_PROJECT_PIXELS;
