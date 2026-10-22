use super::model::Project;
use super::validate::{
    MAXIMUM_IMAGE_BYTES, validate_asset_name, validate_project,
};
use crate::image_input::{
    ImageFormatKind, ImageInfo, decode_image, validate_image_length,
};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;
use zip::ZipArchive;

pub(super) const MAXIMUM_MANIFEST_BYTES: u64 = 5 * 1024 * 1024;
pub(super) const MAXIMUM_ARCHIVE_CONTENT_BYTES: u64 = 600 * 1024 * 1024;
const MAXIMUM_ENTRIES: usize = 64;

#[derive(Clone, Debug)]
pub struct LoadedAsset {
    pub bytes: Vec<u8>,
    pub info: ImageInfo,
}

#[derive(Clone, Debug)]
pub struct LoadedProject {
    pub project: Project,
    pub assets: BTreeMap<String, LoadedAsset>,
}

impl LoadedProject {
    #[must_use]
    pub fn asset(&self, name: &str) -> Option<&LoadedAsset> {
        self.assets.get(name)
    }
}

#[derive(Debug)]
pub enum ProjectLoadError {
    Io(std::io::Error),
    Invalid(String),
}

impl Display for ProjectLoadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(formatter, "Could not read project: {error}")
            }
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl Error for ProjectLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl From<std::io::Error> for ProjectLoadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Load and validate a `.docscan` file.
///
/// # Errors
///
/// Returns an I/O error when the file cannot be read, or an invalid-project
/// error when its ZIP, manifest, model, or page assets fail validation.
pub fn load_archive_file(
    path: impl AsRef<Path>,
) -> Result<LoadedProject, ProjectLoadError> {
    load_archive(File::open(path)?)
}

/// Load and validate a `.docscan` stream.
///
/// # Errors
///
/// Returns an invalid-project error for malformed, unsafe, ambiguous, or
/// unsupported archives and an I/O error if reading fails.
pub fn load_archive<R>(reader: R) -> Result<LoadedProject, ProjectLoadError>
where
    R: Read + Seek,
{
    load_archive_with_images(reader, |_, _| {})
}

/// Load and validate an archive, passing decoded pixels to a staging callback.
/// The callback runs once per referenced asset; keep its results private until
/// the whole load succeeds. This avoids decoding again on the UI thread.
///
/// # Errors
/// Returns the same validation and I/O errors as `load_archive`.
pub fn load_archive_with_images<R>(
    reader: R,
    mut on_image: impl FnMut(&str, image::DynamicImage),
) -> Result<LoadedProject, ProjectLoadError>
where
    R: Read + Seek,
{
    let mut archive = ZipArchive::new(reader).map_err(|error| {
        invalid(format!("Project archive is invalid: {error}"))
    })?;
    let entries = inspect_archive(&mut archive)?;
    let manifest_name = entries
        .get("project.json")
        .ok_or_else(|| invalid("Project manifest is missing."))?;
    let manifest = read_entry(
        &mut archive,
        manifest_name,
        MAXIMUM_MANIFEST_BYTES,
        "Project manifest is too large.",
    )?;
    let project: Project =
        serde_json::from_slice(&manifest).map_err(|error| {
            invalid(format!("Project manifest is invalid: {error}"))
        })?;
    validate_project(&project)?;

    let mut assets = BTreeMap::new();
    for page in &project.pages {
        let archive_name = format!("assets/{}", page.source_asset);
        let stored_name = entries.get(&archive_name).ok_or_else(|| {
            invalid(format!("Page asset is missing: {}", page.source_asset))
        })?;
        let bytes = read_entry(
            &mut archive,
            stored_name,
            MAXIMUM_IMAGE_BYTES,
            "Page asset is too large.",
        )?;
        validate_image_length(bytes.len()).map_err(|error| {
            invalid(format!("Page asset is invalid: {error}"))
        })?;
        let (info, pixels) = decode_image(&bytes).map_err(|error| {
            invalid(format!("Page asset is invalid: {error}"))
        })?;
        validate_asset(page, info)?;
        on_image(&page.source_asset, pixels);
        assets.insert(page.source_asset.clone(), LoadedAsset { bytes, info });
    }

    Ok(LoadedProject { project, assets })
}

fn inspect_archive<R>(
    archive: &mut ZipArchive<R>,
) -> Result<HashMap<String, String>, ProjectLoadError>
where
    R: Read + Seek,
{
    if archive.len() > MAXIMUM_ENTRIES {
        return Err(invalid("Project has too many files."));
    }
    let mut entries = HashMap::new();
    let mut expanded_bytes = 0_u64;
    let mut compressed_bytes = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            invalid(format!("Project archive is invalid: {error}"))
        })?;
        if !entry.is_file() {
            return Err(invalid("Project entries must be regular files."));
        }
        expanded_bytes = expanded_bytes
            .checked_add(entry.size())
            .ok_or_else(|| invalid("Expanded project is too large."))?;
        compressed_bytes = compressed_bytes
            .checked_add(entry.compressed_size())
            .ok_or_else(|| invalid("Project archive is too large."))?;
        if expanded_bytes > MAXIMUM_ARCHIVE_CONTENT_BYTES {
            return Err(invalid("Expanded project is too large."));
        }
        if compressed_bytes > MAXIMUM_ARCHIVE_CONTENT_BYTES {
            return Err(invalid("Project archive is too large."));
        }

        let original = entry
            .name()
            .map_err(|error| {
                invalid(format!("Project archive is invalid: {error}"))
            })?
            .into_owned();
        let normalized = original.replace('\\', "/");
        if !valid_entry_name(&normalized) {
            return Err(invalid(format!("Unsafe project entry: {original}")));
        }
        if entries.insert(normalized.clone(), original).is_some() {
            return Err(invalid(format!(
                "Duplicate project entry: {normalized}"
            )));
        }
    }
    Ok(entries)
}

fn valid_entry_name(name: &str) -> bool {
    if name == "project.json" {
        return true;
    }
    let Some(asset) = name.strip_prefix("assets/") else {
        return false;
    };
    !asset.contains('/') && validate_asset_name(asset).is_ok()
}

fn read_entry<R>(
    archive: &mut ZipArchive<R>,
    name: &str,
    maximum: u64,
    too_large: &str,
) -> Result<Vec<u8>, ProjectLoadError>
where
    R: Read + Seek,
{
    let mut entry = archive.by_name(name).map_err(|error| {
        invalid(format!("Project archive is invalid: {error}"))
    })?;
    let expected = entry.size();
    if expected > maximum {
        return Err(invalid(too_large));
    }
    let capacity = usize::try_from(expected).map_err(|_| invalid(too_large))?;
    let mut bytes = Vec::with_capacity(capacity);
    (&mut entry)
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(ProjectLoadError::Io)?;
    if bytes.len() as u64 != expected || bytes.len() as u64 > maximum {
        return Err(invalid(too_large));
    }
    Ok(bytes)
}

pub(super) fn validate_asset(
    page: &super::model::ProjectPage,
    info: ImageInfo,
) -> Result<(), ProjectLoadError> {
    let expected_width = u32::try_from(page.source_pixel_width)
        .map_err(|_| invalid("Page asset dimensions are invalid."))?;
    let expected_height = u32::try_from(page.source_pixel_height)
        .map_err(|_| invalid("Page asset dimensions are invalid."))?;
    if info.width != expected_width || info.height != expected_height {
        return Err(invalid(format!(
            "Page asset dimensions do not match the manifest: {}",
            page.source_asset
        )));
    }
    let extension = page
        .source_asset
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    let matches = match info.format {
        ImageFormatKind::Jpeg => matches!(extension.as_str(), "jpg" | "jpeg"),
        ImageFormatKind::Png => extension == "png",
    };
    if !matches {
        return Err(invalid(format!(
            "Page asset format does not match its extension: {}",
            page.source_asset
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> ProjectLoadError {
    ProjectLoadError::Invalid(message.into())
}
