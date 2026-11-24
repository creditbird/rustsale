//! One RustSale core per data directory.
//!
//! Two app processes on the same database both run start-up recovery and share
//! one browser profile directory, so the second one used to interrupt the
//! first one's running scrape job (A-U2). The lock is an exclusive OS file
//! lock held for the life of the process: the OS drops it when the process
//! dies, so a crash never leaves a stale lock behind. It does not depend on
//! any UI shell, so the NSH host and the Tauri dev host get it alike.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

const LOCK_FILE_NAME: &str = "rustsale.instance.lock";

#[derive(Debug, thiserror::Error)]
pub enum InstanceLockError {
    #[error("another RustSale instance is already using {path}")]
    AlreadyRunning { path: PathBuf },
    #[error("cannot open the instance lock {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Holds the exclusive lock until it is dropped.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Takes the lock for `data_dir`, or fails at once when another instance
    /// holds it. The directory must already exist.
    pub fn acquire(data_dir: &Path) -> Result<Self, InstanceLockError> {
        let path = data_dir.join(LOCK_FILE_NAME);
        let io = |source| InstanceLockError::Io {
            path: path.clone(),
            source,
        };
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(io)?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { _file: file }),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
            {
                Err(InstanceLockError::AlreadyRunning {
                    path: data_dir.to_path_buf(),
                })
            }
            Err(error) => Err(io(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_instance_on_the_same_data_dir_is_refused_until_the_first_exits() {
        let dir = tempfile::tempdir().unwrap();
        let first = InstanceLock::acquire(dir.path()).unwrap();

        match InstanceLock::acquire(dir.path()) {
            Err(InstanceLockError::AlreadyRunning { path }) => assert_eq!(path, dir.path()),
            other => panic!("second instance must be refused, got {other:?}"),
        }

        drop(first);
        InstanceLock::acquire(dir.path()).expect("the lock is free once the first instance exits");
    }

    #[test]
    fn different_data_dirs_do_not_conflict() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let _first = InstanceLock::acquire(one.path()).unwrap();
        InstanceLock::acquire(two.path()).unwrap();
    }
}
