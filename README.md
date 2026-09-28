# Vacua

> **Storage intelligence for macOS.**

Understand what consumes space.  
Know what is safe to reclaim.  
Clean only with evidence.

`Rust core` · `APFS-aware` · `Incremental` · `Local-first` · `Agent-ready`

[![CI](https://github.com/yuanweize/vacua/actions/workflows/ci.yml/badge.svg)](https://github.com/yuanweize/vacua/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Platform](https://img.shields.io/badge/platform-macOS%20(Apple%20Silicon)-lightgrey.svg)](https://apple.com/macos)

---

```text
$ vacua scan

Storage Scan Report
──────────────────────────────────────────────────
Target Path:         /Users/developer
Files Scanned:       481,209
Directories:         94,112
Logical Content:     142.84 GiB
Physical Allocated:  118.20 GiB (allocated-block metrics)
Allocation Delta:    24.64 GiB (sparse files & block overhead)

Reclaimable Overview
──────────────────────────────────────────────────
Safe                 8.40 GiB  (Reproducible generated caches)
Review              21.70 GiB  (Reconstructable build artifacts)
Protected           63.20 GiB  (Hard invariants: SIP, Documents, Keys)

Top Recommendation
──────────────────────────────────────────────────
Target:              ~/Library/Developer/Xcode/DerivedData
Physical Space:      5.10 GiB
Risk Level:          SAFE (Confidence Score: 0.95)
Reconstructable:     Yes
Rebuild Effect:      Xcode will regenerate indices on next compilation.
Active Process:      Idle (xcodebuild not running)
```
*(Example scan output on a developer workstation)*

---

## Why Vacua?

Most macOS cleaning solutions rely either on surface-level heuristics, opaque commercial background daemons that misrepresent purgeable caches as free space, or shell scripts executing destructive `rm -rf` wildcards without understanding application ownership or APFS block extents.

**Vacua** is built on a different engineering philosophy:

> **Understand storage before deleting storage.**  
> *When certainty decreases, automation must decrease.*

- **Allocation-Aware Accounting**: Distinguishes true physical block allocation (`st_blocks * 512`) from logical file length, and explicitly tags APFS clone extent sharing uncertainties rather than fabricating exact savings.
- **Evidence-Driven Semantics**: Candidates carry a corroborating evidence vector (bundle IDs, process states, file ages, and rebuild consequences).
- **Inviolable Invariants**: System directories, SIP locations, SSH/GPG keys, and unrecognized files (`UNKNOWN`) are statically prevented from automated cleanup.
- **Two-Phase Immutable Plans**: All cleanup proposals compile into an immutable, SHA-256 signed plan with TOCTOU (Time-of-Check to Time-of-Use) pre-execution verification.
- **100% Local-First & Zero Telemetry**: Operates entirely offline with no telemetry, tracking, or background daemons.

---

## Architecture

Vacua is architected with complete decoupling between its core intelligence engine and presentation layers:

- **Core Engine (Rust)**:
  - `vacua-core`: Domain models, multi-tier storage pressure policy, and compile-time invariants.
  - `vacua-scan`: High-performance streaming filesystem scanner with bounded memory and symlink cycle safety.
  - `vacua-index`: SQLite metadata index and macOS FSEvents dirty-tree tracking.
  - `vacua-rules`: Declarative TOML rule evaluation with live process guards (`sysinfo`).
  - `vacua-risk`: Deterministic multi-signal risk and recommendation value evaluator.
  - `vacua-plan`: Two-phase immutable cleanup plan compiler with TOCTOU defenses.
  - `vacua-cli`: Developer-first command-line interface with human and versioned `--json` outputs.
- **On-Device Intelligence (`apple/VacuaIntelligence`)**: Optional Apple Foundation Models integration translating natural language prompts into typed `StructuredIntent`.
- **SwiftUI App (Planned)**: Native macOS desktop user interface.
- **Agent Server (Planned)**: Read-only Model Context Protocol (MCP) server for Claude, Cursor, and autonomous agents.

Read our complete [Architecture Specification](ARCHITECTURE.md) and [Architecture Decision Records](docs/adr/).

---

## Safety Guarantees

Every candidate is evaluated into one of five discrete risk tiers:

| Tier | Definition | Automated Proposal Allowed? | Example |
| :--- | :--- | :--- | :--- |
| **`SAFE`** | Completely reproducible generated cache/artifact with zero user data. | **Yes** (in user-approved plans) | Idle Xcode DerivedData, Homebrew download cache |
| **`REVIEW`** | Reconstructable, but incurs rebuild latency or network bandwidth. | **No** (requires approval) | `node_modules`, Python `venv`, Cargo `target/` |
| **`CAUTION`** | Leftover application data with potential configuration or ambiguity. | **No** (manual selection) | Uninstalled application support folders |
| **`PROTECTED`** | Critical system path, user document, security key, or active database. | **NEVER** | `~/.ssh`, `~/Library/Keychains`, SIP paths |
| **`UNKNOWN`** | Unrecognized or indeterminate files. | **NEVER** | Arbitrary unmapped directory |

Read our complete [Safety Specification](SAFETY.md) and [Threat Model](docs/THREAT_MODEL.md).

---

## Quick Start

### Build from Source

Requirements: macOS (Apple Silicon verified; Intel build compatibility in progress), Rust 1.80+.

```bash
# Clone the repository
git clone https://github.com/yuanweize/vacua.git
cd vacua

# Run test suite
cargo test --all

# Build release CLI binary
cargo build --release --bin vacua
```

### CLI Usage

```bash
# Analyze storage allocation under current directory
./target/release/vacua scan .

# Diagnose system storage pressure, APFS metrics, and Full Disk Access
./target/release/vacua doctor

# List safe cleanup candidates
./target/release/vacua candidates --risk safe

# Inspect full evidence vector for an item
./target/release/vacua explain <candidate_id>

# Generate an immutable cleanup plan
./target/release/vacua plan . --risk safe

# Machine-readable output for scripts and agents
./target/release/vacua --json scan .
```

---

## Engineering Reality Matrix

We maintain complete honesty regarding what is implemented, verified, and what is planned. Please refer to our [Feature Reality Matrix](FEATURE_REALITY_MATRIX.md) and [Roadmap](ROADMAP.md).

---

## License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
