use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("I/O error during trash operation: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to resolve user trash directory")]
    TrashDirectoryUnavailable,

    #[error("Target does not exist: {0}")]
    TargetNotFound(PathBuf),
}

/// Abstraction for moving items to trash or an isolated testing sink.
pub trait TrashBackend: Send + Sync {
    /// Moves target path to trash and returns the resulting destination path.
    fn trash(&self, target: &Path) -> Result<PathBuf, BackendError>;

    /// Returns a human-readable name of the backend.
    fn name(&self) -> &'static str;
}

/// Default macOS Trash backend. Moves items into the user's ~/.Trash folder.
pub struct MacOSTrashBackend {
    trash_dir: PathBuf,
}

impl Default for MacOSTrashBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOSTrashBackend {
    pub fn new() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/Users/unknown"));
        let trash_dir = home.join(".Trash");
        Self { trash_dir }
    }
}

impl TrashBackend for MacOSTrashBackend {
    fn trash(&self, target: &Path) -> Result<PathBuf, BackendError> {
        if !target.exists() {
            return Err(BackendError::TargetNotFound(target.to_path_buf()));
        }

        if !self.trash_dir.exists() {
            fs::create_dir_all(&self.trash_dir)?;
        }

        let file_name = target.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid target filename")
        })?;

        let mut dest = self.trash_dir.join(file_name);
        if dest.exists() {
            let timestamp = chrono::Utc::now().timestamp_millis();
            let unique_name = format!("{}.{}", file_name.to_string_lossy(), timestamp);
            dest = self.trash_dir.join(unique_name);
        }

        // On macOS, try atomic rename first. If cross-volume (EXDEV), fall back to copy+remove.
        match fs::rename(target, &dest) {
            Ok(_) => Ok(dest),
            Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
                if target.is_dir() {
                    copy_dir_recursive(target, &dest)?;
                    fs::remove_dir_all(target)?;
                } else {
                    fs::copy(target, &dest)?;
                    fs::remove_file(target)?;
                }
                Ok(dest)
            }
            Err(e) => Err(BackendError::Io(e)),
        }
    }

    fn name(&self) -> &'static str {
        "macOS Trash (~/.Trash)"
    }
}

/// In-sandbox temporary trash backend for automated tests and dry environments.
pub struct TempTrashBackend {
    trash_dir: PathBuf,
}

impl TempTrashBackend {
    pub fn new(trash_dir: PathBuf) -> Self {
        Self { trash_dir }
    }
}

impl TrashBackend for TempTrashBackend {
    fn trash(&self, target: &Path) -> Result<PathBuf, BackendError> {
        if !target.exists() {
            return Err(BackendError::TargetNotFound(target.to_path_buf()));
        }

        if !self.trash_dir.exists() {
            fs::create_dir_all(&self.trash_dir)?;
        }

        let file_name = target.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid target filename")
        })?;

        let mut dest = self.trash_dir.join(file_name);
        if dest.exists() {
            let timestamp = chrono::Utc::now().timestamp_millis();
            let unique_name = format!("{}.{}", file_name.to_string_lossy(), timestamp);
            dest = self.trash_dir.join(unique_name);
        }

        match fs::rename(target, &dest) {
            Ok(_) => Ok(dest),
            Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
                if target.is_dir() {
                    copy_dir_recursive(target, &dest)?;
                    fs::remove_dir_all(target)?;
                } else {
                    fs::copy(target, &dest)?;
                    fs::remove_file(target)?;
                }
                Ok(dest)
            }
            Err(e) => Err(BackendError::Io(e)),
        }
    }

    fn name(&self) -> &'static str {
        "Temporary Test Trash"
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if entry_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), &dest_child)?;
        }
    }
    Ok(())
}
