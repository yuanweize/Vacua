<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/vacua-lockup-dark.svg">
    <img src="assets/brand/vacua-lockup-light.svg" alt="Vacua — Storage intelligence for macOS" width="340">
  </picture>
</p>

<h1 align="center">Vacua</h1>

<p align="center">
  <strong>Storage intelligence for macOS.</strong><br>
  Understand what consumes space. Know what is safe to reclaim. Clean only with evidence.
</p>

<p align="center">
  <a href="https://github.com/yuanweize/vacua/actions/workflows/ci.yml"><img src="https://github.com/yuanweize/vacua/actions/workflows/ci.yml/badge.svg" alt="CI Status"></a>
  <a href="https://github.com/yuanweize/vacua/releases"><img src="https://img.shields.io/github/v/release/yuanweize/vacua?color=blue&label=release" alt="Release"></a>
  <a href="https://github.com/yuanweize/homebrew-tap"><img src="https://img.shields.io/badge/homebrew-yuanweize%2Ftap%2Fvacua-orange" alt="Homebrew"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue" alt="License"></a>
  <img src="https://img.shields.io/badge/platform-macOS%20%C2%B7%20Apple%20Silicon-lightgrey" alt="Platform">
</p>

---

## Quick Install

### Homebrew (Recommended)

```bash
brew install yuanweize/tap/vacua
```

Verify your installation:

```bash
vacua --version
vacua doctor
```

*(For manual tarball downloads or building from source, see [Installation Options](#installation-options).)*

---

## 30-Second Demo

```bash
# 1. Scan storage with APFS physical extent accounting (Read-Only)
$ vacua scan ~

# 2. Inspect classified candidates bounded by maximum risk
$ vacua candidates ~ --risk safe

# 3. Understand why an item was classified and what happens if cleaned
$ vacua explain b9a4fbf7

# 4. Generate an immutable, SHA-256 hashed cleanup plan
$ vacua plan ~ -o plan.json

# 5. Review preview and execute with live TOCTOU verification
$ vacua execute plan.json
```

<p align="center">
  <img src="assets/demo/terminal-scan.svg" alt="Vacua Scan Terminal Demo" width="720">
</p>

---

## Why Vacua?

Most cleanup utilities for macOS are either simplistic shell wrappers (`rm -rf ~/Library/Caches`) or closed-source commercial applications offering opaque "Scan & Clean" buttons.

Vacua is engineered around a core tenet: **Understand storage before modifying storage.**

- **APFS Extent-Aware Accounting**: Distinguishes logical file size from actual physical blocks (`st_blocks * 512`), properly accounting for sparse files and copy-on-write clone references.
- **Evidence-First Classification**: Every candidate is linked to an evidence vector detailing matched rules, active process guards, reconstructability ratings, and rebuild consequences.
- **Fail-Closed Safety Invariants**: `PROTECTED` locations (`/System`, `~/.ssh`, `~/.gnupg`, `~/Library/Keychains`, `.git/`) and `UNKNOWN` files are unconditionally blocked from automated cleaning.
- **TOCTOU Protected Execution**: Re-validates live device ID, inode, and modification time immediately before touching any file. If a file changed since the plan was compiled, it is skipped.
- **Reversible by Default**: Approved actions move items to the native macOS Trash (`~/.Trash`), preserving the ability to restore files via Finder.
- **Tamper-Evident Audit Journal**: Every execution records a local SQLite audit transaction tracking planned, moved, and skipped items.

---

## How It Works

```
Filesystem (APFS) ──► Scanner ──► Evidence Engine ──► Risk Engine
                                                            │
User NL Query ─────► Apple Intelligence ──► StructuredIntent ┼──► Immutable Plan ──► User Approval ──► Safe Executor
                                                            │
                                             (Zero Deletion Authority)
```

<p align="center">
  <img src="assets/diagrams/architecture.svg" alt="Vacua Architecture Diagram" width="740">
</p>

- **Deterministic Core (Rust)**: Handles bounded filesystem traversal, physical block analysis, rule evaluation, plan compilation, TOCTOU safety guards, and SQLite journaling.
- **Optional Native Intelligence (Swift)**: An isolated helper (`vacua-intelligence`) bridging Apple Foundation Models for natural language queries.

---

## Safety Invariants

Vacua enforces hard compile-time and runtime safety rules:

1. **Read-only by default**: `scan`, `candidates`, `explain`, and `plan` never mutate the filesystem.
2. **Unknown data is never auto-cleaned**: Unrecognized items are classified as `UNKNOWN` and can never be marked as `SAFE`.
3. **AI cannot lower risk or execute deletions**: LLM outputs are treated as untrusted user suggestions and cannot bypass policy invariants.
4. **Destructive actions require a verified plan and explicit user approval**: No arbitrary `rm` command exists in Vacua. Execution requires an immutable, hash-verified plan file.

For the formal safety proof and threat model, see [SAFETY.md](SAFETY.md) and [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).

---

## Apple Intelligence Status

Vacua features an optional native Swift helper that bridges Apple Foundation Models on supported Apple Silicon Macs running macOS Sequoia or newer.

```text
Optional on-device intent translation using Apple Foundation Models.
Vacua's safety decisions never depend on an LLM.
When Apple's model is unavailable, Vacua automatically falls back to deterministic intent parsing.
```

Probing system model availability:
```bash
$ vacua intelligence status
```

Testing natural language query parsing:
```bash
$ vacua intelligence parse "free 10GB of build caches safely"
```

```
Apple Intelligence Status
──────────────────────────────────────────────────
Provider Requested: Apple On-Device (Foundation Models)
Provider Used:      deterministic-fallback (or apple-on-device)
Model Availability: modelNotReady (or available)
Network Egress:     No (Strict on-device inference)
Role:               Intent translation (NL -> StructuredIntent)
Execution:          Forbidden (Zero deletion authority)
──────────────────────────────────────────────────
```

---

## Feature Matrix

| Capability | Engine | Mode | Status |
| :--- | :--- | :--- | :--- |
| **Filesystem Scanning** | Rust | Offline / Deterministic | Shipped (v0.1.0) |
| **APFS Extent Accounting** | Rust | Offline / Deterministic | Shipped (v0.1.0) |
| **Evidence & Risk Engine** | Rust | Offline / Deterministic | Shipped (v0.1.0) |
| **Immutable Cleanup Planner** | Rust | Offline / Deterministic | Shipped (v0.1.0) |
| **TOCTOU Safe Executor** | Rust | Offline / Reversible Trash | Shipped (v0.1.0) |
| **SQLite Transaction Journal** | Rust | Offline / Local Audit | Shipped (v0.1.0) |
| **Apple Intent Translation** | Swift | Optional On-Device / Fallback | Shipped (v0.1.0) |
| **Cloud AI / Telemetry** | None | Disabled / Zero Network Egress | Never / Excluded |
| **Read-Only MCP Server** | Specification | JSON v1 supported | Planned (Phase 4) |
| **Native SwiftUI GUI** | Swift | Desktop Interface | Planned (Phase 5) |

---

## CLI Reference

| Command | Description |
| :--- | :--- |
| `vacua scan <path>` | Scan directory tree and report physical vs. logical storage |
| `vacua candidates <path>` | List classified cleanup candidates (`--risk safe\|review\|caution`) |
| `vacua explain <id>` | Inspect detailed evidence vector and rebuild effects for a candidate |
| `vacua plan <path> [-o <file>]` | Compile a cryptographic cleanup plan |
| `vacua execute <plan> [--dry-run]`| Safely execute an approved cleanup plan with live TOCTOU guards |
| `vacua history [show <tx_id>]` | View past cleanup transactions and itemized audit records |
| `vacua doctor` | Diagnose storage pressure, APFS health, and permissions |
| `vacua completions <shell>` | Generate shell completions (`zsh`, `bash`, `fish`) |
| `vacua intelligence status` | Check Apple Foundation Models availability and IPC state |
| `vacua intelligence parse "<prompt>"` | Translate a natural language query into a structured intent |

---

## Installation Options

### 1. Homebrew Tap (Recommended)
```bash
brew install yuanweize/tap/vacua
```

### 2. Standalone Release Tarball
Download the pre-compiled binary package from [GitHub Releases](https://github.com/yuanweize/vacua/releases):

```bash
# Verify checksum
shasum -a 256 vacua-v0.1.0-aarch64-apple-darwin.tar.gz

# Extract and install
tar -xzf vacua-v0.1.0-aarch64-apple-darwin.tar.gz
cd vacua-v0.1.0-aarch64-apple-darwin
sudo cp bin/vacua bin/vacua-intelligence /usr/local/bin/
```

### 3. Build from Source
Requirements: macOS 14+, Rust 1.80+, Swift 6.0+, Xcode Command Line Tools.

```bash
git clone https://github.com/yuanweize/vacua.git
cd vacua

# Build Rust CLI
cargo build --release --bin vacua

# Build Swift Intelligence Helper
(cd apple/VacuaIntelligence && swift build -c release)

# Sibling discovery: place binaries together
cp apple/VacuaIntelligence/.build/release/vacua-intelligence target/release/
./target/release/vacua --version
```

---

## Documentation

- [Architecture & Design Decisions](ARCHITECTURE.md)
- [Safety Model & Invariant Enforcements](SAFETY.md)
- [Frequently Asked Questions (FAQ)](docs/FAQ.md)
- [Threat Model & Attack Vector Analysis](docs/THREAT_MODEL.md)
- [Brand Identity & Design Guidelines](assets/brand/BRANDING.md)
- [Uninstallation Guide](docs/UNINSTALL.md)
- [Rule Format Specification](docs/RULE_FORMAT.md)
- [Intelligence Bridge Architecture](docs/INTELLIGENCE.md)
- [Feature Reality Matrix](FEATURE_REALITY_MATRIX.md)
- [Contributing Guidelines](CONTRIBUTING.md)

---

## License

Dual-licensed under either:
- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
