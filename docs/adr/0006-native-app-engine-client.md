# ADR 0006: First-Party Native macOS SwiftUI Frontend Dogfooding MCP Stdio Helper

## Status
Accepted (v0.6.0)

## Context
Vacua requires a first-party graphical user interface for macOS to enable visual exploration of storage pressures, cleanup candidates, duplicate sets, application residue, and APFS snapshots.

Key architectural considerations:
1. **Frontend Technology**: Electron, Tauri, WebViews, vs native macOS SwiftUI / AppKit.
2. **Bridge Mechanism**: Embedded C/Rust FFI (e.g. UniFFI, Swift-Bridge) vs Local Subprocess Dogfooding the Model Context Protocol (MCP) over `stdio`.
3. **Execution & Safety Boundary**: Should the GUI possess root execution or deletion authority?

## Decision

### 1. Pure Native macOS SwiftUI / AppKit
We reject Electron, Tauri, and WebViews in favor of pure, modern SwiftUI and AppKit (`macOS 15+`, Swift 6 with Strict Concurrency).
- **Rationale**: Minimal memory footprint, native APFS file drag/drop/system integration, Apple HIG compliance, and avoidance of heavy web runtimes for storage inspection tools.

### 2. "Rust Owns Truth, Swift Owns Presentation"
All domain logic, filesystem scanning, APFS clone analysis, duplicate identification, candidate risk classification, reclaim bound calculations, and proposal compilation remain exclusively in the verified Rust engine (`vacua-core`, `vacua-scan`, `vacua-risk`, `vacua-plan`, `vacua-api`).
- Swift code contains zero business logic, zero risk calculation formulas, and zero duplicate hashing heuristics.
- Swift acts purely as a presentation layer rendering typed DTOs received from the engine.

### 3. Bundled Helper Dogfooding MCP over Stdio
Instead of creating a custom FFI binding layer, the native app launches and supervises `vacua-mcp` as a dedicated child process communicating via standard Model Context Protocol (MCP) JSON-RPC 2.0 over standard I/O (`stdio`).
- **Rationale**:
  - Validates and dogfoods the official Vacua MCP interface in production.
  - Ensures absolute memory and process isolation: any engine panics or memory spikes cannot crash the UI host.
  - Reuses the identical audited API contracts and security guarantees.
  - Avoids dynamic linking complexity across Rust cdylib and Swift binaries.

### 4. Strictly Proposal-Only (Zero Mutation Authority)
The native app client and its bundled helper have strictly zero mutation authority:
- `mutation_authority == false`
- `executor_linked == false`
- No `rm`, `unlink`, `trash`, `emptyTrash`, or shell execution privileges.
- Cleanup actions are restricted to Dry-Run Simulation (`vacua_simulate_cleanup`) and Proposal Generation (`vacua_propose_cleanup_plan`).

## Consequences

### Positive
- Strict isolation: crashing engine cannot compromise frontend.
- Zero chance of accidental data loss: execution code is not even linked.
- Single contract source: identical JSON schemas serve both AI agents and native user interface.
- Lightweight distribution: standalone unsigned `.zip` or drag-and-drop `.app`.

### Negative
- Inter-process communication serialization overhead (negligible for JSON-RPC message rates over stdio pipes).
- Multi-process management requires robust process supervisor and graceful pipe cancellation.
