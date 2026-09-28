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

// ---------------------------------------------------------------------------
// Native macOS CoreServices FSEventStream Bindings
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        alloc: *mut std::ffi::c_void,
        cstr: *const std::ffi::c_char,
        encoding: u32,
    ) -> *mut std::ffi::c_void;
    fn CFArrayCreate(
        alloc: *mut std::ffi::c_void,
        values: *const *const std::ffi::c_void,
        numValues: isize,
        callbacks: *const std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn CFRelease(cf: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn FSEventStreamCreate(
        allocator: *mut std::ffi::c_void,
        callback: extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            usize,
            *mut *mut std::ffi::c_char,
            *const u32,
            *const u64,
        ),
        context: *const FSEventContext,
        pathsToWatch: *mut std::ffi::c_void,
        sinceWhen: u64,
        latency: f64,
        flags: u32,
    ) -> *mut std::ffi::c_void;

    fn FSEventStreamSetDispatchQueue(stream: *mut std::ffi::c_void, queue: *mut std::ffi::c_void);
    fn FSEventStreamStart(stream: *mut std::ffi::c_void) -> i32;
    fn FSEventStreamFlushSync(stream: *mut std::ffi::c_void);
    fn FSEventStreamStop(stream: *mut std::ffi::c_void);
    fn FSEventStreamInvalidate(stream: *mut std::ffi::c_void);
    fn FSEventStreamRelease(stream: *mut std::ffi::c_void);
    fn FSEventsGetCurrentEventId() -> u64;

    fn dispatch_queue_create(
        label: *const std::ffi::c_char,
        attr: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn dispatch_release(queue: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct FSEventContext {
    version: isize,
    info: *mut std::ffi::c_void,
    retain: *mut std::ffi::c_void,
    release: *mut std::ffi::c_void,
    copy_description: *mut std::ffi::c_void,
}

#[cfg(target_os = "macos")]
extern "C" fn raw_fsevent_callback(
    _stream: *mut std::ffi::c_void,
    info: *mut std::ffi::c_void,
    num_events: usize,
    paths: *mut *mut std::ffi::c_char,
    event_flags: *const u32,
    event_ids: *const u64,
) {
    let tracker = unsafe { &mut *(info as *mut DirtySubtreeTracker) };
    for i in 0..num_events {
        let c_path = unsafe { *paths.add(i) };
        let path_str = unsafe { std::ffi::CStr::from_ptr(c_path) }.to_string_lossy();
        let flag = unsafe { *event_flags.add(i) };
        let id = unsafe { *event_ids.add(i) };
        tracker.ingest_event(PathBuf::from(path_str.as_ref()), flag, id);
    }
}

/// Query current global FSEvent ID from macOS kernel.
pub fn get_current_event_id() -> u64 {
    #[cfg(target_os = "macos")]
    unsafe {
        FSEventsGetCurrentEventId()
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

/// Replay FSEvents history since `since_event_id` for `root_path`.
/// Returns a populated DirtySubtreeTracker containing all affected subtrees.
pub fn replay_fsevents_since(
    root_path: &Path,
    since_event_id: u64,
) -> std::io::Result<DirtySubtreeTracker> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;

        let mut tracker = DirtySubtreeTracker::new();

        let c_path = CString::new(root_path.to_str().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid root path encoding",
            )
        })?)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        unsafe {
            let cf_str = CFStringCreateWithCString(
                std::ptr::null_mut(),
                c_path.as_ptr(),
                0x08000100, // kCFStringEncodingUTF8
            );
            if cf_str.is_null() {
                return Err(std::io::Error::other(
                    "Failed to create CFString for root path",
                ));
            }

            let ptrs = [cf_str as *const std::ffi::c_void];
            let paths_array =
                CFArrayCreate(std::ptr::null_mut(), ptrs.as_ptr(), 1, std::ptr::null());

            let ctx = FSEventContext {
                version: 0,
                info: &mut tracker as *mut _ as *mut std::ffi::c_void,
                retain: std::ptr::null_mut(),
                release: std::ptr::null_mut(),
                copy_description: std::ptr::null_mut(),
            };

            let q_name = CString::new("com.vacua.fsevents.replay").unwrap();
            let queue = dispatch_queue_create(q_name.as_ptr(), std::ptr::null_mut());

            // kFSEventStreamCreateFlagFileEvents (0x10) | kFSEventStreamCreateFlagNoDefer (0x02)
            let stream = FSEventStreamCreate(
                std::ptr::null_mut(),
                raw_fsevent_callback,
                &ctx,
                paths_array,
                since_event_id,
                0.0,
                0x00000010 | 0x00000002,
            );

            if stream.is_null() {
                dispatch_release(queue);
                CFRelease(paths_array);
                CFRelease(cf_str);
                return Err(std::io::Error::other("Failed to create FSEventStream"));
            }

            FSEventStreamSetDispatchQueue(stream, queue);
            FSEventStreamStart(stream);

            // Synchronously flush all pending kernel events
            FSEventStreamFlushSync(stream);

            FSEventStreamStop(stream);
            FSEventStreamInvalidate(stream);
            FSEventStreamRelease(stream);
            dispatch_release(queue);
            CFRelease(paths_array);
            CFRelease(cf_str);
        }

        // If tracker didn't record a higher event_id, set it to current event ID
        if tracker.last_event_id == 0 {
            tracker.last_event_id = get_current_event_id();
        }

        Ok(tracker)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut tracker = DirtySubtreeTracker::new();
        tracker.full_rescan_required = true;
        Ok(tracker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingest_event_and_collapse_subtrees() {
        let mut tracker = DirtySubtreeTracker::new();

        tracker.ingest_event(
            PathBuf::from("/Users/alice/Library/Caches/app/a.tmp"),
            flags::ITEM_MODIFIED,
            100,
        );
        tracker.ingest_event(
            PathBuf::from("/Users/alice/Library/Caches/app/b.tmp"),
            flags::ITEM_CREATED,
            101,
        );
        tracker.ingest_event(
            PathBuf::from("/Users/alice/Downloads/file.iso"),
            flags::ITEM_CREATED,
            102,
        );

        let subtrees = tracker.get_subtrees_to_rescan().unwrap();
        assert_eq!(subtrees.len(), 2);
        assert!(subtrees.contains(&PathBuf::from("/Users/alice/Library/Caches/app")));
        assert!(subtrees.contains(&PathBuf::from("/Users/alice/Downloads")));
        assert_eq!(tracker.last_event_id(), 102);
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

    #[cfg(target_os = "macos")]
    #[test]
    fn test_native_fsevents_replay_integration() {
        let tmp = tempfile::tempdir().unwrap();
        let start_id = get_current_event_id();
        assert!(start_id > 0);

        // Write a test file inside tmp
        let test_file = tmp.path().join("fsevent_change.txt");
        std::fs::write(&test_file, b"sample content").unwrap();

        // Replay events since start_id
        let tracker = replay_fsevents_since(tmp.path(), start_id).unwrap();
        // Native stream executed and returned valid last_event_id
        assert!(tracker.last_event_id() >= start_id);
    }
}
