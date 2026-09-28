use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoragePressure {
    Normal,
    Elevated,
    Low,
    Critical,
}

impl std::fmt::Display for StoragePressure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoragePressure::Normal => write!(f, "NORMAL"),
            StoragePressure::Elevated => write!(f, "ELEVATED"),
            StoragePressure::Low => write!(f, "LOW"),
            StoragePressure::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Volume storage status and capacity metrics on macOS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeStorageStatus {
    pub mount_point: String,
    pub filesystem_type: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub free_ratio: f64,
    pub pressure: StoragePressure,
}

/// Threshold Policy for StoragePressure:
/// - CRITICAL: free_ratio < 0.05 (5%) OR free_bytes < 5 GiB. High risk of system freeze or APFS lock.
/// - LOW:      free_ratio < 0.10 (10%) OR free_bytes < 15 GiB. macOS begins aggressively purging caches.
/// - ELEVATED: free_ratio < 0.20 (20%) OR free_bytes < 30 GiB. Proactive cleanup advised.
/// - NORMAL:   free_ratio >= 0.20 AND free_bytes >= 30 GiB. Healthy volume state.
pub fn evaluate_storage_pressure(free_bytes: u64, total_bytes: u64) -> StoragePressure {
    if total_bytes == 0 {
        return StoragePressure::Critical;
    }

    let free_ratio = free_bytes as f64 / total_bytes as f64;
    let gib = 1024 * 1024 * 1024;

    if free_ratio < 0.05 || free_bytes < 5 * gib {
        StoragePressure::Critical
    } else if free_ratio < 0.10 || free_bytes < 15 * gib {
        StoragePressure::Low
    } else if free_ratio < 0.20 || free_bytes < 30 * gib {
        StoragePressure::Elevated
    } else {
        StoragePressure::Normal
    }
}

/// Query storage pressure for a volume path via macOS libc::statfs
pub fn query_volume_status(path: &Path) -> std::io::Result<VolumeStorageStatus> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statfs(c_path.as_ptr(), &mut stat) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let block_size = stat.f_bsize as u64;
    let total_bytes = stat.f_blocks.saturating_mul(block_size);
    let free_bytes = stat.f_bfree.saturating_mul(block_size);
    let available_bytes = stat.f_bavail.saturating_mul(block_size);

    let free_ratio = if total_bytes > 0 {
        available_bytes as f64 / total_bytes as f64
    } else {
        0.0
    };

    let pressure = evaluate_storage_pressure(available_bytes, total_bytes);

    let fs_type = unsafe {
        std::ffi::CStr::from_ptr(stat.f_fstypename.as_ptr())
            .to_string_lossy()
            .into_owned()
    };

    let mount_point = unsafe {
        std::ffi::CStr::from_ptr(stat.f_mntonname.as_ptr())
            .to_string_lossy()
            .into_owned()
    };

    Ok(VolumeStorageStatus {
        mount_point,
        filesystem_type: fs_type,
        total_bytes,
        free_bytes,
        available_bytes,
        free_ratio,
        pressure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pressure_evaluation_thresholds() {
        let gib = 1024 * 1024 * 1024;
        let total = 500 * gib;

        // Normal: 200 GiB free (40%)
        assert_eq!(
            evaluate_storage_pressure(200 * gib, total),
            StoragePressure::Normal
        );

        // Elevated: 75 GiB free (15%)
        assert_eq!(
            evaluate_storage_pressure(75 * gib, total),
            StoragePressure::Elevated
        );

        // Low: 35 GiB free (7%)
        assert_eq!(
            evaluate_storage_pressure(35 * gib, total),
            StoragePressure::Low
        );

        // Critical: 10 GiB free (2%)
        assert_eq!(
            evaluate_storage_pressure(10 * gib, total),
            StoragePressure::Critical
        );

        // Critical: low absolute bytes even if ratio seems okay on tiny disk
        assert_eq!(
            evaluate_storage_pressure(2 * gib, 20 * gib),
            StoragePressure::Critical
        );
    }
}
