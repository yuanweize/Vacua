use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

/// Classified kind of filesystem entry, strictly separating regular files from
/// directories, symlinks, and special device nodes (FIFOs, Sockets, Devices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    Regular,
    Directory,
    Symlink,
    Fifo,
    Socket,
    CharacterDevice,
    BlockDevice,
    Other,
}

impl FileKind {
    pub fn is_regular(&self) -> bool {
        matches!(self, FileKind::Regular)
    }
}

/// Nanosecond-precise filesystem identity captured from an open file descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    pub device_id: u64,
    pub inode: u64,
    pub size: u64,
    pub mtime_sec: i64,
    pub mtime_nsec: i64,
    pub ctime_sec: i64,
    pub ctime_nsec: i64,
    pub file_kind: FileKind,
}

/// Safely opens a regular file for reading, guaranteeing that:
/// 1. Symlinks are NOT followed (O_NOFOLLOW).
/// 2. Special files (FIFOs, sockets) will not block (O_NONBLOCK).
/// 3. The opened file descriptor is verified via fstat to be S_IFREG before returning.
///
/// This primitive prevents TOCTOU symlink substitution races, FIFO deadlocks, and
/// special device node access.
pub fn open_regular_file_safely(path: &Path) -> std::io::Result<File> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::io::FromRawFd;

        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid path with interior null byte: {}", e),
            )
        })?;

        let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;
        let fd = unsafe { libc::open(c_path.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }

        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        let stat_res = unsafe { libc::fstat(fd, &mut st) };
        if stat_res != 0 {
            let err = std::io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err);
        }

        // Verify that the opened fd is strictly a regular file
        if (st.st_mode & libc::S_IFMT) != libc::S_IFREG {
            unsafe { libc::close(fd) };
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Path is not a regular file (mode 0o{:o}): {}",
                    st.st_mode,
                    path.display()
                ),
            ));
        }

        let file = unsafe { File::from_raw_fd(fd) };
        Ok(file)
    }

    #[cfg(not(unix))]
    {
        let file = std::fs::OpenOptions::new().read(true).open(path)?;
        let meta = file.metadata()?;
        if !meta.file_type().is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path is not a regular file: {}", path.display()),
            ));
        }
        Ok(file)
    }
}

/// Queries nanosecond stat identity directly from an open file descriptor.
pub fn query_file_identity(file: &File) -> std::io::Result<FileIdentity> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        let res = unsafe { libc::fstat(fd, &mut st) };
        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let file_kind = match st.st_mode & libc::S_IFMT {
            libc::S_IFREG => FileKind::Regular,
            libc::S_IFDIR => FileKind::Directory,
            libc::S_IFLNK => FileKind::Symlink,
            libc::S_IFIFO => FileKind::Fifo,
            libc::S_IFSOCK => FileKind::Socket,
            libc::S_IFCHR => FileKind::CharacterDevice,
            libc::S_IFBLK => FileKind::BlockDevice,
            _ => FileKind::Other,
        };

        let (mtime_sec, mtime_nsec, ctime_sec, ctime_nsec) =
            (st.st_mtime, st.st_mtime_nsec, st.st_ctime, st.st_ctime_nsec);

        Ok(FileIdentity {
            device_id: st.st_dev as u64,
            inode: st.st_ino as u64,
            size: st.st_size as u64,
            mtime_sec,
            mtime_nsec,
            ctime_sec,
            ctime_nsec,
            file_kind,
        })
    }

    #[cfg(not(unix))]
    {
        use std::time::UNIX_EPOCH;
        let meta = file.metadata()?;
        let size = meta.len();
        let mtime = meta.modified().unwrap_or(UNIX_EPOCH);
        let duration = mtime.duration_since(UNIX_EPOCH).unwrap_or_default();
        let mtime_sec = duration.as_secs() as i64;
        let mtime_nsec = duration.subsec_nanos() as i64;

        Ok(FileIdentity {
            device_id: 0,
            inode: 0,
            size,
            mtime_sec,
            mtime_nsec,
            ctime_sec: mtime_sec,
            ctime_nsec: mtime_nsec,
            file_kind: if meta.is_file() {
                FileKind::Regular
            } else if meta.is_dir() {
                FileKind::Directory
            } else {
                FileKind::Other
            },
        })
    }
}

/// Classifies a filesystem `std::fs::FileType` into a `FileKind`.
pub fn classify_file_type(ft: &std::fs::FileType) -> FileKind {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if ft.is_file() {
            FileKind::Regular
        } else if ft.is_dir() {
            FileKind::Directory
        } else if ft.is_symlink() {
            FileKind::Symlink
        } else if ft.is_fifo() {
            FileKind::Fifo
        } else if ft.is_socket() {
            FileKind::Socket
        } else if ft.is_char_device() {
            FileKind::CharacterDevice
        } else if ft.is_block_device() {
            FileKind::BlockDevice
        } else {
            FileKind::Other
        }
    }

    #[cfg(not(unix))]
    {
        if ft.is_file() {
            FileKind::Regular
        } else if ft.is_dir() {
            FileKind::Directory
        } else if ft.is_symlink() {
            FileKind::Symlink
        } else {
            FileKind::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_open_regular_file_safely_succeeds() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("regular.txt");
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(b"safe regular content").unwrap();
        }

        let file = open_regular_file_safely(&path).expect("regular file must open safely");
        let id = query_file_identity(&file).expect("query identity succeeds");
        assert_eq!(id.file_kind, FileKind::Regular);
        assert_eq!(id.size, 20);
        assert!(id.mtime_sec > 0);
    }

    #[test]
    fn test_open_regular_file_safely_rejects_directory() {
        let dir = tempdir().unwrap();
        let res = open_regular_file_safely(dir.path());
        assert!(res.is_err(), "Opening directory as regular file must fail");
    }

    #[cfg(unix)]
    #[test]
    fn test_open_regular_file_safely_rejects_symlink() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");
        {
            let mut f = File::create(&target).unwrap();
            f.write_all(b"target").unwrap();
        }
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let res = open_regular_file_safely(&link);
        assert!(res.is_err(), "Opening symlink with O_NOFOLLOW must fail");
    }

    #[cfg(unix)]
    #[test]
    fn test_open_regular_file_safely_rejects_fifo_without_blocking() {
        use std::ffi::CString;
        let dir = tempdir().unwrap();
        let fifo_path = dir.path().join("test_fifo.pipe");
        let c_path = CString::new(fifo_path.to_str().unwrap()).unwrap();
        unsafe {
            let ret = libc::mkfifo(c_path.as_ptr(), 0o644);
            if ret != 0 {
                // If mkfifo fails due to sandbox or filesystem, skip
                return;
            }
        }

        let res = open_regular_file_safely(&fifo_path);
        assert!(
            res.is_err(),
            "Opening FIFO must fail immediately without blocking"
        );
    }
}
