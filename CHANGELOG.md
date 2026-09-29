# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-09-29

Content Identity & Duplicate Intelligence Engine: Staged BLAKE3 pipeline, persistent SQLite fingerprint cache, APFS copy-on-write sharing awareness, TOCTOU-safe hashing, and verified duplicate cleanup planning.

### Added
- **Staged Duplicate Intelligence Engine (`vacua-content`)**:
  - 6-stage pipeline: Eligibility filter -> Size bucketing -> Hardlink inode collapse -> APFS clone metadata classification -> Domain-separated 3-window sample hashing (`VACUA_SAMPLE_V1`, 192 KiB) -> Bounded sequential streaming BLAKE3 -> Stage 6 Destructive Pair Confirmation.
  - Bypasses up to 100% of I/O on unique files and 62.5% on same-size adversarial files.
- **APFS Physical-Sharing Duplicate Accounting**:
  - Strict distinction between hardlinks (0 bytes reclaimable), APFS copy-on-write clones (0 bytes confirmed lower bound, exclusive bytes estimate), and independent copies (full allocated bytes reclaimable).
  - Deterministic suggested keep member selection based on protection tier, modification time, and canonical path.
- **Persistent SQLite Content Fingerprint Cache (`vacua-index`)**:
  - Schema migration v3 introducing `content_fingerprints` table.
  - Nanosecond filesystem identity resolution (`mtime_nsec`, `ctime_nsec`) with automatic invalidation upon mutation.
  - Tested migration from v2 to v3 preserving existing snapshots and index records.
- **TOCTOU Safety & Cloud Dataless Protection**:
  - Dual `fstat` on open file descriptors before and after hashing, invalidating cache on `ChangedDuringRead`.
  - Kernel `SF_DATALESS | UF_DATALESS` check skips iCloud/cloud placeholders by default to prevent unwanted remote hydration.
- **CLI Duplicate Discovery & Planning (`vacua-cli`)**:
  - `vacua duplicates <path>` with `--min-size`, `--jobs`, and `--json`.
  - `vacua duplicates show <group-id>` displaying content ID, physical relations, risks, and reclaim bounds.
  - `vacua duplicates plan <group-id> --keep <member>` compiling an immutable `CleanupPlan` with SHA-256 hash. Zero direct deletion authority.
  - `vacua duplicates cache [status|prune]` for cache observability.
- **Duplicate Benchmark Suite (`scripts/benchmark-dedup.py`)**:
  - Evaluates Scenarios A–F against synthetic APFS fixtures, verifying 100% I/O avoidance on mostly unique workloads and 100% cache hits on warm runs.

## [0.3.1] - 2026-09-29

Vacua Reality Hardening release: Grounded Storage AI, recursive subtree accounting, FSEvents reconciliation, multi-signal evidence graph, and verified Homebrew tap automation.

### Added
- **Apple Foundation Models Real Guided Generation (`vacua-intelligence`)**:
  - `@Generable struct GeneratedCleanupIntent` and `@Generable struct GeneratedStorageExplanation` with schema validation.
  - Native `session.respond(to:generating:)` on macOS 26.0+ SDK with compile-time release gate and compile proof.
  - Runtime provenance explicitly distinguishing `apple-guided-generation` from `deterministic-parser`.
- **Grounded Storage Reasoning (`vacua ask`)**:
  - `StorageReasoningContext` feeding candidate facts and snapshot diffs into Foundation Models.
  - Strict Rust reference validator enforcing that models can only cite real candidates and snapshots.
  - Zero deletion authority hard-enforced.
- **Multi-Signal Application Evidence Graph (`vacua-core`)**:
  - Discovery of `/var/db/receipts` (`NodeKind::PackageReceipt`), LaunchAgents/Daemons, Preferences (`NodeKind::Preferences`), Saved State, and active memory processes.
- **Recursive Snapshot Subtree Engine (`vacua-index`)**:
  - Exact bottom-up recursive aggregation of physical directory subtrees ($O(N \log N)$), replacing shallow directory inode block size bug.
- **FSEvents Incremental Index Reconciliation (`vacua-index`)**:
  - Prunes deleted and renamed rows from SQLite index during dirty subtree refresh. Verified identical to fresh full scans.

### Fixed
- Synced `vacua-intelligence` binary version output with CLI suite.
- Reconciled benchmark documentation and peak RSS metrics (`/usr/bin/time -l`).
- Restored Homebrew tap single source of truth with automated post-release SHA256 propagation.

## [0.2.0] - 2026-09-29

Reality-First Storage Intelligence Engine release for macOS.

### Added
- **APFS Clone-Aware Allocation Accounting (`vacua-core`, `vacua-scan`)**:
  - Direct Darwin kernel `getattrlist(FSOPT_ATTR_CMN_EXTENDED)` querying `ATTR_CMNEXT_CLONEID | ATTR_CMNEXT_EXT_FLAGS | ATTR_CMNEXT_CLONE_REFCNT`.
  - Distinguishes physical shared clone space from exclusive blocks to prevent copy-on-write files from inflating reclaimable space estimates.
  - Integration-verified against real `clonefile(2)` system calls.
- **Bounded Concurrency Scanner (`vacua-scan`)**:
  - Streaming parallel worker pool with bounded `sync_channel(2048)` backpressure and deterministic path sorting.
  - Added `-j/--jobs` support with automatic physical core upper bounds.
  - Calibrated benchmark: 302,239 files/sec at 46.9 MB peak RSS on 100,000 files.
- **Native macOS FSEvents Incremental Refresh (`vacua-index`)**:
  - Native CoreServices `FSEventStreamCreate` + `FSEventStreamFlushSync` integration.
  - Persistent SQLite cursors (`watched_roots`) tracking volume identity and last event IDs.
  - Surgical rescan of dirty subtrees via `vacua index refresh` with fail-safe dropped event fallback.
- **Storage Snapshots & Diff Engine (`vacua-index`, `vacua-cli`)**:
  - Point-in-time storage state snapshots (`vacua snapshot create`, `vacua snapshot list`).
  - Differential comparison of allocation deltas across snapshots or against live filesystem (`vacua diff <base> [target]`).
- **Application Evidence Graph & Orphan Analysis (`vacua-core`, `vacua-cli`)**:
  - Multi-signal graph discovering application bundles and filesystem residue across Application Support, Containers, Group Containers, Caches, and Saved State.
  - Deterministic orphan confidence ratings (`vacua apps`, `vacua leftovers`).
- **Reclaim Cost Model & What-If Simulator (`vacua-core`, `vacua-cli`)**:
  - Deterministically assesses rebuild friction and network redownload cost (`ReclaimCost`).
  - What-if cleanup simulator (`vacua plan --simulate`) predicting freeable ranges without modifying disk.
- **Storage Intelligence Query Engine (`vacua ask`)**:
  - Natural language storage query engine grounded strictly in snapshot diffs, candidate evidence, and Reclaim Cost.
- **Executor Hardening & Native macOS Trash (`vacua-executor`)**:
  - Switched production trash execution to native `-[NSFileManager trashItemAtURL:resultingItemURL:error:]` via typed Objective-C runtime FFI.
  - Multi-volume aware without `copy + rm` workarounds.
  - Fail-safe pre-action intent recording: aborts immediately before disk modification if journal write fails.
- **Cryptographic Tamper-Evident Hash Chaining (`vacua-executor`, `vacua-cli`)**:
  - Every execution journal record linked to previous entries via SHA-256 canonical hash chaining.
  - Added `vacua history verify` command to audit journal integrity.
- **Real Apple Foundation Models Session Integration (`apple/VacuaIntelligence`)**:
  - Dynamic `SystemLanguageModel.default.availability` probe.
  - Real `LanguageModelSession.respond` session integration with macOS 26+ `@Generable` guard.
  - Strictly honest provenance tracking (`provider_used = "deterministic-fallback"` when device is not eligible).

---

## [0.1.0] - 2026-09-28

Initial open-source release of **Vacua**, an explainable, safety-first storage intelligence CLI for macOS.

### Added
- **Core Filesystem Scanner (`vacua-scan`)**:
  - Parallel bounded concurrency directory traversal.
  - Low-level APFS allocation accounting distinguishing logical bytes from physical disk blocks (`st_blocks * 512`).
  - Inode-based hardlink deduplication and strict directory symlink isolation.
- **Declarative Rule Engine & Risk Evaluation (`vacua-rules`, `vacua-risk`)**:
  - Declarative TOML-based cleanup rules with regex/glob matching.
  - Active process guards (e.g. aborts if `xcodebuild`, `cargo`, or `brew` is running).
  - Deterministic multi-factor risk categorization (`SAFE`, `REVIEW`, `CAUTION`, `PROTECTED`, `UNKNOWN`).
  - Compile-time and runtime invariant enforcement: `PROTECTED` and `UNKNOWN` files are fail-closed and cannot be auto-cleaned.
- **Two-Phase Immutable Planner (`vacua-plan`)**:
  - Cryptographic plan generation with SHA-256 integrity hash.
  - Itemized physical reclaim estimates and risk breakdowns.
- **Phase 2B Safe Executor & Audit Journal (`vacua-executor`)**:
  - Plan integrity verification against cryptographic hash.
  - Pre-execution TOCTOU verification (re-reads device ID, inode, and mtime; skips on mismatch).
  - Reversible execution by default via native macOS Trash (`~/.Trash`).
  - SQLite transaction audit journal (`~/.vacua/journal.db`) tracking every executed, skipped, or failed item.
- **Incremental Index & FSEvents Foundation (`vacua-index`)**:
  - SQLite index storage (`~/.vacua/index.db`) with schema migrations and session tracking.
  - FSEvents dirty-subtree tracking and coalescing.
- **Optional Native Apple Intelligence Bridge (`apple/VacuaIntelligence`)**:
  - Swift helper binary with process-isolated JSON v1 IPC over standard I/O.
  - Probes `SystemLanguageModel` availability on macOS.
  - Transparent telemetry reporting (`provider_requested`, `provider_used`, `apple_model_availability`).
  - Offline deterministic natural language parsing fallback when on-device model is not ready.
  - Zero deletion authority: AI translates natural language queries into `StructuredIntent`, never directly touching the filesystem or executor.
- **CLI Commands (`vacua-cli`)**:
  - `vacua scan <path>`: Physical vs logical storage scan.
  - `vacua candidates <path>`: Categorized cleanup opportunities with max risk filters.
  - `vacua explain <id>`: Human-readable evidence vector explaining classification reasons and rebuild effects.
  - `vacua plan <path> [-o <file>]`: Compiles and saves an immutable cleanup plan.
  - `vacua execute <plan> [--dry-run]`: Safe, TOCTOU-verified execution with interactive confirmation.
  - `vacua history [show <tx_id>]`: Transaction audit journal viewer.
  - `vacua doctor`: Storage health, APFS stats, FDA status, and system diagnostics.
  - `vacua completions <shell>`: Generates shell completion scripts (zsh, bash, fish).
  - `vacua intelligence status / parse`: Probes on-device model and inspects translated intent.
- **Packaging & Distribution**:
  - Standalone release bundle `vacua-v0.1.0-aarch64-apple-darwin.tar.gz` packaging `vacua` and `vacua-intelligence`.
  - Self-contained sibling binary discovery mechanism supporting Homebrew `bin/` and `libexec/`.
  - Deterministic demo harness (`scripts/generate-demo-fixture.sh`, `scripts/update-demo.sh`).
  - Visual identity and brand assets (`assets/brand/`, `assets/social/`, `assets/diagrams/`, `assets/demo/`).
