use super::*;
use crate::app::FastFill;
use crate::app::tests::annotated_page;
use std::fs;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        Self(
            std::env::temp_dir()
                .join(format!("fastfill-recovery-test-{}", Uuid::new_v4())),
        )
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn live_instances_are_isolated_and_abandoned_snapshots_are_claimed_once() {
    let directory = Directory::new();
    let first = Store::open(&directory.0).unwrap();
    first
        .write(annotated_page().project_snapshot().unwrap())
        .unwrap();
    let second = Store::open(&directory.0).unwrap();
    assert!(second.candidate().is_none());
    assert!(first.candidate().is_none());
    drop(first); // Simulates OS releasing the lock after a crash.
    let third = Store::open(&directory.0).unwrap();
    let candidate = third.candidate().expect("abandoned snapshot");
    let fourth = Store::open(&directory.0).unwrap();
    assert!(fourth.candidate().is_none());
    let opened = third.read(candidate.id).unwrap();
    assert!(opened.path.is_none());
    assert_eq!(opened.name, "Recovered project.docscan");
    assert_eq!(opened.project.pages[0].annotations.len(), 1);
    assert!(opened.images[0].display.is_some());
}

#[test]
fn rejected_snapshot_keeps_previous_recovery_and_adoption_survives_restart() {
    let directory = Directory::new();
    let state = annotated_page();
    let first = Store::open(&directory.0).unwrap();
    first.write(state.project_snapshot().unwrap()).unwrap();
    let mut invalid = state.project_snapshot().unwrap();
    invalid.images.clear();
    assert!(first.write(invalid).is_err());
    drop(first);
    let mut second = Store::open(&directory.0).unwrap();
    let candidate = second.candidate().unwrap();
    let restored = second.read(candidate.id).unwrap();
    assert_eq!(restored.project.id, state.project.header.id);
    second.adopt(candidate.id).unwrap();
    assert!(second.candidate().is_none());
    drop(second); // Recovery itself must not delete the only good copy.
    let third = Store::open(&directory.0).unwrap();
    assert!(third.candidate().is_some());
}

#[test]
fn discard_and_clear_remove_only_owned_snapshots() {
    let directory = Directory::new();
    let first = Store::open(&directory.0).unwrap();
    first
        .write(annotated_page().project_snapshot().unwrap())
        .unwrap();
    drop(first);
    let mut second = Store::open(&directory.0).unwrap();
    let candidate = second.candidate().unwrap();
    assert!(second.discard(Uuid::new_v4()).is_err());
    assert!(second.read(candidate.id).is_ok());
    second.discard(candidate.id).unwrap();
    assert!(second.candidate().is_none());
    second
        .write(FastFill::default().project_snapshot().unwrap())
        .unwrap();
    second.clear().unwrap();
    second.clear().unwrap(); // Cleanup is idempotent.
    drop(second);
    let third = Store::open(&directory.0).unwrap();
    assert!(third.candidate().is_none());
}

#[test]
fn broken_archives_are_kept_until_explicitly_discarded() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let id = Uuid::new_v4();
    let path = directory.0.join(format!("{id}.docscan"));
    fs::write(&path, b"not a ZIP").unwrap();
    fs::write(directory.0.join("unrelated.docscan"), b"keep").unwrap();
    let mut store = Store::open(&directory.0).unwrap();
    assert!(store.read(id).is_err());
    assert!(path.exists());
    store.discard(id).unwrap();
    assert!(!path.exists());
    assert!(directory.0.join("unrelated.docscan").exists());
}

#[test]
fn newest_is_offered_first_and_unrestored_snapshots_are_preserved() {
    let directory = Directory::new();
    let older = Store::open(&directory.0).unwrap();
    let newer = Store::open(&directory.0).unwrap();
    older
        .write(annotated_page().project_snapshot().unwrap())
        .unwrap();
    newer
        .write(annotated_page().project_snapshot().unwrap())
        .unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&older.current.path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
        ))
        .unwrap();
    let newer_id = newer.current.candidate.id;
    let older_id = older.current.candidate.id;
    drop(older);
    drop(newer);
    let mut restoring = Store::open(&directory.0).unwrap();
    assert_eq!(restoring.candidate().unwrap().id, newer_id);
    restoring.adopt(newer_id).unwrap();
    let mut another = Store::open(&directory.0).unwrap();
    assert_eq!(another.candidate().unwrap().id, older_id);
    another.defer();
    let again = Store::open(&directory.0).unwrap();
    assert_eq!(again.candidate().unwrap().id, older_id);
}

#[cfg(unix)]
#[test]
fn foreign_symlinks_are_ignored_and_new_directory_is_private() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = Directory::new();
    let store = Store::open(&directory.0).unwrap();
    assert_eq!(
        fs::metadata(&directory.0).unwrap().permissions().mode() & 0o777,
        0o700,
    );
    let foreign = directory.0.join("foreign");
    fs::write(&foreign, b"keep").unwrap();
    symlink(
        &foreign,
        directory.0.join(format!("{}.docscan", Uuid::new_v4())),
    )
    .unwrap();
    let id = Uuid::new_v4();
    fs::write(directory.0.join(format!("{id}.docscan")), b"archive").unwrap();
    symlink(&foreign, directory.0.join(format!("{id}.lock"))).unwrap();
    drop(store);
    let store = Store::open(&directory.0).unwrap();
    assert!(store.candidate().is_none());
    assert_eq!(fs::read(foreign).unwrap(), b"keep");
}
