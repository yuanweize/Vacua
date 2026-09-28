# ADR 0005: Rust ↔ Swift Inter-Process Communication (IPC) Protocol

## Status
Accepted

## Context
Vacua's architecture couples a high-performance Rust core engine with native macOS Apple Intelligence features written in Swift. We need an IPC mechanism to communicate between the Rust CLI and the Swift intelligence module that:
1. Avoids fragile FFI memory-management bugs between the Rust runtime and Swift ARC.
2. Supports standalone testing and compilation of both the Rust crates and the Swift package independently.
3. Allows graceful degradation when Apple Intelligence is unavailable or unsupported on the host Mac.
4. Maintains a versioned, typed contract.

## Decision
We adopt a **Versioned JSON Subprocess Protocol** over `stdin` and `stdout`:

1. **Protocol Envelope**:
   ```json
   // Request (Rust -> Swift via stdin)
   {
     "protocol_version": 1,
     "request_id": "req-1790629000",
     "action": "parse_intent" | "check_availability",
     "payload": { ... }
   }

   // Response (Swift -> Rust via stdout)
   {
     "protocol_version": 1,
     "request_id": "req-1790629000",
     "status": "success" | "error",
     "payload": { ... },
     "error_message": null
   }
   ```
2. **Channel Discipline**:
   - `stdout`: Strictly reserved for valid, machine-readable JSON payloads.
   - `stderr`: Diagnostic logging and debug traces.
3. **Graceful Fallback**:
   - If the Swift binary is absent or reports that Apple Intelligence is unavailable on the device, the Rust CLI falls back cleanly to deterministic heuristic rules or reports status without failing.

## Consequences
- **Positive**:
  - Loose coupling: Swift package can be built with `swift build` and tested with `swift test` without needing complex FFI bindings.
  - Process isolation: If the model provider crashes or runs out of memory, the Rust core process is unaffected.
- **Negative / Mitigations**:
  - Subprocess invocation adds small process spawn overhead (~10ms). *Mitigation*: Intelligence queries are user-interactive and infrequent, making 10ms unnoticeable.
