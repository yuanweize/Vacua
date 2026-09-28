# Architecture of Project Reclaim (macOS Storage Intelligence Engine)

> **Core Axiom**: *Understand storage before deleting storage.*  
> **Operational Axiom**: *When certainty decreases, automation must decrease.*

---

## 1. System Overview

Project Reclaim is an explainable, safety-first, incremental storage intelligence engine built specifically for macOS. Unlike legacy script-based cleaners or UI-bloated utilities, Reclaim operates on deterministic evidence, precise APFS allocation accounting, immutable execution plans, and strict non-bypassable safety invariants.

```
+-----------------------------------------------------------------------------------+
|                                  Client Layer                                     |
|                                                                                   |
|   +--------------------+     +---------------------+     +--------------------+   |
|   |  Rust CLI (reclaim)|     |  SwiftUI macOS App  |     |  reclaim-mcp Agent |   |
|   | (Terminal / Shell) |     |  (Native Desktop)   |     |  (Claude/Codex/IDE)|   |
|   +---------+----------+     +----------+----------+     +---------+----------+   |
+-------------|---------------------------|--------------------------|--------------+
              |                           |                          |
              +-------------------+-------+                          |
                                  | (CLI/IPC/Lib)                    | (JSON-RPC)
+---------------------------------v----------------------------------v--------------+
|                           reclaim-core & Engine Layer                             |
|                                                                                   |
|  +---------------------+   +---------------------+   +-------------------------+  |
|  |    reclaim-scan     |   |    reclaim-rules    |   |      reclaim-risk       |  |
|  | - Streaming Traversal|  | - Declarative TOML  |   | - Evidence Aggregator   |  |
|  | - Bounded Workers   |   | - Process Guards    |   | - Invariant Enforcer    |  |
|  | - Mount Boundaries  |   | - Versioned Rules   |   | - Value Scoring         |  |
|  | - Stat Allocation   |   +----------+----------+   +------------+------------+  |
|  +----------+----------+              |                           |               |
|             |                         |                           |               |
|             v                         v                           v               |
|  +---------------------+   +---------------------------------------------------+  |
|  |    reclaim-index    |   |                   reclaim-plan                    |  |
|  | - SQLite Metadata   |   | - Immutable Plan Generation                       |  |
|  | - FSEvents Monitor  |   | - SHA-256 Plan Hash & TOCTOU Pre-verification     |  |
|  | - Rescan Diffing    |   +--------------------------+------------------------+  |
|  +---------------------+                              |                           |
|                                                       v                           |
|                                    +-----------------------------------+          |
|                                    |         reclaim-executor          |          |
|                                    | - macOS Trash API (Non-destructive) |        |
|                                    | - Disposable Sandbox Verifier     |          |
|                                    | - SQLite Transaction Audit Log    |          |
|                                    +-----------------------------------+          |
+-----------------------------------------------------------------------------------+
```

---

## 2. Competitive Landscape & Capability Matrix

| Feature / Dimension | tw93/Mole | Pearcleaner | CleanMyMac | disky / ncdu | Proposed Project (Reclaim) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Core Architecture** | Shell scripts / CLI | Swift / AppKit | C++ / Swift / Daemon | Rust / C (TUI/CLI) | **Rust Core + Rust CLI + SwiftUI GUI + MCP** |
| **Physical vs Logical Sizing** | No (Logical only) | No (Logical only) | Misleading (Purgeable as free) | Logical / Blocks | **Strictly separated (Allocated vs Extents vs Purgeable)** |
| **Storage Pressure Model** | Raw `df` output | None | Basic percentage | Raw size only | **Documented Multi-Tier Policy (APFS metrics)** |
| **Incremental Traversal** | Full scan every run | Scan on demand | Periodic polling daemon | Full scan | **SQLite metadata index + macOS FSEvents** |
| **Evidence & Explainability** | None (Scripted paths) | Basic path matching | Blackbox heuristics | None (Size only) | **First-class Evidence Vector with confidence** |
| **Safety Invariants** | Unsafe wildcard risks | Soft confirmation | Opaque whitelist | Read-only meter | **Hard-coded Invariant Layer (`PROTECTED` & `UNKNOWN`)** |
| **Transaction & TOCTOU** | Direct `rm -rf` | Direct `FileManager.trashItem` | Direct deletion | N/A | **Two-Phase Immutable Plan + TOCTOU pre-verification** |
| **Audit & Reversibility** | None | macOS Trash only | Internal log | None | **Structured Transaction Log + Trash by default** |
| **Agent / MCP Native** | None | None | None | None | **Read-Only MCP Server + Typed JSON Schema** |
| **Telemetry & Privacy** | Zero telemetry | Zero telemetry | Extensive analytics | Zero telemetry | **100% Local-First, Zero Telemetry** |

---

## 3. High-Level Subsystems

### 3.1 `reclaim-core`
The foundational crate defining shared models, contracts, and safety invariants:
- **Allocation Model**: Distinguishes `logical_bytes`, `allocated_bytes` (`st_blocks * 512`), `potentially_reclaimable_bytes`, and APFS clone extent uncertainty.
- **Evidence Model**: Represents signals contributing to classification (`PathSemantic`, `BundleIdentifier`, `ProcessRunning`, `FileAge`, `Reconstructability`).
- **Safety Invariant System**: Hard checks preventing mutation or deletion of system volumes, SIP paths, credentials, user document domains, and unknown items.
- **Storage Pressure Model**: Evaluates volume health based on raw available, important available, and opportunistic capacity.

### 3.2 `reclaim-scan`
High-performance, resource-bounded filesystem scanner:
- Bounded concurrency with backpressure via crossbeam/bounded channels.
- Metadata-only traversal (`libc::stat`, `symlink_metadata`). Does not read file contents during scanning.
- Mount-boundary detection to prevent unintended traversals into network shares (NFS/SMB) or Time Machine backup disks.
- Inode and device tracking (`st_dev`, `st_ino`) to deduplicate hard links and identify multi-link nodes.

### 3.3 `reclaim-rules`
Declarative rules engine:
- Rules defined as versioned data (TOML format), not executable scripts.
- Support for guard conditions: process running checks (`sysinfo`), minimum file age, and path glob matching.
- Built-in rule fallback for common developer caches (Xcode DerivedData, Cargo target, package manager caches).

### 3.4 `reclaim-risk`
Deterministic risk and value scoring:
- Maps evidence to five discrete risk classifications: `SAFE`, `REVIEW`, `CAUTION`, `PROTECTED`, `UNKNOWN`.
- Independent Value scoring (`HIGH`, `MEDIUM`, `LOW`, `NEGLIGIBLE`) based on physical footprint and reclaim impact. Risk is never lowered simply because an artifact is large.
- Produces human-readable explanations detailing why an artifact is safe, what generated it, and what happens when it is reclaimed.

### 3.5 `reclaim-index`
Incremental indexing engine:
- Embedded SQLite database maintaining path identities, inode fingerprints, allocated sizes, and classifications.
- Integration with macOS `FSEvents` API to detect path changes and trigger surgical rescans instead of repeated full disk scans.

### 3.6 `reclaim-plan` & `reclaim-executor`
Deterministic cleanup planning and safe execution:
- **Plan Generation**: Creates an immutable JSON plan containing target items, expected file IDs/inodes, sizes, and a SHA-256 plan hash.
- **TOCTOU Pre-verification**: Immediately before execution, re-verifies that paths exist, file IDs and sizes match, and risk classifications remain valid. If any discrepancy is detected, the item is skipped.
- **Non-Destructive Execution**: Moves targets to the macOS Trash (`FileManager.trashItem` / Darwin API) wherever possible. Permanent deletion is strictly restricted to ephemeral caches and explicitly flagged.
- **Transaction Audit Log**: Persists execution records (plan hash, candidate ID, path, action, rule, result) locally.

---

## 4. Key Design Decisions

1. **Decoupled Architecture**: All core logic lives in Rust libraries and CLI. The SwiftUI macOS app is a thin native client that interacts via shared library bindings or the structured JSON CLI interface.
2. **Deterministic over Opaque**: No opaque machine learning models decide deletion. Classification and risk scoring are 100% auditable, deterministic, and verifiable.
3. **Opt-in, Read-Only Agent Interface**: The `reclaim-mcp` server provides read-only discovery, inspection, and plan creation tools to LLM agents. Execution requires explicit out-of-band user approval.
