# Feature Reality Matrix

> **Rule of Engineering Honesty**:  
> Status may ONLY be marked as `VERIFIED` when automated test suites and compiler builds have successfully validated the implementation. No forward-looking claims or mock implementations may be marked as complete.

| Feature / Subsystem | Status | Implementation | Test | Limitation / Notes |
| :--- | :--- | :--- | :--- | :--- |
| **System Architecture & Safety Specs** | `VERIFIED` | `ARCHITECTURE.md`, `SAFETY.md`, `ROADMAP.md` | Doc reviews & specs | Phase 0 complete |
| **Safety Invariants & Hard Boundaries** | `VERIFIED` | `vacua-core::invariants` | 4 unit tests (`test_protected_paths`, etc.) | Statically blocks system, SIP, SSH, mail, git |
| **APFS Allocation vs Logical Sizing** | `VERIFIED` | `vacua-core::allocation`, `vacua-scan::entry` | `test_scanner_deduplicates_hardlinks` | Uses `st_blocks * 512`, distinguishes sparse & flags extent uncertainty |
| **Bounded Concurrency Scanner** | `VERIFIED` | `vacua-scan::scanner` | 2 unit tests (`symlink_safety`, etc.) | Never follows directory symlinks; mount boundary safe |
| **Declarative Rules Engine & Guards** | `VERIFIED` | `vacua-rules::engine`, `guard` | Integration test pipeline | Live process checks (`sysinfo`), TOML rules |
| **Deterministic Risk Scoring** | `VERIFIED` | `vacua-risk::evaluator` | 3 unit tests (`xcode_safe`, `ssh_protected`, etc.) | Invariants strictly enforced; Value separated |
| **Storage Pressure Model** | `VERIFIED` | `vacua-core::pressure` | 1 unit test + live `statfs` | Evaluates raw free, ratio, thresholds policy |
| **CLI (scan, candidates, explain, plan, doctor)** | `VERIFIED` | `vacua-cli::main` | Live binary invocation + `--json` | Standalone binary with human & JSON outputs |
| **End-to-End Pipeline & Plan Hashing** | `VERIFIED` | `vacua-plan::plan`, `integration_pipeline.rs` | 1 integration test + live plan test | SHA-256 integrity hash + TOCTOU pre-check |
| **SQLite Metadata Index** | `VERIFIED` | `vacua-index::db`, `vacua-index::schema` | 4 unit tests (`test_migration_*`, `test_record_session_*`, `test_rebuild_*`) | `PRAGMA user_version` migrations, fail-closed version checking, session stats |
| **FSEvents Dirty-Tree Tracking** | `VERIFIED` | `vacua-index::fsevents` | 2 unit tests (`test_dirty_subtree_tracker_*`) | Hierarchical subtree coalescing, fallback full-rescan on dropped events |
| **StructuredIntent Schema** | `VERIFIED` | `schemas/structured-intent-v1.json` | Validated against Swift model & JSON outputs | Strict version 1, zero deletion authority, policy-bounded |
| **Apple Foundation Models Provider** | `VERIFIED` | `apple/VacuaIntelligence` | 3 Swift unit tests (`swift test`) + live CLI execution | Tested on macOS Darwin; probes `SystemLanguageModel` availability; deterministic fallback on `modelNotReady` |
| **Rust↔Swift IPC Boundary** | `VERIFIED` | `vacua-cli::find_intelligence_binary` | Integration CLI tests (`vacua intelligence status / parse`) | Stdin/stdout JSON v1 protocol (ADR-0005) with process isolation |
| **Intelligence Provider Boundary** | `VERIFIED` | `docs/INTELLIGENCE.md`, `ADR-0004` | Architecture validation | Strict invariant: AI output is untrusted input; zero deletion authority |
| **Transaction Audit Log & Trash Execution** | `NOT_STARTED` | Planned for Phase 2B (`vacua-executor`) | N/A | Local SQLite execution records planned |
| **Application Evidence Graph** | `NOT_STARTED` | Planned for Phase 3 | N/A | Multi-signal orphan analysis planned |
| **Staged Duplicate Detection (BLAKE3)** | `NOT_STARTED` | Planned for Phase 3 | N/A | APFS clone-aware hashing planned |
| **Read-Only MCP Server** | `NOT_STARTED` | Planned for Phase 4 (`vacua-mcp`) | N/A | Read-only tool schemas planned |
| **SwiftUI Native macOS App** | `NOT_STARTED` | Planned for Phase 5 (`apps/macos`) | N/A | Native macOS desktop UI planned |
| **Local Bayesian Preference Learning** | `NOT_STARTED` | Planned for Phase 6 | N/A | Beta-Bernoulli category scoring planned |
