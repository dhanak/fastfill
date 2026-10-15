//! File dialogs and archive work run outside the application update loop.

use crate::picker::ImportedImage;
use fastfill_rust::project::{
    LoadedAsset, LoadedProject, Project, decode_editor_page,
    load_archive_with_images,
};
use iced::widget::image::Handle;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct OpenedProject {
    pub(super) project: Project,
    pub(super) images: Vec<ImportedImage>,
    pub(super) path: Option<PathBuf>,
    pub(super) name: String,
}

#[derive(Debug)]
pub(super) struct SaveSnapshot {
    pub(super) project: Project,
    pub(super) images: Vec<Option<Handle>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SaveReceipt {
    pub(super) path: Option<PathBuf>,
    pub(super) name: String,
}

pub(super) fn prepare_opened(
    loaded: LoadedProject,
    path: Option<PathBuf>,
    name: String,
) -> Result<OpenedProject, String> {
    let directory = path
        .as_ref()
        .and_then(|p| p.parent())
        .map(std::path::Path::to_path_buf);
    let images = loaded
        .assets
        .into_iter()
        .map(|(name, asset)| {
            let image = ImportedImage::from_validated(
                name.clone(),
                asset.bytes,
                asset.info,
                directory.clone(),
            );
            (name, image)
        })
        .collect::<BTreeMap<_, _>>();
    let pages = loaded
        .project
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            decode_editor_page(page).map_err(|error| {
                format!("Page {} cannot be edited yet: {error}", index + 1)
            })?;
            images
                .get(&page.source_asset)
                .cloned()
                .ok_or_else(|| "Page image is missing.".into())
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(OpenedProject {
        project: loaded.project,
        images: pages,
        path,
        name,
    })
}

pub(super) async fn open_project(
    directory: Option<PathBuf>,
) -> Result<Option<Arc<OpenedProject>>, String> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Open project")
        .add_filter("FastFill project", &["docscan"]);
    if let Some(directory) = directory.filter(|p| p.is_dir()) {
        dialog = dialog.set_directory(directory);
    }
    let Some(file) = dialog.pick_file().await else {
        return Ok(None);
    };
    let mut pixels = BTreeMap::new();
    let mut on_image = |name: &str, image: image::DynamicImage| {
        let image = image.into_rgba8();
        pixels.insert(
            name.to_owned(),
            Handle::from_rgba(image.width(), image.height(), image.into_raw()),
        );
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (loaded, path) = (
        load_archive_with_images(
            std::fs::File::open(file.path())
                .map_err(|error| format!("Could not read project: {error}"))?,
            &mut on_image,
        )
        .map_err(|error| error.to_string())?,
        Some(file.path().to_path_buf()),
    );
    #[cfg(target_arch = "wasm32")]
    let (loaded, path) = {
        // Check browser file size before its read allocates the whole ZIP.
        if file.inner().size() > 610.0 * 1024.0 * 1024.0 {
            return Err("Project file exceeds the browser size limit.".into());
        }
        (
            load_archive_with_images(
                Cursor::new(file.read().await),
                &mut on_image,
            )
            .map_err(|error| error.to_string())?,
            None,
        )
    };
    let mut opened = prepare_opened(loaded, path, file.file_name())?;
    for image in &mut opened.images {
        image.display = pixels.get(&image.name).cloned();
    }
    Ok(Some(Arc::new(opened)))
}

pub(super) fn encode_snapshot(
    snapshot: SaveSnapshot,
) -> Result<LoadedProject, String> {
    if snapshot.images.len() != snapshot.project.pages.len() {
        return Err("Page image count does not match the document.".into());
    }
    let mut assets = BTreeMap::new();
    for (page, handle) in snapshot.project.pages.iter().zip(snapshot.images) {
        if assets.contains_key(&page.source_asset) {
            continue;
        }
        let bytes = match handle {
            Some(Handle::Bytes(_, bytes)) => bytes.as_ref().to_vec(),
            Some(Handle::Rgba {
                width,
                height,
                pixels,
                ..
            }) => {
                let image = image::RgbaImage::from_raw(
                    width,
                    height,
                    pixels.as_ref().to_vec(),
                )
                .ok_or_else(|| "Page pixels are invalid.".to_string())?;
                encode_png(&image::DynamicImage::ImageRgba8(image))?
            }
            Some(Handle::Path(_, _)) => {
                return Err(
                    "Page image data must be loaded before saving.".into()
                );
            }
            None => {
                let width = u32::try_from(page.source_pixel_width)
                    .map_err(|error| error.to_string())?;
                let height = u32::try_from(page.source_pixel_height)
                    .map_err(|error| error.to_string())?;
                encode_png(&image::DynamicImage::ImageRgb8(
                    image::RgbImage::from_pixel(
                        width,
                        height,
                        image::Rgb([255, 255, 255]),
                    ),
                ))?
            }
        };
        // The archive writer performs full validation before any file commit.
        let info = fastfill_rust::image_input::inspect_image(&bytes)
            .map_err(|error| error.to_string())?;
        assets.insert(page.source_asset.clone(), LoadedAsset { bytes, info });
    }
    Ok(LoadedProject {
        project: snapshot.project,
        assets,
    })
}

fn encode_png(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| format!("Could not encode page image: {error}"))?;
    Ok(bytes.into_inner())
}

pub(super) async fn save_project(
    snapshot: SaveSnapshot,
    path: Option<PathBuf>,
    directory: Option<PathBuf>,
    name: String,
) -> Result<Option<SaveReceipt>, String> {
    // Native Save writes the existing path; Save as always uses the dialog.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = path {
        let archive = encode_snapshot(snapshot)?;
        fastfill_rust::project::save_archive_file(&archive, &path)
            .map_err(|error| error.to_string())?;
        return Ok(Some(receipt(path)));
    }
    #[cfg(target_arch = "wasm32")]
    let _ = path;
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Save project")
        .set_file_name(&name)
        .add_filter("FastFill project", &["docscan"]);
    if let Some(directory) = directory.filter(|p| p.is_dir()) {
        dialog = dialog.set_directory(directory);
    }
    let Some(file) = dialog.save_file().await else {
        return Ok(None);
    };
    let archive = encode_snapshot(snapshot)?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        fastfill_rust::project::save_archive_file(&archive, file.path())
            .map_err(|error| error.to_string())?;
        Ok(Some(receipt(file.path().to_path_buf())))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut zip = Cursor::new(Vec::new());
        fastfill_rust::project::save_archive(&archive, &mut zip)
            .map_err(|error| error.to_string())?;
        file.write(&zip.into_inner())
            .await
            .map_err(|error| format!("Could not download project: {error}"))?;
        Ok(Some(SaveReceipt {
            path: None,
            name: file.file_name(),
        }))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn receipt(path: PathBuf) -> SaveReceipt {
    SaveReceipt {
        name: path.file_name().map_or_else(
            || "Project.docscan".into(),
            |name| name.to_string_lossy().into_owned(),
        ),
        path: Some(path),
    }
}
