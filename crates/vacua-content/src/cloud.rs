use std::path::Path;

pub const SF_DATALESS: u32 = 0x40000000;
pub const UF_DATALESS: u32 = 0x00000020;

/// Returns true if the file is a dataless / cloud-only placeholder
/// (e.g. iCloud Drive, OneDrive, FileProvider placeholder) which would
/// trigger an automatic network download/hydration if opened and read.
pub fn is_cloud_placeholder(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        if let Some(c_path) = path.to_str().and_then(|s| CString::new(s).ok()) {
            let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::lstat(c_path.as_ptr(), &mut stat_buf) } == 0 {
                return (stat_buf.st_flags & (SF_DATALESS | UF_DATALESS)) != 0;
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }

    false
}

/// Checks if an already open raw file descriptor is a dataless cloud placeholder.
pub fn is_cloud_placeholder_fd(fd: std::os::unix::io::RawFd) -> bool {
    #[cfg(target_os = "macos")]
    {
        let mut stat_buf: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(fd, &mut stat_buf) } == 0 {
            return (stat_buf.st_flags & (SF_DATALESS | UF_DATALESS)) != 0;
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = fd;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_regular_local_file_is_not_cloud_placeholder() {
        let file = NamedTempFile::new().unwrap();
        assert!(!is_cloud_placeholder(file.path()));
    }
}
