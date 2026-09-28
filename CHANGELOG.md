# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-09-28

Initial open-source release of **Vacua**, an explainable, safety-first storage intelligence CLI for macOS.

### Added
- **Core Filesystem Scanner (`vacua-scan`)**:
  - Parallel bounded concurrency directory traversal.
  - Low-level APFS allocation accounting distinguishing logical bytes from physical disk blocks (`st_blocks * 512`).
  - Inode-based hardlink deduplication and strict directory symlink isolation.
- **Declarative Rule Engine & Risk Evaluation (`vacua-rules`, `vacua-risk`)**:
  - Declarative TOML-based cleanup rules with regex/glob matching.
  - Active process guards (e.g. aborts if `xcodebuild`, `cargo`, or `brew` is running).
  - Deterministic multi-factor risk categorization (`SAFE`, `REVIEW`, `CAUTION`, `PROTECTED`, `UNKNOWN`).
  - Compile-time and runtime invariant enforcement: `PROTECTED` and `UNKNOWN` files are fail-closed and cannot be auto-cleaned.
- **Two-Phase Immutable Planner (`vacua-plan`)**:
  - Cryptographic plan generation with SHA-256 integrity hash.
  - Itemized physical reclaim estimates and risk breakdowns.
- **Phase 2B Safe Executor & Audit Journal (`vacua-executor`)**:
  - Plan integrity verification against cryptographic hash.
  - Pre-execution TOCTOU verification (re-reads device ID, inode, and mtime; skips on mismatch).
  - Reversible execution by default via native macOS Trash (`~/.Trash`).
  - SQLite transaction audit journal (`~/.vacua/journal.db`) tracking every executed, skipped, or failed item.
- **Incremental Index & FSEvents Foundation (`vacua-index`)**:
  - SQLite index storage (`~/.vacua/index.db`) with schema migrations and session tracking.
  - FSEvents dirty-subtree tracking and coalescing.
- **Optional Native Apple Intelligence Bridge (`apple/VacuaIntelligence`)**:
  - Swift helper binary with process-isolated JSON v1 IPC over standard I/O.
  - Probes `SystemLanguageModel` availability on macOS.
  - Transparent telemetry reporting (`provider_requested`, `provider_used`, `apple_model_availability`).
  - Offline deterministic natural language parsing fallback when on-device model is not ready.
  - Zero deletion authority: AI translates natural language queries into `StructuredIntent`, never directly touching the filesystem or executor.
- **CLI Commands (`vacua-cli`)**:
  - `vacua scan <path>`: Physical vs logical storage scan.
  - `vacua candidates <path>`: Categorized cleanup opportunities with max risk filters.
  - `vacua explain <id>`: Human-readable evidence vector explaining classification reasons and rebuild effects.
  - `vacua plan <path> [-o <file>]`: Compiles and saves an immutable cleanup plan.
  - `vacua execute <plan> [--dry-run]`: Safe, TOCTOU-verified execution with interactive confirmation.
  - `vacua history [show <tx_id>]`: Transaction audit journal viewer.
  - `vacua doctor`: Storage health, APFS stats, FDA status, and system diagnostics.
  - `vacua completions <shell>`: Generates shell completion scripts (zsh, bash, fish).
  - `vacua intelligence status / parse`: Probes on-device model and inspects translated intent.
- **Packaging & Distribution**:
  - Standalone release bundle `vacua-v0.1.0-aarch64-apple-darwin.tar.gz` packaging `vacua` and `vacua-intelligence`.
  - Self-contained sibling binary discovery mechanism supporting Homebrew `bin/` and `libexec/`.
  - Deterministic demo harness (`scripts/generate-demo-fixture.sh`, `scripts/update-demo.sh`).
  - Visual identity and brand assets (`assets/brand/`, `assets/social/`, `assets/diagrams/`, `assets/demo/`).
