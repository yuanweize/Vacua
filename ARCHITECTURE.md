# Architecture of Vacua (macOS Storage Intelligence Engine)

> **Core Axiom**: *Understand storage before deleting storage.*  
> **Operational Axiom**: *When certainty decreases, automation must decrease.*

---

## 1. System Overview

Vacua is an explainable, safety-first, incremental storage intelligence engine engineered natively for macOS. Unlike legacy script-based utilities or opaque commercial apps, Vacua operates on deterministic multi-signal evidence, allocated-block APFS accounting with explicit extent uncertainty, immutable execution plans, and compile-time non-bypassable safety invariants.

```
+-----------------------------------------------------------------------------------+
|                                  Client Layer                                     |
|                                                                                   |
|   +--------------------+     +---------------------+     +--------------------+   |
|   |   Rust CLI (vacua) |     |  SwiftUI macOS App  |     |   vacua-mcp Agent  |   |
|   | (Terminal / Shell) |     | (Native - Planned)  |     |  (Claude/Codex/IDE)|   |
|   +---------+----------+     +----------+----------+     +---------+----------+   |
+-------------|---------------------------|--------------------------|--------------+
              |                           |                          |
              +-------------------+-------+                          |
                                  | (CLI / IPC)                      | (JSON-RPC)
+---------------------------------v----------------------------------v--------------+
|                            vacua-core & Engine Layer                              |
|                                                                                   |
|  +---------------------+   +---------------------+   +-------------------------+  |
|  |     vacua-scan      |   |     vacua-rules     |   |       vacua-risk        |  |
|  | - Streaming Walk    |   | - Declarative TOML  |   | - Evidence Aggregator   |  |
|  | - Bounded Memory    |   | - Process Guards    |   | - Invariant Enforcer    |  |
|  | - Mount Boundaries  |   | - Versioned Rules   |   | - Value Scoring         |  |
|  | - Block Allocation  |   +----------+----------+   +------------+------------+  |
|  +----------+----------+              |                           |               |
|             |                         |                           |               |
|             v                         v                           v               |
|  +---------------------+   +---------------------------------------------------+  |
|  |     vacua-index     |   |                    vacua-plan                     |  |
|  | - SQLite Metadata   |   | - Immutable Plan Generation                       |  |
|  | - FSEvents Monitor  |   | - SHA-256 Plan Hash & TOCTOU Pre-verification     |  |
|  | - Rescan Diffing    |   +---------------------------------------------------+  |
|  +---------------------+                                                          |
+-----------------------------------------------------------------------------------+
```

---

## 2. Competitive Landscape & Capability Analysis

| Dimension | Mole | disky | Pearcleaner | CleanMyMac | Vacua |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Primary Implementation** | Shell / Binary / Companion | Rust CLI | Swift / AppKit | C++ / Swift / Daemon | **Rust Core + Rust CLI + Planned Native Swift** |
| **Storage Sizing Model** | Logical file sizing | Blocks / Logical | Logical sizing | Purgeable counted as free | **Allocated blocks (`st_blocks * 512`) + APFS clone extent uncertainty** |
| **Storage Pressure** | Raw capacity readout | Size metrics | None | Percentage alerts | **Documented Multi-Tier Policy (`NORMAL`/`ELEVATED`/`LOW`/`CRITICAL`)** |
| **Change Detection** | Full rescan per run | Full scan / diff | On-demand scan | Periodic polling daemon | **SQLite metadata index + macOS FSEvents dirty-tree tracking** |
| **Evidence & Explanations** | Scripted path definitions | Size trees | Bundle ID heuristics | Opaque heuristics | **First-class Evidence Vector with calibrated confidence scores** |
| **Safety Invariants** | Configurable whitelist | Interactive prompt | Confirmation dialog | Internal list | **Hard-coded in code: `PROTECTED` & `UNKNOWN` cannot be auto-cleaned** |
| **Execution Safety** | Direct commands / dry-run | Direct action | macOS Trash | Direct deletion | **Two-phase immutable plan (SHA-256) + TOCTOU pre-verification** |
| **Audit & Reversibility** | JSON history & log | None | macOS Trash | Proprietary log | **Structured SQLite transaction log + macOS Trash by default** |
| **Agent / MCP Native** | None | Agent-oriented JSON | None | None | **Read-Only MCP Server + Shared StructuredIntent schema** |
| **Telemetry & Privacy** | Zero telemetry | Zero telemetry | Zero telemetry | Analytics suite | **100% Local-First, Zero Telemetry** |

---

## 3. High-Level Subsystems

### 3.1 `vacua-core`
The foundational crate defining shared models, contracts, and safety invariants:
- **Allocation Model**: Distinguishes `logical_bytes`, `allocated_bytes` (`st_blocks * 512`), `potentially_reclaimable_bytes`, and explicitly flags APFS clone extent sharing uncertainty.
- **Evidence Model**: Multi-signal evidence representation (`PathSemantic`, `BundleIdentifier`, `ProcessState`, `FileAge`, `Reconstructability`, `RuleEngine`).
- **Safety Invariant System**: Compile-time and runtime checks preventing mutation or deletion of system volumes, SIP paths, credentials, user documents, and unknown items.
- **Storage Pressure Model**: Evaluates volume health based on raw available, important available, and opportunistic capacity under documented policy thresholds.

### 3.2 `vacua-scan`
High-performance, resource-bounded streaming filesystem scanner:
- Bounded memory traversal using `walkdir`.
- Metadata-only traversal (`libc::stat`, `symlink_metadata`). Does not read file contents during scanning.
- Mount-boundary detection to prevent unintended traversals into network shares (NFS/SMB) or Time Machine backup disks.
- Inode and device tracking (`st_dev`, `st_ino`) to deduplicate hard links and prevent double-counting physical blocks.

### 3.3 `vacua-rules`
Declarative rules engine:
- Rules defined as versioned data (TOML format), not executable scripts.
- Guard conditions: active process checks (`sysinfo`), minimum file age, and path glob matching.
- Built-in rule fallback for common developer caches (Xcode DerivedData, Cargo target, package manager caches).

### 3.4 `vacua-risk`
Deterministic risk and value scoring:
- Maps evidence to five discrete risk classifications: `SAFE`, `REVIEW`, `CAUTION`, `PROTECTED`, `UNKNOWN`.
- Independent Value scoring (`HIGH`, `MEDIUM`, `LOW`, `NEGLIGIBLE`). Value never lowers risk.
- Produces human-readable explanations detailing why an artifact is safe, what generated it, and what happens when it is reclaimed.

### 3.5 `vacua-index` (Phase 2A)
Incremental indexing engine:
- Embedded SQLite database maintaining volume identities, path fingerprints, allocated sizes, and classifications.
- Integration with macOS `FSEvents` API to detect changed directory trees and trigger surgical rescans instead of repeated full disk scans.

### 3.6 `vacua-plan`
Deterministic cleanup planning and TOCTOU defense:
- **Plan Generation**: Creates an immutable JSON plan containing target items, expected file IDs/inodes, sizes, and a SHA-256 plan hash.
- **TOCTOU Pre-verification**: Immediately before execution, re-verifies that paths exist, file IDs and sizes match, and risk classifications remain valid. If any discrepancy is detected, the item is skipped.
