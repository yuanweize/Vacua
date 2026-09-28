# Feature Reality Matrix

> **Rule of Engineering Honesty**:  
> Status may ONLY be marked as `VERIFIED` when automated test suites and compiler builds have successfully validated the implementation. No forward-looking claims or mock implementations may be marked as complete.

| Feature / Subsystem | Status | Implementation | Test | Limitation / Notes |
| :--- | :--- | :--- | :--- | :--- |
| **System Architecture & Safety Specs** | `VERIFIED` | `ARCHITECTURE.md`, `SAFETY.md`, `ROADMAP.md` | Doc reviews & specs | Phase 0 complete |
| **Safety Invariants & Hard Boundaries** | `VERIFIED` | `reclaim-core::invariants` | 4 unit tests (`test_protected_paths`, etc.) | Statically blocks system, SIP, SSH, mail, git |
| **APFS Allocation vs Logical Sizing** | `VERIFIED` | `reclaim-core::allocation`, `reclaim-scan::entry` | `test_scanner_deduplicates_hardlinks` | Uses `st_blocks * 512`, distinguishes sparse & diff |
| **Bounded Concurrency Scanner** | `VERIFIED` | `reclaim-scan::scanner` | 2 unit tests (`symlink_safety`, etc.) | Never follows directory symlinks; mount safe |
| **Declarative Rules Engine & Guards** | `VERIFIED` | `reclaim-rules::engine`, `guard` | Integration test pipeline | Live process checks (`sysinfo`), TOML rules |
| **Deterministic Risk Scoring** | `VERIFIED` | `reclaim-risk::evaluator` | 3 unit tests (`xcode_safe`, `ssh_protected`, etc.) | Invariants strictly enforced; Value separated |
| **Storage Pressure Model** | `VERIFIED` | `reclaim-core::pressure` | 1 unit test + live `statfs` | Evaluates raw free, ratio, thresholds policy |
| **CLI (scan, candidates, explain, plan, doctor)** | `VERIFIED` | `reclaim-cli::main` | Live binary invocation + `--json` | Standalone binary with human & JSON outputs |
| **End-to-End Pipeline & Plan Hashing** | `VERIFIED` | `reclaim-plan::plan`, `integration_pipeline.rs` | 1 integration test + live plan test | SHA-256 integrity hash + TOCTOU pre-check |
| **SQLite Metadata Index** | `NOT_STARTED` | Planned for Phase 2 (`reclaim-index`) | N/A | Schema designed, unbuilt |
| **FSEvents Incremental Monitoring** | `NOT_STARTED` | Planned for Phase 2 (`reclaim-index`) | N/A | Stream listener planned |
| **Transaction Audit Log & Trash Execution** | `NOT_STARTED` | Planned for Phase 2 (`reclaim-executor`) | N/A | Local SQLite execution records planned |
| **Application Evidence Graph** | `NOT_STARTED` | Planned for Phase 3 | N/A | Multi-signal orphan analysis planned |
| **Staged Duplicate Detection (BLAKE3)** | `NOT_STARTED` | Planned for Phase 3 | N/A | APFS clone-aware hashing planned |
| **Read-Only MCP Server** | `NOT_STARTED` | Planned for Phase 4 (`reclaim-mcp`) | N/A | Read-only tool schemas planned |
| **SwiftUI Native macOS App** | `NOT_STARTED` | Planned for Phase 5 (`apps/macos`) | N/A | Native macOS desktop UI planned |
| **Local Bayesian Preference Learning** | `NOT_STARTED` | Planned for Phase 6 | N/A | Beta-Bernoulli category scoring planned |
