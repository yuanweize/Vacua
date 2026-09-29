# Feature Reality Matrix

> **Rule of Engineering Honesty**:  
> Statuses are strictly categorized based on verifiable runtime proof and code audit:
> - `RUNTIME_VERIFIED`: Real native OS kernel/runtime calls executed and observed on macOS hardware (e.g. APFS clonefile, native Trash FFI, FSEvents replay, Homebrew installation).
> - `VERIFIED`: Complete native implementation backed by automated unit, integration, and equivalence tests.
> - `PARTIAL`: Real foundational implementation active, but explicit boundaries/uncertainties exist (e.g. extent-level physical ownership vs conservative bounds).
> - `EXPERIMENTAL`: Working prototype undergoing metric calibration.
> - `IMPLEMENTED`: Code compiles and passes static gates, waiting for hardware/environment qualification.
> - `NOT_STARTED`: Formally planned but no implementation code committed.

| Feature / Subsystem | Status | Implementation | Test / Verification Proof | Limitation / Ground Truth |
| :--- | :--- | :--- | :--- | :--- |
| **System Architecture & Safety Specs** | `VERIFIED` | `ARCHITECTURE.md`, `SAFETY.md`, `ROADMAP.md` | Formal specifications | Phase 0 complete |
| **Safety Invariants & Hard Boundaries** | `VERIFIED` | `vacua-core::invariants` | 4 unit tests (`test_protected_paths`, etc.) | Statically blocks system roots, SIP, SSH keys, mail, active git metadata |
| **APFS Clone Metadata Detection** | `RUNTIME_VERIFIED` | `vacua-scan::entry`, `vacua-scan::scanner` | `test_real_apfs_clonefile_detection` | Kernel `getattrlist(FSOPT_ATTR_CMN_EXTENDED)` querying `ATTR_CMNEXT_CLONEID \| ATTR_CMNEXT_CLONE_REFCNT`; verified via Darwin clonefile FFI |
| **APFS Physical Reclaim Accounting** | `PARTIAL` | `vacua-core::allocation` | Unit tests on `confirmed_freeable_bytes`, `upper_bound` | Distinguishes exclusive vs clone-shared extents. Conservative: clones allocate confirmed lower bounds without claiming unprovable extent ownership |
| **Streaming & Bounded Concurrency Scanner** | `VERIFIED` | `vacua-scan::scanner` | `test_scanner_bounded_concurrent_matches_sequential` | Bounded `sync_channel(2048)` backpressure; deterministic sorting; calibrated benchmark: 302k files/s, 46.9 MB peak RSS on 100k nodes |
| **Declarative Rules Engine & Guards** | `VERIFIED` | `vacua-rules::engine`, `guard` | Integration pipeline test | Live process checks, guarded category evaluations |
| **Deterministic Risk Scoring** | `VERIFIED` | `vacua-risk::evaluator` | 3 unit tests (`xcode_safe`, `ssh_protected`, etc.) | Invariants strictly enforced; Protected/Review/Caution/Safe tiers |
| **Storage Pressure Model** | `RUNTIME_VERIFIED` | `vacua-core::pressure` | Live `statfs` verification + unit tests | Evaluates raw free, ratio, and threshold policies on mounted APFS volumes |
| **Native FSEvents Binding** | `RUNTIME_VERIFIED` | `vacua-index::fsevents` | `test_native_fsevents_replay_integration` | Native CoreServices `FSEventStreamCreate` + `FSEventStreamFlushSync` with persistent event ID cursors |
| **Incremental Index Reconciliation** | `VERIFIED` | `vacua-index::db` (`reconcile_subtrees`) | `test_fsevents_reconciliation_exact_equivalence` | Rigorously verified: DB state after deletion/rename reconciliation is 100% equivalent to fresh full-scan output |
| **Recursive Snapshot Subtree Engine & Diff** | `VERIFIED` | `vacua-index::db` (`build_recursive_subtrees`) | `test_recursive_subtree_aggregation`, `test_storage_snapshot_and_diff` | True bottom-up aggregation of physical allocated bytes and file counts across directory subtrees; diffs surface real directory growth |
| **Application Evidence Graph** | `VERIFIED` | `vacua-core::evidence_graph` | `test_multi_signal_graph_discovery`, `test_orphan_evaluation_with_uninstalled_bundle` | Discovers bundles, `/var/db/receipts`, LaunchAgents/Daemons, Containers, Preferences, Saved State, and active memory processes; multi-factor scoring |
| **Reclaim Cost Model 2.0** | `EXPERIMENTAL` | `vacua-core::cost` | Unit tests on `ReclaimCostInputs` | Models rebuild friction, git project activity, and network redownload bounds |
| **Native macOS Trash Execution** | `RUNTIME_VERIFIED` | `vacua-executor::backend` | `test_executor_trash_execution_and_journaling` | Production execution invokes `-[NSFileManager trashItemAtURL:resultingItemURL:error:]` via Objective-C runtime FFI |
| **Pre-Action Journal & Hash Chain** | `VERIFIED` | `vacua-executor::journal`, `executor.rs` | `test_journal_tamper_detection`, `vacua history verify` | SHA-256 hash-chained integrity verification; detects unsynchronized SQLite record modifications; fail-safe pre-action write |
| **Apple Foundation Models Provider** | `IMPLEMENTED` | `apple/VacuaIntelligence` | Compile proof (`FoundationModelsProof`) + CLI self-test | `@Generable` guided generation compiled; `SystemLanguageModel` typed calls implemented; honest fallback when host is ineligible |
| **Apple Neural Inference On-Device** | `IMPLEMENTED` | `apple/VacuaIntelligence` | Host telemetry: `availability = deviceNotEligible` | Not marked `RUNTIME_VERIFIED` until run on an Apple Intelligence-eligible Mac with models installed |
| **Grounded Storage AI (`vacua ask`)** | `VERIFIED` | `vacua-cli::handle_ask` | Live CLI + JSON provenance tests | Input grounded in `StorageReasoningContext`; AI outputs verified against candidate/snapshot IDs; hallucinated IDs dropped; zero execution authority |
| **Homebrew Tap Distribution** | `RUNTIME_VERIFIED` | `yuanweize/homebrew-tap` | `brew install yuanweize/tap/vacua` + `brew test` | Verified against actual GitHub Release artifact SHA256; single formula source in tap repo |
| **Staged Content Identity Engine (BLAKE3)** | `VERIFIED` | `vacua-content::staged`, `identity` | `test_staged_pipeline_discovers_identical_files`, `test_sample_hash_collision_rejection`, `benchmark-dedup.py` | 6-stage pipeline (Size -> Hardlink -> Clone -> Sample -> Full BLAKE3 -> Stage 6 Confirmation). 3-window sample hash never treated as duplicate proof. |
| **APFS Clone & Hardlink Physical Accounting** | `RUNTIME_VERIFIED` | `vacua-content::group`, `staged`, `vacua-scan` | `test_real_apfs_clonefile_duplicate_behavior`, `test_hardlink_collapse_and_plan_generation`, `benchmark_scenario_f` | Tested on live APFS with Darwin `clonefile(2)` and `os.link`. Hardlinks collapse to 0B reclaimable. Clones account for copy-on-write sharing (0 confirmed / exclusive estimate / allocated upper bound). |
| **Persistent SQLite Fingerprint Cache** | `VERIFIED` | `vacua-index::schema` (v3), `db`, `vacua-content::cache` | `test_migrations_v2_to_v3_preserves_data`, `test_cache_hit_and_mtime_invalidation` | Stores versioned BLAKE3 digests keyed by nanosecond stat identity. Cache invalidates upon mtime/ctime/size changes. Tested automatic SQLite schema migration. |
| **TOCTOU-Safe Hashing & Mutation Invalidation** | `VERIFIED` | `vacua-content::identity` (`compute_full_fingerprint_toctou`) | `test_toctou_verification_detects_mutation` | Dual `fstat` on exact open file descriptor before and after read. Files changed during streaming are marked `ChangedDuringRead` and discarded from cache. |
| **Cloud & Dataless Placeholder Protection** | `VERIFIED` | `vacua-content::cloud` | `test_regular_local_file_is_not_cloud_placeholder` | Checks Darwin kernel `SF_DATALESS \| UF_DATALESS` flags via `st_flags` on path and open fd. Skips cloud placeholders by default to prevent unwanted remote hydration. |
| **Duplicate Cleanup Plan Integration** | `VERIFIED` | `vacua-content::engine`, `vacua-cli` | `test_hardlink_collapse_and_plan_generation`, `vacua duplicates plan` | Generates immutable `CleanupPlan` with SHA-256 hash. Preserves existing risk engine tiers (Protected/Review/Safe). Zero automatic deletion. Requires user-confirmed keep target. |
| **Read-Only MCP Server** | `NOT_STARTED` | Planned for Phase 4B (`vacua-mcp`) | N/A | Specification in `docs/MCP.md`; `--json` supported |
| **SwiftUI Native macOS App** | `NOT_STARTED` | Planned for Phase 5 (`apps/macos`) | N/A | Native macOS desktop UI planned |

