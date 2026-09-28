# ADR 0001: Core Language & Client Decoupling Architecture

## Status
Accepted

## Context
Vacua aims to build a high-performance, explainable, safety-critical macOS storage intelligence system. The system must support:
1. Low-level, high-throughput POSIX and macOS filesystem operations (`getattrlist`, `stat`, `statfs`, APFS clone handling, FSEvents).
2. Predictable, bounded memory consumption during large-scale directory traversal (avoiding runaway GC pauses or multi-gigabyte memory footprints across millions of inodes).
3. Absolute memory safety and concurrency correctness to prevent data corruption or crashes in background monitoring.
4. Clean decoupling from GUI layers so that the engine can be executed headlessly by CLI scripts, background schedulers, CI pipelines, and AI agent protocols (MCP).
5. A native macOS desktop user experience following Apple Human Interface Guidelines without relying on heavy web runtimes (Electron/Tauri WebView) or background daemons.

## Considered Options
1. **Pure Swift (Swift Core + Swift CLI + SwiftUI)**:
   - *Pros*: Excellent macOS native integration, direct access to Cocoa/Foundation APIs.
   - *Cons*: Swift server/CLI packaging outside macOS is limited; package manager ecosystem for high-performance low-level concurrent filesystem scanning (e.g., crossbeam channels, memory-mapped SQLite optimizations) is less mature than Rust's; interoperability with external agents or cross-platform targets is more complex.
2. **Go Core + SwiftUI**:
   - *Pros*: Simple concurrency, good CLI ecosystem.
   - *Cons*: Go's runtime GC introduces latency spikes and unpredictable memory overhead during deep filesystem walks; Darwin system call integration requires cgo overhead; poor FFI story with Swift.
3. **Rust Core & CLI + Native SwiftUI macOS App**:
   - *Pros*:
     - Zero-cost abstractions, deterministic memory management without garbage collection.
     - Fearless concurrency (`Send`/`Sync`) preventing race conditions during parallel traversal.
     - Exceptional ecosystem for filesystem performance (`walkdir`, `crossbeam`, `rusqlite`, `sysinfo`).
     - Easy compilation into a clean, standalone, static CLI binary with zero runtime dependencies.
     - Robust C-ABI / FFI generation or IPC via typed JSON schema for the SwiftUI app and Swift intelligence modules.
     - Native SwiftUI frontend maintains premier macOS user interface quality and Apple ecosystem polish.

## Decision
We adopt **Rust** for the core engine (`vacua-core`, `vacua-scan`, `vacua-index`, `vacua-rules`, `vacua-risk`, `vacua-plan`, `vacua-cli`, `vacua-mcp`), and **Swift** for the standalone macOS GUI application and on-device Apple Intelligence adapter (`apple/VacuaIntelligence`).

All business logic, filesystem traversal, risk computation, and execution planning reside strictly in Rust. The presentation and platform intelligence layers consume the engine via structured IPC.

## Consequences
- **Positive**:
  - The CLI is instantly usable, scriptable, and distributable via Homebrew with zero dependencies.
  - Scanner achieves peak I/O throughput with bounded RSS memory.
  - The core can be exercised exhaustively via standard Rust unit, integration, and fuzz testing.
- **Negative / Mitigations**:
  - Requires maintaining a clean Rust-to-Swift IPC/FFI contract. *Mitigation*: Versioned JSON IPC contract documented in ADR 0005.
