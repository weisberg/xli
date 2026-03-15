use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;

use xli_core::XliError;

/// Exclusive lock held on a workbook file until dropped.
#[derive(Debug)]
pub struct WorkbookLock {
    file: File,
}

impl WorkbookLock {
    /// Acquire an exclusive lock on an existing workbook file.
    pub fn acquire(path: &Path) -> Result<Self, XliError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => XliError::FileNotFound {
                    path: path.display().to_string(),
                },
                _ => XliError::LockConflict {
                    path: path.display().to_string(),
                },
            })?;

        file.lock_exclusive().map_err(|_| XliError::LockConflict {
            path: path.display().to_string(),
        })?;

        Ok(Self { file })
    }

    /// Access the underlying locked file handle.
    pub fn file(&self) -> &File {
        &self.file
    }
}

