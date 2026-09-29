use tempfile::tempdir;
use vacua_content::{
    run_staged_duplicate_pipeline, DuplicateEngine, DuplicateScanOptions, FingerprintCache,
};
use vacua_core::RiskLevel;
use vacua_index::IndexDatabase;
use vacua_scan::ScannedEntry;

#[test]
fn test_cache_hit_and_mtime_invalidation() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("cached_test.bin");
    let content = vec![0x42; 250 * 1024]; // 250 KiB
    std::fs::write(&file_path, &content).unwrap();

    let db = IndexDatabase::open_in_memory().unwrap();
    let cache = FingerprintCache::new(&db);

    let entry = ScannedEntry::from_path(file_path.clone()).unwrap();
    let options = DuplicateScanOptions {
        min_size: 100 * 1024,
        include_empty: false,
        hash_jobs: 1,
        skip_cloud: true,
        use_cache: true,
    };

    // First scan: cold cache (miss)
    let (groups1, stats1) =
        run_staged_duplicate_pipeline(std::slice::from_ref(&entry), &options, Some(&cache), |_| {
            (RiskLevel::Review, "test".to_string())
        })
        .unwrap();
    assert_eq!(groups1.len(), 0); // single file, no duplicates
    assert_eq!(stats1.full_hash_cache_hits, 0);

    // Now populate full hash into cache manually to test cache hit
    let (full_digest, _, _) = vacua_content::compute_full_fingerprint_toctou(
        &entry.path,
        entry.device_id,
        entry.inode,
        entry.logical_bytes,
        entry.mtime_sec,
        entry.mtime_nsec,
        entry.ctime_sec,
        entry.ctime_nsec,
    )
    .unwrap();

    cache
        .put_full(
            entry.device_id,
            entry.inode,
            &entry.path,
            entry.logical_bytes,
            entry.mtime_sec,
            entry.mtime_nsec,
            entry.ctime_sec,
            entry.ctime_nsec,
            None,
            &full_digest,
        )
        .unwrap();

    // Query cache directly with exact same stat identity -> Hit
    let cached = cache
        .get(
            entry.device_id,
            entry.inode,
            &entry.path,
            entry.logical_bytes,
            entry.mtime_sec,
            entry.mtime_nsec,
            entry.ctime_sec,
            entry.ctime_nsec,
        )
        .unwrap();
    assert!(cached.is_some());
    assert_eq!(
        cached.unwrap().full_hash.as_deref(),
        Some(full_digest.as_str())
    );

    // Query cache with modified mtime -> Cache Miss (invalidated!)
    let invalid_mtime = cache
        .get(
            entry.device_id,
            entry.inode,
            &entry.path,
            entry.logical_bytes,
            entry.mtime_sec + 10,
            entry.mtime_nsec,
            entry.ctime_sec,
            entry.ctime_nsec,
        )
        .unwrap();
    assert!(invalid_mtime.is_none());
}

#[test]
fn test_sample_hash_collision_rejection() {
    // Two files having identical first 64KB, but differing in the middle window!
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("collision1.bin");
    let f2 = dir.path().join("collision2.bin");

    let size = 500 * 1024; // 500 KiB
    let data1 = vec![0x11; size];
    let mut data2 = vec![0x11; size];

    // Alter middle window in data2
    let mid_start = size / 2;
    for b in data2[mid_start..mid_start + 1024].iter_mut() {
        *b = 0x99;
    }

    std::fs::write(&f1, &data1).unwrap();
    std::fs::write(&f2, &data2).unwrap();

    let entries = vec![
        ScannedEntry::from_path(f1).unwrap(),
        ScannedEntry::from_path(f2).unwrap(),
    ];

    let options = DuplicateScanOptions {
        min_size: 100 * 1024,
        include_empty: false,
        hash_jobs: 1,
        skip_cloud: true,
        use_cache: false,
    };

    let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
        (RiskLevel::Review, "test".to_string())
    })
    .unwrap();

    // Size collision files seen = 2, but sample hash differentiates them!
    assert_eq!(stats.size_collision_files, 2);
    assert_eq!(groups.len(), 0); // Not grouped as duplicates!
}

#[test]
fn test_hardlink_collapse_and_plan_generation() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.bin");
    let hardlink = dir.path().join("hardlink.bin");

    let data = vec![0x77; 300 * 1024];
    std::fs::write(&original, &data).unwrap();
    std::fs::hard_link(&original, &hardlink).unwrap();

    let entries = vec![
        ScannedEntry::from_path(original.clone()).unwrap(),
        ScannedEntry::from_path(hardlink.clone()).unwrap(),
    ];

    let options = DuplicateScanOptions {
        min_size: 100 * 1024,
        include_empty: false,
        hash_jobs: 1,
        skip_cloud: true,
        use_cache: false,
    };

    let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
        (RiskLevel::Review, "test".to_string())
    })
    .unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(
        group.physical_sharing_state,
        vacua_content::PhysicalRelation::HardlinkAliasInGroup
    );
    assert_eq!(group.confirmed_reclaimable_bytes, 0);
    assert_eq!(stats.hardlinks_collapsed, 1);

    // Build cleanup plan keeping 'original'
    let plan = DuplicateEngine::build_cleanup_plan(group, &original).unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].path, hardlink);
    assert_eq!(plan.preservation_guards.len(), 1);
    assert_eq!(plan.preservation_guards[0].path, original);
    assert!(plan.items[0].content_guard.is_some());
    assert!(plan.verify_integrity().is_ok());
}

#[test]
fn test_external_hardlink_sharing_classification() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.bin");
    let external_link = dir.path().join("external_link.bin");
    let duplicate = dir.path().join("duplicate.bin");

    let data = vec![0x88; 200 * 1024];
    std::fs::write(&original, &data).unwrap();
    std::fs::hard_link(&original, &external_link).unwrap();
    std::fs::write(&duplicate, &data).unwrap();

    // Scan includes 'original' and 'duplicate', but omits 'external_link'
    let entries = vec![
        ScannedEntry::from_path(original.clone()).unwrap(),
        ScannedEntry::from_path(duplicate.clone()).unwrap(),
    ];

    let options = DuplicateScanOptions {
        min_size: 100 * 1024,
        include_empty: false,
        hash_jobs: 1,
        skip_cloud: true,
        use_cache: false,
    };

    let (groups, _) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
        (RiskLevel::Review, "test".to_string())
    })
    .unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    let orig_member = group.find_member(&original).unwrap();
    // Invariant: original must NOT be marked Independent because it has an external hardlink!
    assert_eq!(
        orig_member.physical_relation,
        vacua_content::PhysicalRelation::HardlinkSharedExternal
    );
}

#[test]
fn test_special_files_fifo_symlink_skipped_without_blocking() {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("reg1.bin");
    let f2 = dir.path().join("reg2.bin");
    let symlink = dir.path().join("symlink.bin");

    let data = vec![0x33; 150 * 1024];
    std::fs::write(&f1, &data).unwrap();
    std::fs::write(&f2, &data).unwrap();
    std::os::unix::fs::symlink(&f1, &symlink).unwrap();

    let fifo_path = dir.path().join("test_fifo.pipe");
    let c_fifo = std::ffi::CString::new(fifo_path.to_str().unwrap()).unwrap();
    unsafe {
        libc::mkfifo(c_fifo.as_ptr(), 0o644);
    }

    let entries = vec![
        ScannedEntry::from_path(f1).unwrap(),
        ScannedEntry::from_path(f2).unwrap(),
        ScannedEntry::from_path(symlink).unwrap(),
        ScannedEntry::from_path(fifo_path).unwrap(),
    ];

    let options = DuplicateScanOptions {
        min_size: 100 * 1024,
        include_empty: false,
        hash_jobs: 2,
        skip_cloud: true,
        use_cache: false,
    };

    let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
        (RiskLevel::Review, "test".to_string())
    })
    .unwrap();

    assert_eq!(groups.len(), 1);
    assert!(stats.special_files_skipped >= 2);
}

#[cfg(target_os = "macos")]
#[test]
fn test_real_apfs_clonefile_duplicate_behavior() {
    use std::ffi::CString;

    extern "C" {
        fn clonefile(src: *const std::ffi::c_char, dst: *const std::ffi::c_char, flags: u32)
            -> i32;
    }

    let dir = tempdir().unwrap();
    let src = dir.path().join("clone_src.bin");
    let dst = dir.path().join("clone_dst.bin");

    let payload = vec![0xFE; 256 * 1024];
    std::fs::write(&src, &payload).unwrap();

    let c_src = CString::new(src.to_str().unwrap()).unwrap();
    let c_dst = CString::new(dst.to_str().unwrap()).unwrap();

    let res = unsafe { clonefile(c_src.as_ptr(), c_dst.as_ptr(), 0) };
    if res != 0 {
        eprintln!("clonefile not supported on this volume (skipped APFS test)");
        return;
    }

    let entries = vec![
        ScannedEntry::from_path(src.clone()).unwrap(),
        ScannedEntry::from_path(dst.clone()).unwrap(),
    ];

    let options = DuplicateScanOptions {
        min_size: 100 * 1024,
        include_empty: false,
        hash_jobs: 1,
        skip_cloud: true,
        use_cache: false,
    };

    let (groups, stats) = run_staged_duplicate_pipeline(&entries, &options, None, |_| {
        (RiskLevel::Review, "test".to_string())
    })
    .unwrap();

    assert_eq!(groups.len(), 1);
    let group = &groups[0];

    // If on APFS, clone_id is detected and physical relation is APFSCloneInGroup
    if entries[0].is_clone && entries[1].is_clone {
        assert_eq!(
            group.physical_sharing_state,
            vacua_content::PhysicalRelation::APFSCloneInGroup
        );
        assert_eq!(group.confirmed_reclaimable_bytes, 0);
        assert_eq!(group.upper_bound_reclaimable_bytes, 256 * 1024);
        assert!(stats.clone_family_members >= 2);
    }
}
