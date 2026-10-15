use super::archive::{
    LoadedProject, MAXIMUM_ARCHIVE_CONTENT_BYTES, MAXIMUM_MANIFEST_BYTES,
    ProjectLoadError, validate_asset,
};
use super::validate::validate_project;
use crate::image_input::{validate_image, validate_image_length};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
#[cfg(not(target_arch = "wasm32"))]
use std::io::SeekFrom;
use std::io::{Seek, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug)]
pub enum ProjectSaveError {
    Io(std::io::Error),
    Invalid(String),
}

impl Display for ProjectSaveError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => {
                write!(formatter, "Could not save project: {error}")
            }
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl Error for ProjectSaveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl From<std::io::Error> for ProjectSaveError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProjectLoadError> for ProjectSaveError {
    fn from(error: ProjectLoadError) -> Self {
        match error {
            ProjectLoadError::Io(error) => Self::Io(error),
            ProjectLoadError::Invalid(message) => Self::Invalid(message),
        }
    }
}

/// Validate and write a C# schema-v1 compatible `.docscan` archive.
///
/// # Errors
///
/// Returns an error when the project or a referenced asset is invalid, or
/// when serialization or output fails. A failed write may leave partial data
/// in `writer`; use `save_archive_file` for atomic native file replacement.
pub fn save_archive<W>(
    loaded: &LoadedProject,
    writer: W,
) -> Result<W, ProjectSaveError>
where
    W: Write + Seek,
{
    let contents = validate_contents(loaded)?;
    let mut archive = ZipWriter::new(writer);
    write_entry(
        &mut archive,
        "project.json",
        &contents.manifest,
        CompressionMethod::Deflated,
    )?;
    for (name, bytes) in contents.assets {
        write_entry(
            &mut archive,
            &format!("assets/{name}"),
            bytes,
            CompressionMethod::Stored,
        )?;
    }
    archive.finish().map_err(zip_error)
}

/// Atomically replace a native `.docscan` file after validating its bytes.
///
/// # Errors
///
/// Returns an error without replacing an existing destination when project
/// validation, writing, read-back validation, or the atomic commit fails.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_archive_file(
    loaded: &LoadedProject,
    destination: impl AsRef<Path>,
) -> Result<(), ProjectSaveError> {
    use atomicwrites::{AllowOverwrite, AtomicFile, Error as AtomicError};
    use std::fs::OpenOptions;

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(true);
    AtomicFile::new(destination, AllowOverwrite)
        .write_with_options(
            |file| {
                let _ = save_archive(loaded, &mut *file)?;
                file.flush()?;
                file.seek(SeekFrom::Start(0))?;
                verify_read_back(loaded, &mut *file)
            },
            options,
        )
        .map_err(|error| match error {
            AtomicError::Internal(error) => ProjectSaveError::Io(error),
            AtomicError::User(error) => error,
        })
}

struct ArchiveContents<'a> {
    manifest: Vec<u8>,
    assets: Vec<(&'a str, &'a [u8])>,
}

fn validate_contents(
    loaded: &LoadedProject,
) -> Result<ArchiveContents<'_>, ProjectSaveError> {
    validate_project(&loaded.project)?;
    let manifest =
        serde_json::to_vec_pretty(&loaded.project).map_err(|error| {
            invalid(format!("Could not serialize project manifest: {error}"))
        })?;
    if manifest.len() as u64 > MAXIMUM_MANIFEST_BYTES {
        return Err(invalid("Project manifest is too large."));
    }

    let mut names = BTreeSet::new();
    for page in &loaded.project.pages {
        let asset = loaded.assets.get(&page.source_asset).ok_or_else(|| {
            invalid(format!("Page asset is missing: {}", page.source_asset))
        })?;
        validate_image_length(asset.bytes.len()).map_err(|error| {
            invalid(format!("Page asset is invalid: {error}"))
        })?;
        let info = validate_image(&asset.bytes).map_err(|error| {
            invalid(format!("Page asset is invalid: {error}"))
        })?;
        validate_asset(page, info)?;
        names.insert(page.source_asset.as_str());
    }

    let mut expanded = manifest.len() as u64;
    let assets = names
        .into_iter()
        .map(|name| {
            let bytes = loaded
                .assets
                .get(name)
                .expect("referenced assets were checked above")
                .bytes
                .as_slice();
            expanded = expanded
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| invalid("Expanded project is too large."))?;
            Ok((name, bytes))
        })
        .collect::<Result<Vec<_>, ProjectSaveError>>()?;
    if expanded > MAXIMUM_ARCHIVE_CONTENT_BYTES {
        return Err(invalid("Expanded project is too large."));
    }
    Ok(ArchiveContents { manifest, assets })
}

fn write_entry<W>(
    archive: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    compression: CompressionMethod,
) -> Result<(), ProjectSaveError>
where
    W: Write + Seek,
{
    let options = SimpleFileOptions::default().compression_method(compression);
    archive.start_file(name, options).map_err(zip_error)?;
    archive.write_all(bytes)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn verify_read_back<R: Read + Seek>(
    expected: &LoadedProject,
    reader: R,
) -> Result<(), ProjectSaveError> {
    let mut archive = zip::ZipArchive::new(reader).map_err(zip_error)?;
    let names = expected
        .project
        .pages
        .iter()
        .map(|page| page.source_asset.as_str())
        .collect::<BTreeSet<_>>();
    if archive.len() != names.len() + 1 {
        return Err(invalid("Saved project failed read-back validation."));
    }
    let manifest = serde_json::to_vec_pretty(&expected.project)
        .map_err(|error| invalid(error.to_string()))?;
    verify_entry(&mut archive, "project.json", &manifest)?;
    // Input pixels were fully validated before writing. Byte equality plus
    // ZIP CRC verifies them without a second image decode.
    for name in names {
        verify_entry(
            &mut archive,
            &format!("assets/{name}"),
            &expected.assets[name].bytes,
        )?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn verify_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    expected: &[u8],
) -> Result<(), ProjectSaveError> {
    let mut actual = Vec::new();
    archive
        .by_name(name)
        .map_err(zip_error)?
        .take(expected.len() as u64 + 1)
        .read_to_end(&mut actual)?;
    if actual != expected {
        return Err(invalid("Saved project failed read-back validation."));
    }
    Ok(())
}

fn zip_error(error: zip::result::ZipError) -> ProjectSaveError {
    match error {
        zip::result::ZipError::Io(error) => ProjectSaveError::Io(error),
        error => invalid(format!("Could not create project archive: {error}")),
    }
}

fn invalid(message: impl Into<String>) -> ProjectSaveError {
    ProjectSaveError::Invalid(message.into())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn readback_rejects_changed_short_long_missing_and_corrupt_entries() {
        for bytes in [
            b"expected".as_slice(),
            b"changed",
            b"short",
            b"expected plus extra",
        ] {
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            write_entry(&mut writer, "asset", bytes, CompressionMethod::Stored)
                .unwrap();
            let original = writer.finish().unwrap().into_inner();
            let mut archive =
                zip::ZipArchive::new(Cursor::new(&original)).unwrap();
            assert_eq!(
                verify_entry(&mut archive, "asset", b"expected").is_ok(),
                bytes == b"expected"
            );
            assert!(
                verify_entry(&mut archive, "missing", b"expected").is_err()
            );
            let mut damaged = original;
            // Flip content without updating the ZIP CRC.
            let offset = damaged
                .windows(bytes.len())
                .position(|window| window == bytes)
                .unwrap();
            damaged[offset] ^= 1;
            let mut archive =
                zip::ZipArchive::new(Cursor::new(damaged)).unwrap();
            assert!(verify_entry(&mut archive, "asset", bytes).is_err());
        }
    }
}
