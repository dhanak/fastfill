//! One locked snapshot per running instance; abandoned snapshots stay intact.

use super::super::project_io::{
    OpenedProject, SaveSnapshot, decode_project, encode_snapshot,
};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(super) id: Uuid,
    pub(super) modified: SystemTime,
}

struct Lease {
    candidate: Candidate,
    path: PathBuf,
    lock_path: PathBuf,
    lock: Option<File>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        // Leave snapshot locks on disk; the OS releases the actual lease.
        if !self.path.exists() {
            self.lock.take();
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

pub(super) struct Store {
    current: Lease,
    candidates: Vec<Lease>,
}

impl Store {
    pub(super) fn open(directory: &Path) -> Result<Self, String> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(directory)
            .map_err(|error| error.to_string())?;
        let id = Uuid::new_v4();
        let lock_path = directory.join(format!("{id}.lock"));
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|error| error.to_string())?;
        lock.try_lock().map_err(|error| error.to_string())?;
        let mut store = Self {
            current: Lease {
                candidate: Candidate {
                    id,
                    modified: SystemTime::now(),
                },
                path: directory.join(format!("{id}.docscan")),
                lock_path,
                lock: Some(lock),
            },
            candidates: Vec::new(),
        };
        for entry in
            fs::read_dir(directory).map_err(|error| error.to_string())?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let Some(id) = path
                .file_stem()
                .and_then(|name| name.to_str())
                .and_then(|name| Uuid::parse_str(name).ok())
            else {
                continue;
            };
            if entry.file_name() != format!("{id}.docscan").as_str()
                || !entry.file_type().map_err(|e| e.to_string())?.is_file()
            {
                continue;
            }
            let lock_path = directory.join(format!("{id}.lock"));
            // The recovery directory is private. Never follow foreign links.
            match fs::symlink_metadata(&lock_path) {
                Ok(metadata) if !metadata.is_file() => continue,
                Err(error) if error.kind() != io::ErrorKind::NotFound => {
                    return Err(error.to_string());
                }
                _ => {}
            }
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&lock_path)
                .map_err(|error| error.to_string())?;
            match lock.try_lock() {
                Ok(()) => {}
                Err(fs::TryLockError::WouldBlock) => continue,
                Err(error) => return Err(error.to_string()),
            }
            // Another instance may have cleaned up since read_dir.
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            store.candidates.push(Lease {
                candidate: Candidate {
                    id,
                    modified: metadata
                        .modified()
                        .unwrap_or(SystemTime::UNIX_EPOCH),
                },
                path,
                lock_path,
                lock: Some(lock),
            });
        }
        store
            .candidates
            .sort_by_key(|lease| lease.candidate.modified);
        Ok(store)
    }

    pub(super) fn candidate(&self) -> Option<Candidate> {
        self.candidates.last().map(|lease| lease.candidate.clone())
    }

    pub(super) fn write(&self, snapshot: SaveSnapshot) -> Result<(), String> {
        let archive = encode_snapshot(snapshot)?;
        fastfill_rust::project::save_archive_file(&archive, &self.current.path)
            .map_err(|error| error.to_string())
    }

    pub(super) fn clear(&self) -> Result<(), String> {
        remove_snapshot(&self.current.path)
    }

    pub(super) fn read(&self, id: Uuid) -> Result<OpenedProject, String> {
        let candidate = self
            .candidates
            .iter()
            .find(|lease| lease.candidate.id == id)
            .ok_or("Recovery snapshot is no longer available.")?;
        decode_project(
            File::open(&candidate.path).map_err(|error| error.to_string())?,
            None,
            "Recovered project.docscan".into(),
        )
    }

    pub(super) fn discard(&mut self, id: Uuid) -> Result<(), String> {
        let index = self
            .candidates
            .iter()
            .position(|lease| lease.candidate.id == id)
            .ok_or("Recovery snapshot is no longer available.")?;
        remove_snapshot(&self.candidates[index].path)?;
        self.candidates.remove(index);
        Ok(())
    }

    pub(super) fn adopt(&mut self, id: Uuid) -> Result<(), String> {
        let index = self
            .candidates
            .iter()
            .position(|lease| lease.candidate.id == id)
            .ok_or("Recovery snapshot is no longer available.")?;
        self.clear()?;
        self.current = self.candidates.remove(index);
        // Other abandoned snapshots remain for a later launch.
        self.candidates.clear();
        Ok(())
    }

    pub(super) fn defer(&mut self) {
        self.candidates.clear();
    }
}

fn remove_snapshot(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
