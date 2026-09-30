# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-09-30

Phase 5: Native SwiftUI macOS App Foundation & Storage Intelligence UI: First-party native macOS desktop client over the verified Rust storage truth engine, bundled helper architecture over stdio JSON-RPC 2.0 (MCP), strictly proposal-only safety model, APFS clone awareness, and cross-language contract fixtures.

### Added
- **First-Party Native macOS Desktop App (`apps/macos/Vacua`)**:
  - Pure native SwiftUI + AppKit macOS application (`macOS 15+`, Swift 6 with Strict Concurrency).
  - Modern `NavigationSplitView` architecture with Sidebar navigation: Overview, Candidates, Duplicates, Applications, Snapshots, and Settings.
  - Storage Pressure Overview: APFS physical allocation breakdown (Used, Free, Purgeable), Swift Charts storage visualization, and guaranteed physical reclaim lower bounds vs estimated upper bounds.
  - Cleanup Candidates Table & Inspector: Filterable by risk level (`Safe`, `Caution`, `Review`, `Protected`) and category; deep inspector displaying full multi-signal evidence graphs, rebuild consequences, and reclaim bounds.
  - Duplicates Intelligence with Physical APFS Truth: Explicit user-initiated BLAKE3 analysis with disk read warning; clone awareness displaying shared vs independent extents; explicitly marks unmeasured APFS private storage as "Unknown".
  - Application Residue Explorer: Discloses installed applications and potential residual data with server-side query filtering and lazy graph building.
  - Storage Snapshot Browser & Real Diff UI: Enumerates point-in-time storage metadata observations and executes real differential analysis via `vacua_diff_snapshots`, surfacing recursive subtree deltas and horizontal delta charts without client size recalculation.
  - Transparent Settings & Security Diagnostics: Configurable allowed root directories via `NSOpenPanel`; root switching automatically cancels in-flight tasks and invalidates state; displays engine connection state and honestly explains macOS Full Disk Access requirements.
- **Lazy Runtime Architecture & State Safety**:
  - Cheap startup: Engine handshake only queries capabilities and volume summary; zero duplicate hashing or candidate scanning at launch.
  - Per-feature `LoadState` (`.idle`, `.loading`, `.loaded`, `.failed`) preserving previous successful data during background refreshes.
  - Cursor-aware pagination ("Load More…") for Candidates, Duplicates, Applications, and Snapshots.
  - Selection and in-flight request cancellation preventing race conditions and stale detail presentation.
  - OSLog privacy: Private interpolation for all paths and error details; production child process strictly uses `--path-disclosure home-relative`.
- **Strict Proposal-Only Safety Model**:
  - The native app and its bundled helper have strictly zero deletion privileges.
  - No `rm`, `unlink`, `trash`, `emptyTrash`, or shell execution code is present or linked.
  - Runtime verification asserts `mutation_authority == false` and `executor_linked == false` during engine handshake.
  - Includes interactive Dry-Run Simulation (`CleanupSimulationSheet`) and immutable Proposal Review (`CleanupProposalSheet`).
- **Bundled Engine Subprocess & Process Supervisor (`VacuaClient`)**:
  - Pure Swift 6 package (`apps/macos/Packages/VacuaClient`) dogfooding the official Model Context Protocol (MCP) JSON-RPC 2.0 interface over `stdio`.
  - Non-blocking `MCPStdioTransport` utilizing GCD `readabilityHandler` and async continuations to prevent actor thread deadlocks.
  - `EngineProcessSupervisor` managing child process lifecycle, automatic restarts, root transitions via `--allow-root`, and trusted bundled helper resolution (`Contents/Helpers/vacua-mcp`).
- **Cross-Language API Contract Fixtures (`fixtures/api/`)**:
  - 12 comprehensive contract fixtures verifying bidirectional serialization fidelity between Rust (`vacua-api`) and Swift (`VacuaClient`).
  - Unit and integration tests in both Rust and Swift testing all 19 schemas/DTOs.
- **Xcode Project & Build Automation**:
  - Generated Xcode project (`apps/macos/Vacua.xcodeproj`) with shared scheme `Vacua`, `VacuaTests`, and `VacuaUITests`.
  - Automated build script (`scripts/build-macos-app.sh`) compiling release Rust helpers and embedding them in `Contents/Helpers/`.
  - Packaging script (`scripts/package-macos-app.sh`) generating standalone `Vacua-v0.6.0-macos-arm64-unsigned.zip`.
  - CI workflow job `macos-app-check` building and testing the native app on GitHub Actions.

---

## [0.5.1] - 2026-09-29

MCP Truth, Privacy & Capability Policy Hardening: Authoritative startup canonicalized root authorization, symlink escape rejection, domain-separated BLAKE3 opaque IDs, recursive zero-leak privacy validation, physical reclaim lower-bound accounting, read-only SQLite open guarantees, CursorV2 query binding, snapshot resource template resolution, and pinned official Inspector CI qualification.

### Fixed & Hardened
- **Authoritative Root Authorization & Policy Boundary**:
  - Replaced raw path matching with `AllowedRoot` model performing startup canonicalization and directory validation; server fails closed on non-existent or invalid roots.
  - Replaced fail-open empty roots with strict fail-closed enforcement (empty roots deny all filesystem access).
  - Eliminated arbitrary path traversal by requiring all paths to reside within canonical allowed roots; returns `VACUA_POLICY_DENIED`.
  - Added symlink escape rejection: symlinks under allowed roots resolving to targets outside allowed roots are blocked.
  - Multi-root candidate caches and query parameters are strictly partitioned by `root_id`.
  - Historical snapshots and snapshot diffs are strictly restricted to configured allowed roots.
- **Path Privacy & Domain-Separated Opaque Identifiers**:
  - Replaced path-leaking member IDs and artifact IDs with deterministic BLAKE3 opaque identifiers (`cand-<hex>`, `dup-<hex>`, `mem-<hex>`, `art-<hex>`, `root-<hex>`).
  - Implemented recursive JSON privacy verification proving 0 raw path leaks across all outputs, errors, proposals, and resources in `Redacted` mode.
  - Non-home roots are displayed as `<root:{root_id}>/{rel_path}` in `HomeRelative` mode, eliminating full path leaks on external mounts.
  - Sanitized untrusted metadata: stripped ANSI escape sequences, C0/C1 control characters, and bidi override/isolate controls while preserving valid multi-language UTF-8.
- **Storage & Reclaim Truth**:
  - Eliminated `allocated_bytes == reclaim_bytes` conflation in candidate summaries, storage summaries, and simulations.
  - Added additive machine truth fields: `candidate_confirmed_reclaim_bytes`, `candidate_estimated_reclaim_bytes`, `candidate_reclaim_upper_bound`, `confirmed_reclaim_lower_bound`, `reclaim_upper_bound`.
  - Hardlinks and shared APFS clones report 0 bytes confirmed lower-bound reclaim until extent independence is verified.
  - Unknown APFS private sizes are truthfully marked `kernel_private_bytes_known = false` rather than assumed equal to allocated bytes.
  - Application artifacts calculate physical storage via `st_blocks * 512` rather than logical metadata length.
  - Clarified execution journal history semantics between immediate trash movement and eventual reclaim upon Trash emptying.
- **Read-Only SQLite Guarantees**:
  - Implemented `IndexDatabase::open_read_only` and `ExecutionJournal::open_read_only` using SQLite `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`.
  - Agent queries never perform background migrations, create WAL/SHM artifacts, or lock the database.
  - Outdated index schemas trigger `VACUA_STALE_STATE` with actionable guidance to run the standard CLI once.
- **Proposal-Only Plan Isolation**:
  - Propose-only plan tool no longer exposes executable `serialized_plan` by default; requires explicit `--allow-plan-export` flag AND `Full` path disclosure.
  - Protected candidates and unknown files return explicit `VACUA_PROTECTED` machine error codes on proposal attempts.
- **Machine Contract, Input Budgets & CursorV2**:
  - Upgraded cursors to `CursorV2` (Base64URL no-padding) binding offset to entity kind, root ID, and query fingerprint. Corrupted or mismatched cursors return `VACUA_INVALID_ARGUMENT`.
  - Enforced input bounds: max 200 proposal candidates, max 500 simulation candidates, max 512 bytes string parameters (returns `VACUA_LIMIT_EXCEEDED`).
  - Added typed enums `McpRiskFilter` and `ApplicationFilter` with strict schema validation.
- **Resource Templates & Tool Annotations**:
  - Implemented `get_snapshot` domain handler and `vacua://snapshot/{snapshot_id}` resource template read handler.
  - Updated tool annotations: duplicate analysis tools accurately set `readOnlyHint: false, destructiveHint: false, idempotentHint: true` reflecting fingerprint cache updates; all local tools declare `openWorldHint: false`.
  - Synchronous filesystem and application scanning offloaded from Tokio executor threads via `tokio::task::spawn_blocking` and bounded by semaphore.
- **Official Client Integration & CI Verification**:
  - Added comprehensive `rmcp` 3.5.0 client child-process test suite (`official_rmcp_client_tests.rs`).
  - Added pinned `@modelcontextprotocol/inspector@2.8.0` automated CI verification gate testing all tools, prompts, resources, and all 4 resource templates against real fixtures.

---

## [0.5.0] - 2026-09-29

### Added
- **Stable Public Machine DTO Boundary (`vacua-api`)**:
  - 18 public, versioned DTOs (`vacua.mcp.*.v1`) for storage state, snapshot diffs, cleanup candidates, application evidence graph, exact BLAKE3 duplicates, simulations, plan proposals, and execution history.
  - Canonical machine error codes (`VacuaErrorCode`) and JSON Schema generation via `schemars`.
  - Schema drift CI gate (`crates/vacua-api/tests/schema_drift.rs`) ensuring tracked schemas in `schemas/mcp/*.schema.json` match code exactly.
- **Agent-Native MCP Stdio Server (`vacua-mcp`)**:
  - Official `rmcp` 3.5.0 implementation supporting stdio transport, dynamic capability negotiation, tools, resources, and prompts.
  - **Compile-Time Capability Isolation**: `vacua-mcp` does not depend on `vacua-executor` under any code path. Zero deletion or execution authority. Verified via automated dependency boundary assertions in CI (`cargo tree -p vacua-mcp`).
  - **Stdout Purity**: Protocol stdout traffic is strictly framed JSON-RPC; all logs and tracing are routed exclusively to stderr.
  - **14 Tools**: `vacua_get_capabilities`, `vacua_storage_summary`, `vacua_list_snapshots`, `vacua_diff_snapshots`, `vacua_list_candidates`, `vacua_explain_candidate`, `vacua_list_applications`, `vacua_get_application`, `vacua_list_duplicates`, `vacua_get_duplicate_group`, `vacua_simulate_cleanup`, `vacua_propose_cleanup_plan`, `vacua_history_summary`, `vacua_verify_history`.
  - **Direct Resources & Templates**: `vacua://capabilities`, `vacua://storage/summary`, and templates for candidates, snapshots, duplicates, and applications.
  - **MCP Prompts**: `review_storage_growth` and `review_cleanup_proposal`.
  - **Propose-Only Plan Tool**: Generates immutable `CleanupPlan` v2 with SHA-256 seal and preservation guards; marked `proposal_status: PROPOSAL_ONLY_NOT_EXECUTABLE_VIA_MCP`.
- **Security & Untrusted Metadata Handling**:
  - Filesystem names and paths treated strictly as untrusted data with ANSI/control-character sanitization.
  - Path disclosure modes: `home-relative` (default), `full`, and `redacted`.
  - Allowed roots policy with pre-indexed bounds preventing arbitrary filesystem enumeration.
  - Bounded pagination (default 50, maximum 200) with opaque base64 cursors and concurrency semaphore for expensive operations.
- **Vendor-Neutral Agent Skill (`skills/vacua/SKILL.md`)**:
  - Operating manual for AI agents (Claude, Cursor, Codex, VS Code) outlining truth model, APFS clone/hardlink reclaim semantics, and execution prohibition.
- **Official MCP Inspector Qualification**:
  - Live qualification against `@modelcontextprotocol/inspector` v2.8.0 CLI for all tools, resources, and prompts.

---

## [0.4.1] - 2026-09-29

Content Integrity, APFS Reclaim Truth & Plan Safety Hardening: CleanupPlan schema v2, PreservationGuards, full preflight revalidation, APFS kernel private size accounting, conservative external hardlink/clone detection, fingerprint cache v4 with merge semantics, safe regular-file open primitives, and deterministic benchmark suite.

### Added
- **CleanupPlan Schema v2 (`vacua-plan`)**:
  - `CURRENT_PLAN_SCHEMA_VERSION = 2` with canonical deterministic SHA-256 hash using domain separator `VACUA_PLAN_V2\n`.
  - Hashing covers all safety-critical fields: schema version, ruleset version, creation timestamp, raw OsStr path bytes (`std::os::unix::ffi::OsStrExt`), risk, category, expected size, nanosecond mtime (`mtime_sec`, `mtime_nsec`), nanosecond ctime (`ctime_sec`, `ctime_nsec`), ContentGuards, and PreservationGuards.
  - Legacy schema v1 plans are strictly refused for destructive execution (`PlanExecutionRefused`).
  - Strict verification against hash tampering on all plan fields (`test_plan_hash_mutation_detected_for_all_fields`).
- **PreservationGuards & Target Content Revalidation (`vacua-plan`, `vacua-executor`)**:
  - Duplicate cleanup plans enforce `>= 1` `PreservationGuard` recording user's preserved copy with cryptographic BLAKE3 content digest.
  - All-item preflight: before any destructive mutation, ALL `PreservationGuard`s and `ContentGuard`s are verified. If the preserved copy or target content changed in any way, the entire plan aborts before any filesystem mutation occurs.
- **Safe Regular-File Open Primitives (`vacua-core::fs`)**:
  - Centralized `open_regular_file_safely` primitive on macOS/Unix using `O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK` followed by `fstat` verifying `S_IFREG`.
  - Completely blocks symlink substitution, FIFO blocking, socket/device opening, and special file processing.
- **APFS Kernel Private-Size Accounting (`vacua-scan`, `vacua-core`)**:
  - Integrated `ATTR_CMNEXT_PRIVATESIZE` via `fgetattrlist` on open file descriptors.
  - Determines exact bytes uniquely attributable to file and immediately reclaimable on deletion, excluding blocks shared across clone or snapshot relationships.
- **External Hardlink & APFS Clone Sharing Detection (`vacua-content::group`)**:
  - Distinguishes internal group aliases from external sharing (`HardlinkSharedExternal`, `APFSCloneSharedExternal`).
  - Unlinking a non-final hardlink confirms 0 bytes physical reclaim.
  - Keep-dependent dynamic reclaim calculation: `DuplicatePlanEstimate::for_keep(group, keep_path, selected_remove_paths)` replaces static estimates.
- **Fingerprint Cache Schema v4 (`vacua-index`)**:
  - Decoupled `sample_version` and `full_version` columns in SQLite.
  - Merging semantics: `put_sample` and `put_full` preserve valid staged hashes for identical stat identity, but completely invalidate stale hashes when size, mtime, or ctime changes.
  - Version mismatches guarantee cache misses.
- **Bounded Hash Worker Pool & Performance Hardening (`vacua-content::staged`)**:
  - Real worker pool governed by `--jobs` (default 4) for sample and full hash stages.
  - Hardlink collapse: hashes representative inode once and maps results back to all aliases, avoiding duplicate disk reads.
  - Small-file direct full hashing: files `<= 192 KiB` compute full hash in Stage 4 and bypass Stage 5 reread.
  - Per-file error resilience: unreadable files increment error counters without aborting full-tree scans.
- **Truthful Reclaim Accounting & CLI Semantics (`vacua-executor`, `vacua-cli`)**:
  - `ExecutionReport` accurately separates `bytes_moved_to_trash`, `estimated_eventual_reclaim_after_purge`, and `immediate_reclaimed_bytes` (0 for native trash).
  - CLI reporting reflects "Moved to Trash" and "Potential Reclaim After Trash Is Emptied" rather than claiming immediate free space.
- **Dataless Cloud Placeholder Classification Fix (`vacua-content::cloud`)**:
  - Deleted buggy `0x20` mapping (which collided with macOS `UF_COMPRESSED`).
  - Aligned strictly with macOS XNU `SF_DATALESS = 0x40000000`; local compressed files are never falsely classified as cloud placeholders.
- **Deterministic Benchmark Suite (`scripts/benchmark-dedup.py`)**:
  - PRNG-seeded deterministic data generation (`seed = 42`).
  - Real naive full-hash baseline via `vacua-naive-baseline` reading all bytes and computing BLAKE3 in 64 KiB chunks.
  - Automated structural assertions, machine-readable output in `benchmarks/dedup-v0.4.1.json`, and automated Markdown table generation.

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
