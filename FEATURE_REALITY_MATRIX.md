# Feature Reality Matrix

> **Rule of Engineering Honesty**:  
> Status may ONLY be marked as `VERIFIED` when automated test suites and compiler builds have successfully validated the implementation. No forward-looking claims or mock implementations may be marked as complete.

| Feature / Subsystem | Status | Implementation | Test | Limitation / Notes |
| :--- | :--- | :--- | :--- | :--- |
| **System Architecture & Safety Specs** | `VERIFIED` | `ARCHITECTURE.md`, `SAFETY.md`, `ROADMAP.md` | Doc reviews & specs | Phase 0 complete |
| **Safety Invariants & Hard Boundaries** | `VERIFIED` | `vacua-core::invariants` | 4 unit tests (`test_protected_paths`, etc.) | Statically blocks system, SIP, SSH, mail, git |
| **APFS Extent & Clone Accounting** | `VERIFIED` | `vacua-core::allocation`, `vacua-scan::entry` | `test_real_apfs_clonefile_detection`, `test_scanner_deduplicates_hardlinks` | Kernel `getattrlist(FSOPT_ATTR_CMN_EXTENDED)` querying `ATTR_CMNEXT_CLONEID | ATTR_CMNEXT_EXT_FLAGS | ATTR_CMNEXT_CLONE_REFCNT`; distinguishes shared clone space vs exclusive blocks; sparse & hardlink aware |
| **Streaming & Bounded Concurrency Scanner** | `VERIFIED` | `vacua-scan::scanner` | `test_scanner_bounded_concurrent_matches_sequential`, `test_scanner_does_not_follow_symlink_into_foreign_dir` | Parallel worker pool with bounded `sync_channel(2048)` backpressure; deterministic path sorting; calibrated benchmark: 302k files/s, 46.9 MB peak RSS on 100k nodes |
| **Declarative Rules Engine & Guards** | `VERIFIED` | `vacua-rules::engine`, `guard` | Integration test pipeline | Live process checks (`sysinfo`), TOML rules |
| **Deterministic Risk Scoring** | `VERIFIED` | `vacua-risk::evaluator` | 3 unit tests (`xcode_safe`, `ssh_protected`, etc.) | Invariants strictly enforced; Value separated |
| **Storage Pressure Model** | `VERIFIED` | `vacua-core::pressure` | 1 unit test + live `statfs` | Evaluates raw free, ratio, thresholds policy |
| **CLI Command Suite** | `VERIFIED` | `vacua-cli::main` | Live binary invocation + `--json` | Standalone binary with human & JSON outputs |
| **End-to-End Pipeline & Plan Hashing** | `VERIFIED` | `vacua-plan::plan`, `integration_pipeline.rs` | 1 integration test + live plan test | SHA-256 integrity hash + TOCTOU pre-check |
| **SQLite Metadata Index** | `VERIFIED` | `vacua-index::db`, `vacua-index::schema` | 4 unit tests (`test_migration_*`, `test_record_session_*`, `test_rebuild_*`) | `PRAGMA user_version` migrations, fail-closed version checking, session stats |
| **Native FSEvents Incremental Refresh** | `VERIFIED` | `vacua-index::fsevents`, `vacua-index::db` | `test_native_fsevents_replay_integration`, `test_dropped_events_triggers_fallback_full_rescan` | Native CoreServices `FSEventStreamCreate` + `FSEventStreamFlushSync`; persistent SQLite cursors (`watched_roots`); surgical dirty subtree rescan (`vacua index refresh`) |
| **Storage Snapshots & Diff Engine** | `VERIFIED` | `vacua-index::db` | `test_storage_snapshot_and_diff`, live CLI | Point-in-time snapshots (`vacua snapshot create/list`), differential comparisons between snapshots or against live filesystem (`vacua diff baseline current`) |
| **Application Evidence Graph** | `VERIFIED` | `vacua-core::evidence_graph` | `test_orphan_evaluation_with_uninstalled_bundle`, live CLI | Discovers bundles & residue across App Support, Containers, Group Containers, Caches, Saved State; deterministic multi-signal orphan confidence (`vacua apps / leftovers`) |
| **Reclaim Cost Model & Simulation** | `VERIFIED` | `vacua-core::cost` | Live CLI (`vacua plan --simulate`) | Models rebuild friction, network redownload cost, and active project state; what-if cleanup simulation without modifying filesystem |
| **Storage Intelligence Query Engine (`vacua ask`)** | `VERIFIED` | `vacua-cli::handle_ask` | Live CLI (`vacua ask "Why did my storage grow?"`) | Natural language query engine grounded in snapshot diff evidence, application residue graph, candidate categories, and Reclaim Cost |
| **Native macOS Trash Execution** | `VERIFIED` | `vacua-executor::backend` | `test_executor_trash_execution_and_journaling` | Production execution invokes `-[NSFileManager trashItemAtURL:resultingItemURL:error:]` via typed Objective-C runtime FFI; cross-volume aware without `copy + rm` |
| **Fail-Safe Pre-Action Journal & Hash Chaining** | `VERIFIED` | `vacua-executor::journal`, `executor.rs` | `test_journal_tamper_detection`, `vacua history verify` | Fail-safe: aborts before modification if intent record fails; cryptographic SHA-256 canonical hash chaining (`prev_hash`, `entry_hash`); verified via `vacua history verify` |
| **StructuredIntent Schema** | `VERIFIED` | `schemas/structured-intent-v1.json` | Validated against Swift model & JSON outputs | Strict version 1, zero deletion authority, policy-bounded |
| **Apple Foundation Models Provider** | `IMPLEMENTED` | `apple/VacuaIntelligence` | Self-test harness (`vacua-intelligence test`) + CLI | Real `SystemLanguageModel.default.availability` queried; real `LanguageModelSession.respond` path compiled; strictly honest provenance (`provider_used = "deterministic-fallback"` when `deviceNotEligible`); zero deletion authority |
| **Rust↔Swift IPC Boundary** | `VERIFIED` | `vacua-cli::find_intelligence_binary` | Integration CLI tests (`vacua intelligence status / parse`) | Stdin/stdout JSON v1 protocol (ADR-0005) with process isolation |
| **Bundled Distribution Packaging** | `VERIFIED` | `scripts/package-release.sh`, `vacua-cli::find_intelligence_binary` | Clean smoke test in `/tmp/vacua-release-test` | Bundles `vacua` + `vacua-intelligence` with sibling discovery & shell completions |
| **Brand Identity & Social Assets** | `VERIFIED` | `assets/brand/`, `assets/social/`, `assets/diagrams/`, `assets/demo/` | Visual inspections, SVG validations, PIL generation | SVG mark (dark/light), lockups, architecture diagram, 1280x640 social preview |
| **Deterministic Demo Workflow** | `VERIFIED` | `scripts/generate-demo-fixture.sh`, `scripts/update-demo.sh` | End-to-end execution on synthetic fixture | Zero hardcoded fabricated numbers; verifiable via `update-demo.sh` |
| **Staged Duplicate Detection (BLAKE3)** | `NOT_STARTED` | Planned for Phase 4 | N/A | Size grouping -> Inode collapse -> Partial hash -> Full BLAKE3 |
| **Read-Only MCP Server** | `NOT_STARTED` | Planned for Phase 4 (`vacua-mcp`) | N/A | Specification in `docs/MCP.md`; `--json` supported |
| **SwiftUI Native macOS App** | `NOT_STARTED` | Planned for Phase 5 (`apps/macos`) | N/A | Native macOS desktop UI planned |
