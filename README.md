# Project Reclaim

> **An explainable, safety-first storage intelligence engine for macOS.**

Understand what is taking space.  
Know what is safe to reclaim.  
Clean only with evidence.

---

```text
$ reclaim scan

Storage Scan Report
──────────────────────────────────────────────────
Target Path:         /Users/alice
Files Scanned:       481,209
Directories:         94,112
Logical Content:     142.84 GiB
Physical Allocated:  118.20 GiB
Allocation Delta:    24.64 GiB (due to sparse files & block overhead)

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

---

## Why Reclaim?

Most macOS cleaning tools fall into one of two extremes:
1. **Opaque Commercial Utilities**: Bloated with memory cleaners and background daemons, calculating purgeable cache space as "freeable space" to inflate marketing numbers.
2. **Brittle Shell Scripts**: Wrapper scripts running destructive `rm -rf ~/Library/Caches/*` wildcards without understanding application ownership, APFS clone extents, or file locks.

**Project Reclaim** operates on a different principle:

> **Understand storage before deleting storage.**  
> *When certainty decreases, automation must decrease.*

- **True APFS Accounting**: Accurately measures physical block allocation (`st_blocks * 512`) vs logical size, and explicitly flags APFS clone extent uncertainties.
- **Evidence-Driven Classification**: Storage candidates carry a vector of corroborating evidence (bundle IDs, file age, process guards, and rebuildability).
- **Inviolable Invariants**: System directories, SIP locations, SSH/GPG keys, and unrecognized files (`UNKNOWN`) can **never** be automatically cleaned by any model or rule.
- **Two-Phase Immutable Plans**: All destructive actions require an immutable plan with cryptographic integrity hashes and TOCTOU pre-verification.
- **100% Local-First & Zero Telemetry**: Operates entirely offline with no telemetry, tracking, or unexpected background activity.

---

## Architecture at a Glance

Reclaim is engineered with a strict separation between its core intelligence engine and presentation layers:

- **Core Engine (Rust)**:
  - `reclaim-core`: Foundational models, storage pressure policy, and compile-time invariants.
  - `reclaim-scan`: High-performance, streaming filesystem scanner with bounded memory and symlink safety.
  - `reclaim-rules`: Declarative TOML rule evaluation with live process guards (`sysinfo`).
  - `reclaim-risk`: Deterministic multi-signal risk and value evaluator.
  - `reclaim-plan`: Two-phase immutable cleanup plan generator with TOCTOU defenses.
  - `reclaim-cli`: Developer-first command-line interface with native human and versioned `--json` outputs.
- **GUI (SwiftUI - In Progress)**: A native macOS desktop application interfacing via C-ABI / JSON IPC.
- **Agent Native (MCP - Planned)**: Read-only Model Context Protocol server exposing discovery and planning to AI pair programmers.

Read our complete [Architecture Specification](ARCHITECTURE.md) and [Architecture Decision Records](docs/adr/).

---

## Safety Guarantees

Every storage candidate is evaluated into one of five discrete risk tiers:

| Tier | Definition | Automated Proposal Allowed? | Example |
| :--- | :--- | :--- | :--- |
| **`SAFE`** | Completely reproducible generated cache/artifact with zero user data. | **Yes** (via approved plan) | Idle Xcode DerivedData, Homebrew download cache |
| **`REVIEW`** | Reconstructable, but incurs rebuild latency or network bandwidth. | **No** (requires approval) | `node_modules`, Python `venv`, Cargo `target/` |
| **`CAUTION`** | Leftover application data with potential configuration or ambiguity. | **No** (manual selection) | Uninstalled application support folders |
| **`PROTECTED`** | Critical system path, user document, security key, or active database. | **NEVER** | `~/.ssh`, `~/Library/Keychains`, SIP paths |
| **`UNKNOWN`** | Unrecognized or indeterminate files. | **NEVER** | Arbitrary unmapped directory |

Read our complete [Safety Specification](SAFETY.md) and [Threat Model](docs/THREAT_MODEL.md).

---

## Quick Start

### Build from Source

Requirements: macOS (Apple Silicon or Intel), Rust 1.80+.

```bash
# Clone the repository
git clone https://github.com/project-reclaim/reclaim.git
cd reclaim

# Run test suite
cargo test --all

# Build release CLI binary
cargo build --release --bin reclaim
```

### CLI Usage

```bash
# Analyze storage allocation under current directory
./target/release/reclaim scan .

# Diagnose system storage pressure, APFS metrics, and Full Disk Access
./target/release/reclaim doctor

# List safe cleanup candidates
./target/release/reclaim candidates --risk safe

# Inspect full evidence vector for an item
./target/release/reclaim explain <candidate_id>

# Generate an immutable cleanup plan
./target/release/reclaim plan . --risk safe

# Machine-readable output for scripts and agents
./target/release/reclaim --json scan .
```

---

## Development Status & Reality Matrix

We maintain complete engineering honesty regarding what is implemented, verified, and what is currently in design. Please refer to our [Feature Reality Matrix](FEATURE_REALITY_MATRIX.md) and [Roadmap](ROADMAP.md).

---

## License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
