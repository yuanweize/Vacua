use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Standard macOS FSEventStream event flags defined in <CoreServices/CoreServices.h>
pub mod flags {
    pub const MUST_SCAN_SUBDIRS: u32 = 0x0000_0001;
    pub const USER_DROPPED: u32 = 0x0000_0002;
    pub const KERNEL_DROPPED: u32 = 0x0000_0004;
    pub const EVENT_IDS_WRAPPED: u32 = 0x0000_0008;
    pub const HISTORY_DONE: u32 = 0x0000_0010;
    pub const ROOT_CHANGED: u32 = 0x0000_0020;
    pub const MOUNT: u32 = 0x0000_0040;
    pub const UNMOUNT: u32 = 0x0000_0080;
    pub const ITEM_CREATED: u32 = 0x0000_0100;
    pub const ITEM_REMOVED: u32 = 0x0000_0200;
    pub const ITEM_RENAMED: u32 = 0x0000_0800;
    pub const ITEM_MODIFIED: u32 = 0x0000_1000;
    pub const ITEM_IS_DIR: u32 = 0x0002_0000;
}

/// Tracks changed subtrees from macOS FSEvents to enable surgical rescan invalidation.
///
/// Safety Principle:
/// FSEvents is treated strictly as a performance hint to narrow down which directories
/// require rescanning. It is NEVER treated as a safety authority to delete or bypass
/// filesystem metadata verification.
#[derive(Debug, Clone, Default)]
pub struct DirtySubtreeTracker {
    dirty_subtrees: HashSet<PathBuf>,
    full_rescan_required: bool,
    last_event_id: u64,
}

impl DirtySubtreeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest a raw FSEvents notification event with its flags.
    pub fn ingest_event(&mut self, path: PathBuf, event_flags: u32, event_id: u64) {
        if event_id > self.last_event_id {
            self.last_event_id = event_id;
        }

        // Dropped events or wrapped IDs indicate that the kernel or user buffer overflowed
        // and events were lost. We must conservatively fall back to a full rescan.
        if (event_flags
            & (flags::USER_DROPPED
                | flags::KERNEL_DROPPED
                | flags::EVENT_IDS_WRAPPED
                | flags::ROOT_CHANGED))
            != 0
        {
            self.full_rescan_required = true;
            return;
        }

        // If MustScanSubDirs or directory event, mark path as dirty subtree
        if (event_flags & (flags::MUST_SCAN_SUBDIRS | flags::ITEM_IS_DIR)) != 0 {
            self.dirty_subtrees.insert(path);
        } else {
            // For individual file mutations, mark parent directory as dirty subtree
            if let Some(parent) = path.parent() {
                self.dirty_subtrees.insert(parent.to_path_buf());
            } else {
                self.dirty_subtrees.insert(path);
            }
        }
    }

    /// Mark an explicit directory path as dirty (e.g. upon error or user request).
    pub fn mark_dirty(&mut self, path: &Path) {
        self.dirty_subtrees.insert(path.to_path_buf());
    }

    /// Reset all tracked events after rescan is completed.
    pub fn clear(&mut self) {
        self.dirty_subtrees.clear();
        self.full_rescan_required = false;
    }

    pub fn last_event_id(&self) -> u64 {
        self.last_event_id
    }

    pub fn is_full_rescan_required(&self) -> bool {
        self.full_rescan_required
    }

    /// Returns the minimal, deduplicated list of ancestor subtrees that need targeted rescanning.
    /// Returns None if a full rescan of the entire tree is required due to dropped events.
    pub fn get_subtrees_to_rescan(&self) -> Option<Vec<PathBuf>> {
        if self.full_rescan_required {
            return None;
        }

        // Collapse child subtrees into parent subtrees
        let mut sorted: Vec<PathBuf> = self.dirty_subtrees.iter().cloned().collect();
        sorted.sort_by_key(|p| p.as_os_str().len());

        let mut minimal: Vec<PathBuf> = Vec::new();
        for candidate in sorted {
            let already_covered = minimal.iter().any(|parent| candidate.starts_with(parent));
            if !already_covered {
                minimal.push(candidate);
            }
        }

        Some(minimal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingest_event_and_collapse_subtrees() {
        let mut tracker = DirtySubtreeTracker::new();

        tracker.ingest_event(
            PathBuf::from("/Users/alice/Library/Caches/com.apple.dt.Xcode/ModuleCache/foo"),
            flags::ITEM_MODIFIED,
            100,
        );
        tracker.ingest_event(
            PathBuf::from("/Users/alice/Library/Caches/com.apple.dt.Xcode"),
            flags::MUST_SCAN_SUBDIRS,
            101,
        );

        let subtrees = tracker.get_subtrees_to_rescan().unwrap();
        // Since /Users/alice/Library/Caches/com.apple.dt.Xcode is parent of ModuleCache,
        // it must collapse to only the parent!
        assert_eq!(subtrees.len(), 1);
        assert_eq!(
            subtrees[0],
            PathBuf::from("/Users/alice/Library/Caches/com.apple.dt.Xcode")
        );
        assert_eq!(tracker.last_event_id(), 101);
    }

    #[test]
    fn test_dropped_events_triggers_fallback_full_rescan() {
        let mut tracker = DirtySubtreeTracker::new();

        tracker.ingest_event(
            PathBuf::from("/Users/alice/Downloads"),
            flags::KERNEL_DROPPED,
            200,
        );

        assert!(tracker.is_full_rescan_required());
        assert!(tracker.get_subtrees_to_rescan().is_none());
    }
}
