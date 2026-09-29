use std::path::Path;

/// Darwin kernel system dataless flag (SF_DATALESS = 0x40000000).
/// When set on a vnode (e.g. iCloud Drive dataless file, FileProvider placeholder),
/// reading its extents will trigger a blocking kernel/daemon fault to download contents.
pub const SF_DATALESS: u32 = 0x40000000;

/// Darwin user compressed flag (UF_COMPRESSED = 0x00000020).
/// This indicates transparent local filesystem compression (HFS+/APFS decmpfs)
/// and must NEVER be confused with cloud placeholders.
pub const UF_COMPRESSED: u32 = 0x00000020;

/// Pure flag classifier: returns true ONLY if SF_DATALESS is set,
/// and strictly returns false when only local flags like UF_COMPRESSED are present.
pub fn is_dataless_flags(flags: u32) -> bool {
    (flags & SF_DATALESS) != 0
}

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
                return is_dataless_flags(stat_buf.st_flags);
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
            return is_dataless_flags(stat_buf.st_flags);
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

    #[test]
    fn test_dataless_flag_classification_correctness() {
        // SF_DATALESS must be recognized as dataless cloud placeholder
        assert!(is_dataless_flags(SF_DATALESS));
        assert!(is_dataless_flags(SF_DATALESS | 0x01));

        // UF_COMPRESSED (0x20) alone must NOT be recognized as cloud placeholder
        assert!(!is_dataless_flags(UF_COMPRESSED));
        assert!(!is_dataless_flags(0));
        assert!(!is_dataless_flags(UF_COMPRESSED | 0x02)); // UF_NODUMP etc

        // Combination with SF_DATALESS correctly triggers dataless
        assert!(is_dataless_flags(SF_DATALESS | UF_COMPRESSED));
    }
}
