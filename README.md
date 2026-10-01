<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/vacua-mark-dark.svg">
    <img src="assets/brand/vacua-mark-light.svg" alt="Vacua Logo" width="80" height="80">
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
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="License"></a>
  <img src="https://img.shields.io/badge/platform-macOS%20%C2%B7%20Apple%20Silicon-lightgrey" alt="Platform">
</p>

---

---

## Your Mac is full?

Vacua first tells you where the space went.

Then it builds a verified cleanup plan.

You review **one plan**, not thousands of files.

Vacua only executes actions that pass its deterministic safety rules.

```text
Disk nearly full
        ↓
What is consuming it? (Whole-Volume Accounting & Domains)
        ↓
What can I safely reclaim? (SAFE_TO_RECLAIM vs REVIEW vs PROTECTED)
        ↓
How much can I reclaim? (Confirmed Physical Reclaim vs Logical Bounds)
        ↓
Can Vacua safely prepare everything? (Active Process Guard & Preservation Guards)
        ↓
User makes ONE decision (Group-level Review Sheet)
        ↓
Vacua executes the verified plan (vacua-executor via macOS Trash)
```

---

## 1. Storage Rescue & One-Decision Cleanup

When storage pressure is elevated or critical, Vacua launches directly into **Storage Rescue**:

- **Whole-Volume Accounting**: Uses kernel `statfs` facts to reconcile total capacity, volume used, volume available, Vacua-attributed domains, and unattributed/system-managed storage. Material discrepancies are shown truthfully rather than fabricating numbers.
- **Three Deterministic Evidence Levels**:
  - `SAFE_TO_RECLAIM`: Only items satisfying all hard safety requirements: verified build outputs with rebuild manifests, reconstructable dependency caches, disposable application caches, and duplicate copies with preservation guards.
  - `REVIEW_REQUIRED`: Downloads, user archives, personal documents, developer environments with incomplete rebuild evidence, and clone-shared items with unproven physical reclaim. Never silently included in the one-click safe plan.
  - `PROTECTED`: `/System`, `/usr`, `/bin`, `~/.ssh`, `~/.gnupg`, `~/Library/Keychains`, `.git/`, credentials, and system roots. Permanently blocked by hard invariants.
- **Group-Level Selection**: Instead of presenting a wall of thousands of file checkboxes, items are consolidated into canonical groups:
  - **Developer builds** (Rust `target/`, Xcode `DerivedData/`, CMake/Gradle)
  - **Dependency caches** (Cargo registry, npm/pnpm/yarn cache, uv cache, `__pycache__`)
  - **Application caches** (Verified disposable application/browser caches)
  - **Verified duplicates** (Bit-for-bit duplicate copies with >=1 copy permanently preserved)
- **One Approval**: The user reviews the consolidated safe plan and approves once. Detail inspection remains available but is completely optional.
- **Active Project & Process Guard**: Compiler and build processes (`rustc`, `cargo`, `xcodebuild`, `node`, `python`) and files modified within 180 seconds are detected live. Active build targets are automatically deferred to prevent disrupting in-flight work.
- **Preflight & Stale Plan Recalculation**: If filesystem state changes between review and execution, preflight rejects the stale plan and recalculates before requesting confirmation.

---

## 2. Storage Intelligence & Apple Foundation Models

Vacua integrates Apple Intelligence on Apple Silicon Macs running macOS 15+ as an **optional, read-only explanation and intent layer**:

- **Grounded Reasoning**: The model receives structured, bounded context (`StorageRescueSummaryV1`, `WholeVolumeAccountingV1`, `CandidateGroupSummaryV1`). It is prohibited from fabricating numbers, inventing paths, or overriding risk levels.
- **Zero Mutation Authority**: Apple Intelligence **cannot** authorize deletion, **cannot** modify `CleanupPlan`, and **cannot** invoke `vacua-executor`. Destructive authority resides exclusively in human confirmation of deterministic Rust plans.
- **Truthful Availability**: Inspects `SystemLanguageModel.default.availability` live (`.available`, `.deviceNotEligible`, `.appleIntelligenceNotEnabled`, `.modelNotReady`). When unavailable, the entire deterministic application functions with 100% feature completeness.

---

## 3. Security Architecture

```text
                        ┌─────────────────────────────────────┐
                        │   Apple Foundation Models (On-Dev)  │
                        │   "Ask Vacua" / Intent Translation  │
                        │   READ-ONLY EXPLANATION LAYER       │
                        └──────────────────┬──────────────────┘
                                           │
                                       READ ONLY
                                           │
                                           ▼
┌────────────────────────────────────────────────────────────────────────┐
│                      Deterministic Vacua Core (Rust)                   │
│                                                                        │
│   Whole-Volume Scan → Index → Evidence → Risk → Reclaim → Plan v2    │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                           EXPLICIT HUMAN APPROVAL
                                   │
                                   ▼
                        ┌─────────────────────┐
                        │   vacua-executor    │
                        │                     │
                        │  Preflight Checks   │
                        │  Journal Hash-Chain │
                        │  Native macOS Trash │
                        └─────────────────────┘

Hard Security Invariants:
  Apple Intelligence ────X────> vacua-executor (NO mutation authority)
  MCP Protocol       ────X────> vacua-executor (NO mutation authority)
  SwiftUI Front-End  ────X────> Direct File Deletion (NO direct unlink)
```

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

*(For standalone `.app` bundle, see [Production macOS App](#production-macos-app).)*

---

## Storage Rescue CLI

```bash
# 1. Analyze storage pressure and inspect safe reclaim opportunities
$ vacua rescue

# 2. Review proposed group-level plan in JSON format
$ vacua rescue --dry-run --json

# 3. Apply safe plan with single approval
$ vacua rescue --apply
```

---

## Duplicate Intelligence

Discover exact byte-identical duplicates without blind full-disk hashing, while respecting APFS copy-on-write sharing and hardlinks:

```bash
$ vacua duplicates ~ --min-size 10M
```

*(Example illustrative output)*:
```text
Exact duplicate groups:        14
Logical duplicate bytes:       38.2 GB
Confirmed reclaimable:         12.8 GB
Estimated reclaimable:         15.1 GB
APFS shared / clone extents:   18.3 GB
```

### Core Identity Principles
- **Logical duplicates != Physical reclaimability**: Hardlinks sharing the same inode reclaim **0 bytes** unless the last link is deleted. APFS clones share copy-on-write physical extents; deleting one copy frees only its exclusive private extents. Vacua reports confirmed lower bounds alongside logical duplication.
- **Sample hashes filter; Full BLAKE3 proves**: Deterministic 3-window sampling quickly rejects same-size non-duplicates, but exact duplicates are never declared without full cryptographic BLAKE3 verification.
- **Duplicate evidence is not deletion authority**: User documents are classified under `REVIEW` or `PROTECTED`. Cleaning requires explicit review (`vacua duplicates show <group-id>`) and compilation of an immutable, verified cleanup plan (`vacua duplicates plan <group-id> --keep <member>`). Zero direct deletion path exists.

---

## Hierarchical Storage Map

Explore filesystem storage distribution through a deterministic Rust hierarchical model and a native macOS Squarified Treemap:

```bash
$ vacua tree ~/Projects --metric allocated --limit 10
```

```text
/Users/developer/Projects
  48.21 GiB allocated (52.14 GiB logical)

  node_modules              18.42 GiB ( 38.2%)  [dir, 142100 files]
  build                     11.70 GiB ( 24.3%)  [dir, 1240 files]
  datasets                   9.10 GiB ( 18.9%)  [dir, 4 files]
  ...
  Other                      3.40 GiB (  7.1%)  [remainder, 38 items]
```

- **Logical vs. Allocated Views**: Separates nominal file length (`st_size`) from attributed filesystem allocation (`st_blocks * 512`). The tree explains observed namespace attribution, not freeable or unallocated disk space.
- **Hardlink-Aware Attribution**: When multiple hardlinks share an inode, allocated blocks are attributed exclusively to the lexicographically smallest canonical relative path. Peer aliases report `0` allocated bytes, eliminating double-counting.
- **APFS Clone Extent Uncertainty**: Copy-on-write clones may share physical extents even across distinct inodes; the engine explicitly flags extent-sharing uncertainty without pretending to know unprovable unique disk ownership.
- **Lazy Bounded Drill-Down**: Swift views navigate with sub-6ms latency across stable opaque node IDs (`stn_...`), backed by authoritative `remainder` aggregates for unreturned items.
- **Snapshot Growth Overlay**: Compares current directory allocations against historical storage snapshots, coloring treemap nodes by verified growth and shrinkage.

For full technical specifications, see [docs/STORAGE_MAP.md](docs/STORAGE_MAP.md) and [ADR 0007](docs/adr/0007-hierarchical-storage-tree.md).

---

## Why Vacua?

Most cleanup utilities for macOS are either simplistic shell wrappers (`rm -rf ~/Library/Caches`) or closed-source commercial applications offering opaque "Scan & Clean" buttons.

Vacua is engineered around a core tenet: **Understand storage before modifying storage.**

- **APFS Clone-Aware Reclaim Accounting**: Distinguishes logical file size from actual physical blocks (`st_blocks * 512`), properly accounting for copy-on-write clone references, hardlink sharing, and kernel private sizes (`ATTR_CMNEXT_PRIVATESIZE`) on supported APFS volumes.
- **Evidence-First Classification**: Every candidate is linked to an evidence vector detailing matched rules, active process guards, reconstructability ratings, and rebuild consequences.
- **Fail-Closed Safety Invariants**: `PROTECTED` locations (`/System`, `~/.ssh`, `~/.gnupg`, `~/Library/Keychains`, `.git/`) and `UNKNOWN` files are unconditionally blocked from automated cleaning.
- **TOCTOU Mitigated Execution**: Safely opens regular files via `O_NOFOLLOW | O_NONBLOCK`, re-validates live device ID, inode, regular file type, size, nanosecond mtime/ctime, and cryptographic BLAKE3 content digest immediately before moving to Trash. If a file changed since plan compilation, execution is aborted.
- **Native macOS Trash**: Approved actions invoke native macOS `-[NSFileManager trashItemAtURL:resultingItemURL:error:]` across volumes, preserving the ability to restore files via Finder.
- **Hash-Chained Audit Journal**: Every execution records a local SQLite audit transaction forming an unbroken SHA-256 hash chain verified via `vacua history verify`. *(Note: without a separately protected signing key, the chain detects unsynchronized modifications and broken links rather than preventing complete history rewrites by an attacker with direct database write access).*

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

Vacua features an optional native Swift helper that bridges Apple Foundation Models on supported Apple Silicon Macs.

> [!IMPORTANT]
> **Runtime Availability Truth**:
> - **Vacua Application Minimum**: macOS 15.0+ (Sequoia or newer).
> - **Foundation Models API**: Compiled conditionally under `#available(macOS 26.0, *)`.
> - **Real On-Device Neural Inference**: Requires an Apple Intelligence-eligible device, supported locale/region, Apple Intelligence toggled on in System Settings, and system model weights fully downloaded.
> - **Runtime Authority**: Vacua queries Apple's official `SystemLanguageModel.availability` API directly. It never assumes eligibility based solely on marketing hardware names. When models are unavailable, unready, or unsupported, Vacua falls back deterministically.

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
Model Availability: modelNotReady (or available / deviceNotEligible)
Network Egress:     No (Strict on-device inference)
Role:               Intent translation (NL -> StructuredIntent)
Execution:          Forbidden (Zero deletion authority)
──────────────────────────────────────────────────
```

---

## Feature Matrix

| Capability | Engine | Mode | Status |
| :--- | :--- | :--- | :--- |
| **Filesystem Scanning** | Rust | Offline / Deterministic | SHIPPED (v0.1.0) |
| **APFS Extent Accounting** | Rust | Offline / Deterministic | SHIPPED (v0.1.0) |
| **Evidence & Risk Engine** | Rust | Offline / Deterministic | SHIPPED (v0.1.0) |
| **Immutable Cleanup Planner** | Rust | Offline / Deterministic | SHIPPED (v0.1.0) |
| **TOCTOU Safe Executor** | Rust | Offline / Reversible Trash | SHIPPED (v0.1.0) |
| **SQLite Transaction Journal** | Rust | Offline / Local Audit | SHIPPED (v0.1.0) |
| **Content Duplicate Intelligence** | Rust | Offline / BLAKE3 Staged Pipeline | SHIPPED (v0.5.0) |
| **Agent MCP Server** | Rust (`rmcp`) | Stdio / Propose-Only / Isolated | SHIPPED (v0.5.1 / v0.8.0) |
| **Hierarchical Storage Map** | Rust / Swift | Subtree Rollup & Treemap GUI | SHIPPED (v0.7.0) |
| **Native SwiftUI App** | Swift | Desktop Interface (Proposal-Only) | SHIPPED (v0.7.0) |
| **Developer Artifacts Center** | Rust / Swift | Deterministic Rebuild Intelligence | SHIPPED (v0.8.0) |
| **Apple Neural Inference** | Swift | On-Device Foundation Models | IMPLEMENTED / Awaiting eligible-hardware runtime qualification |
| **Cloud AI / Telemetry** | None | Disabled / Zero Network Egress | Never / Excluded |

---

## Agent & MCP Interface (`vacua-mcp`)

Vacua provides an authoritative, capability-isolated, policy-hardened Model Context Protocol (MCP) server:

```text
                    ┌── CLI (`vacua`)
Vacua Domain Core ──┼── MCP Server (`vacua-mcp`, stdio)
                    └── Native SwiftUI App (`Vacua.app`)

vacua-mcp ──X──► vacua-executor (NO DEPENDENCY PATH, ZERO DELETION AUTHORITY)
```

The MCP interface allows external AI environments (**Claude Desktop**, **Cursor**, **Codex**, **VS Code**) to inspect APFS volume pressure, evaluate snapshot diffs, analyze multi-signal application evidence, identify exact BLAKE3 duplicates, simulate what-if reclaims, and propose cryptographically sealed `CleanupPlan` files.

### Compile-Time Capability Isolation & Policy Hardening
- `vacua-mcp` **does not link `vacua-executor`**. It is physically impossible for an AI agent to execute deletions, move items to Trash, run shell commands, or mutate files through the MCP server.
- **Authoritative Root Authorization**: All `--allow-root` paths are canonicalized at server startup. Empty roots fail closed. Traversal outside allowed roots and symlink escapes to external targets are strictly rejected with `VACUA_POLICY_DENIED`.
- **Zero Absolute Path Leaks in Privacy Modes**: Opaque domain-separated BLAKE3 identifiers (`cand-`, `dup-`, `mem-`, `art-`, `root-`) prevent path leakage. Redacted mode is verified via automated recursive tree audits across all outputs, errors, and resources.
- **Physical Storage & Reclaim Truth**: DTOs strictly separate confirmed lower bounds (0 bytes for shared extents/hardlinks) from conservative estimates. Unknown APFS private sizes remain unestimated (`known: false`).
- **Read-Only SQLite Access**: Agent metadata queries open databases using SQLite `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`, preventing implicit schema migrations or write-lock contention.
- **Proposal-Only Plans**: The server can compile immutable `CleanupPlan` v2 proposals, but raw `serialized_plan` export is disabled by default (`--allow-plan-export` required with `Full` path disclosure). Clients are directed to review proposals via the Vacua CLI.
- **Official Protocol & Inspector CI Qualification**: Passes standard `rmcp` 3.5.0 child process integration tests and automated GitHub Actions verification via pinned `@modelcontextprotocol/inspector@2.8.0`.
- For complete setup instructions and security details, see [docs/MCP.md](docs/MCP.md) and [skills/vacua/SKILL.md](skills/vacua/SKILL.md).


---

## CLI Reference

| Command | Description |
| :--- | :--- |
| `vacua scan <path> [-j N] [--incremental]` | Stream filesystem metadata with APFS clone accounting and bounded concurrency |
| `vacua index status` | Inspect persistent SQLite metadata index stats and watched FSEvents roots |
| `vacua index refresh <path>` | Surgically refresh dirty subtrees via native macOS `FSEventStream` replay |
| `vacua snapshot create <path> --name <name>` | Record an immutable point-in-time storage allocation snapshot |
| `vacua snapshot list` | List historical point-in-time snapshots and indexed sizes |
| `vacua diff <base> [target]` | Differential comparison of storage allocation deltas (or live `current`) |
| `vacua apps [show <bundle_id>]` | Inspect installed application bundles and associated filesystem residue |
| `vacua leftovers` | Detect uninstalled application residue with high orphan confidence |
| `vacua ask "<query>"` | Evidence-grounded natural language storage reasoning query engine |
| `vacua candidates <path>` | List classified cleanup candidates (`--risk safe\|review\|caution`) |
| `vacua explain <id>` | Inspect detailed evidence vector and rebuild effects for a candidate |
| `vacua artifacts <path>` | Inspect developer projects, generated artifacts, and rebuild evidence |
| `vacua plan <path> [--simulate]` | Compile immutable cleanup plan or simulate consequences without modifying disk |
| `vacua execute <plan> [--dry-run]`| Safely execute an approved cleanup plan with live TOCTOU guards via native Trash |
| `vacua history [show <tx_id>]` | View past cleanup transactions and itemized audit records |
| `vacua history verify` | Verify cryptographic SHA-256 hash chain integrity of the audit journal |
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
# Verify cryptographic provenance attestation
gh attestation verify vacua-v0.7.0-aarch64-apple-darwin.tar.gz --repo yuanweize/vacua

# Verify checksum
shasum -a 256 vacua-v0.7.0-aarch64-apple-darwin.tar.gz

# Extract and install
tar -xzf vacua-v0.7.0-aarch64-apple-darwin.tar.gz
cd vacua-v0.7.0-aarch64-apple-darwin
sudo cp bin/vacua bin/vacua-intelligence bin/vacua-mcp /usr/local/bin/
```

### 3. Native macOS App (SwiftUI)

Download the standalone `Vacua-v0.7.0-macos-arm64-unsigned.zip` from [GitHub Releases](https://github.com/yuanweize/vacua/releases), unzip, and drag `Vacua.app` to your `/Applications` folder:

- **Strictly Proposal-Only**: Zero mutation or deletion authority in the GUI client (`mutation_authority == false`).
- **Pure Native UI**: Built with pure SwiftUI and AppKit; no Electron or WebViews.
- **Storage Intelligence**: Visualizes APFS physical allocations, copy-on-write clone families, application residue, snapshot deltas, and evidence vectors.

> [!NOTE]
> **macOS Gatekeeper First Launch**: Because community builds are ad-hoc signed without an Apple Developer ID certificate, macOS Sequoia (15+) will block direct execution. On first launch, open **System Settings → Privacy & Security**, scroll down to the Security section, and click **Open Anyway** (or run `xattr -cr /Applications/Vacua.app`).

See [docs/MACOS_APP.md](docs/MACOS_APP.md) for complete architectural and operational details.

### 4. Build from Source
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

- [Calibrated Performance Benchmarks](BENCHMARKS.md)
- [Architecture & Design Decisions](ARCHITECTURE.md)
- [Native macOS App Architecture](docs/MACOS_APP.md)
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

Licensed under the **Apache License, Version 2.0** ([LICENSE](LICENSE)).
