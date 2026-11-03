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
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use zip::ZipArchive;
use zip::read::{ArchiveOffset, Config};

pub const MAXIMUM_MANIFEST_BYTES: u64 = 5 * 1024 * 1024;
pub const MAXIMUM_ARCHIVE_CONTENT_BYTES: u64 = 600 * 1024 * 1024;
const MAXIMUM_ENTRIES: usize = 64;
const MAXIMUM_DIRECTORY_BYTES: u64 =
    MAXIMUM_ENTRIES as u64 * (46 + 3 * u16::MAX as u64);
const MAXIMUM_ARCHIVE_BYTES: u64 =
    MAXIMUM_ARCHIVE_CONTENT_BYTES + 16 * 1024 * 1024;

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
    mut reader: R,
    mut on_image: impl FnMut(&str, image::DynamicImage),
) -> Result<LoadedProject, ProjectLoadError>
where
    R: Read + Seek,
{
    preflight_archive(&mut reader)?;
    // Preflight requires absolute offsets; avoid zip's whole-file magic scan.
    let config = Config {
        archive_offset: ArchiveOffset::Known(0),
    };
    let mut archive =
        ZipArchive::with_config(config, reader).map_err(|error| {
            invalid(format!("Project archive is invalid: {error}"))
        })?;
    archive.validate_header_consistency().map_err(|error| {
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

fn preflight_archive<R: Read + Seek>(
    reader: &mut R,
) -> Result<(), ProjectLoadError> {
    let length = reader.seek(SeekFrom::End(0))?;
    if length > MAXIMUM_ARCHIVE_BYTES {
        return Err(invalid("Project archive is too large."));
    }
    // Cover zip's footer search window, including its in-memory header size.
    let tail_length =
        usize::try_from(length.min(66 * 1024)).expect("ZIP footer fits usize");
    let mut tail = vec![0; tail_length];
    reader.seek(SeekFrom::Start(length - tail_length as u64))?;
    reader.read_exact(&mut tail)?;
    let mut ends = tail.windows(4).enumerate().filter_map(|(index, bytes)| {
        (bytes == b"PK\x05\x06"
            && tail.len() - index >= 22
            && tail.len() - index
                == 22 + usize::from(zip_u16(&tail[index..], 20)))
        .then_some(index)
    });
    let end = ends
        .next()
        .ok_or_else(|| invalid("Project archive footer is missing."))?;
    if ends.next().is_some() {
        return Err(invalid("Project archive footer is ambiguous."));
    }
    reject_alternate_footers(reader, &tail, length - tail_length as u64, end)?;
    let footer = &tail[end..];
    if zip_u16(footer, 4) != 0 || zip_u16(footer, 6) != 0 {
        return Err(invalid("Multi-disk projects are unsupported."));
    }
    let footer_offset = length - footer.len() as u64;
    let mut directory_end = footer_offset;
    let mut count = u64::from(zip_u16(footer, 10));
    let mut bytes = u64::from(zip_u32(footer, 12));
    let mut offset = u64::from(zip_u32(footer, 16));
    if zip_u16(footer, 8) != zip_u16(footer, 10) {
        return Err(invalid("Project archive file counts are inconsistent."));
    }
    if count == u64::from(u16::MAX)
        || bytes == u64::from(u32::MAX)
        || offset == u64::from(u32::MAX)
    {
        (count, bytes, offset, directory_end) =
            preflight_zip64(reader, footer_offset)?;
    }
    if count > MAXIMUM_ENTRIES as u64 {
        return Err(invalid("Project has too many files."));
    }
    if bytes > MAXIMUM_DIRECTORY_BYTES {
        return Err(invalid("Project archive directory is too large."));
    }
    if offset.checked_add(bytes) != Some(directory_end) {
        return Err(invalid("Project archive directory bounds are invalid."));
    }
    for _ in 0..count {
        if directory_end.saturating_sub(offset) < 46 {
            return Err(invalid("Project archive directory is truncated."));
        }
        let mut header = [0; 46];
        reader.seek(SeekFrom::Start(offset))?;
        reader.read_exact(&mut header)?;
        if &header[..4] != b"PK\x01\x02" {
            return Err(invalid("Project archive directory is invalid."));
        }
        offset += 46
            + u64::from(zip_u16(&header, 28))
            + u64::from(zip_u16(&header, 30))
            + u64::from(zip_u16(&header, 32));
        if offset > directory_end {
            return Err(invalid("Project archive directory is truncated."));
        }
    }
    if offset != directory_end {
        return Err(invalid("Project archive file count is inconsistent."));
    }
    Ok(())
}

fn reject_alternate_footers<R: Read + Seek>(
    reader: &mut R,
    tail: &[u8],
    tail_offset: u64,
    checked: usize,
) -> Result<(), ProjectLoadError> {
    // zip retries older footers after a metadata error. Ignore harmless image
    // bytes, but reject alternatives that could allocate unchecked metadata.
    for (index, magic) in tail.windows(4).enumerate() {
        if index == checked || magic != b"PK\x05\x06" {
            continue;
        }
        let footer = &tail[index..];
        if footer.len() < 22
            || 22 + usize::from(zip_u16(footer, 20)) > footer.len()
        {
            continue;
        }
        let position = tail_offset + index as u64;
        let count = zip_u16(footer, 10);
        let offset = u64::from(zip_u32(footer, 16));
        if position >= 20
            && (count == u16::MAX
                || zip_u32(footer, 12) == u32::MAX
                || offset == u64::from(u32::MAX))
        {
            let mut locator = [0; 4];
            reader.seek(SeekFrom::Start(position - 20))?;
            reader.read_exact(&mut locator)?;
            if &locator == b"PK\x06\x07" {
                return Err(invalid("Project archive footer is ambiguous."));
            }
        }
        if count > 0 && offset < position {
            let mut directory = [0; 4];
            reader.seek(SeekFrom::Start(offset))?;
            reader.read_exact(&mut directory)?;
            if &directory == b"PK\x01\x02" {
                return Err(invalid("Project archive footer is ambiguous."));
            }
        }
    }
    Ok(())
}

fn preflight_zip64<R: Read + Seek>(
    reader: &mut R,
    footer_offset: u64,
) -> Result<(u64, u64, u64, u64), ProjectLoadError> {
    let locator_offset = footer_offset
        .checked_sub(20)
        .ok_or_else(|| invalid("Project ZIP64 footer is invalid."))?;
    let mut locator = [0; 20];
    reader.seek(SeekFrom::Start(locator_offset))?;
    reader.read_exact(&mut locator)?;
    if &locator[..4] != b"PK\x06\x07"
        || zip_u32(&locator, 4) != 0
        || zip_u32(&locator, 16) != 1
    {
        return Err(invalid("Project ZIP64 locator is invalid."));
    }
    let directory_end = zip_u64(&locator, 8);
    // Extensible ZIP64 metadata is unnecessary for project assets.
    if directory_end.checked_add(56) != Some(locator_offset) {
        return Err(invalid("Project ZIP64 footer is too large or invalid."));
    }
    let mut zip64 = [0; 56];
    reader.seek(SeekFrom::Start(directory_end))?;
    reader.read_exact(&mut zip64)?;
    if &zip64[..4] != b"PK\x06\x06"
        || zip_u64(&zip64, 4) != 44
        || zip_u32(&zip64, 16) != 0
        || zip_u32(&zip64, 20) != 0
        || zip_u64(&zip64, 24) != zip_u64(&zip64, 32)
    {
        return Err(invalid("Project ZIP64 footer is invalid."));
    }
    Ok((
        zip_u64(&zip64, 32),
        zip_u64(&zip64, 40),
        zip_u64(&zip64, 48),
        directory_end,
    ))
}

fn zip_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn zip_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn zip_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
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
