# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phase 0 Architecture & Safety Model**:
  - Inviolable compile-time and runtime safety invariants (`PROTECTED` and `UNKNOWN` are never automatable).
  - Multi-tier storage pressure policy model (`NORMAL`, `ELEVATED`, `LOW`, `CRITICAL`).
  - Formal threat model covering 12 filesystem and AI execution attack vectors.
- **Phase 1 Streaming Scanner & Allocation-Aware Accounting**:
  - Bounded concurrency streaming filesystem scanner (`vacua-scan`).
  - Allocation-aware accounting distinguishing logical length from physical blocks (`st_blocks * 512`) and flagging APFS clone uncertainty.
  - Inode-based hard-link deduplication and strict symlink isolation.
  - Declarative TOML rule engine with live process guards (`vacua-rules`).
  - Deterministic risk & value evaluation with human-readable evidence explanations (`vacua-risk`).
  - Two-phase immutable cleanup plan compiler with SHA-256 integrity hash and pre-execution TOCTOU checks (`vacua-plan`).
  - Production CLI with human terminal and machine-readable `--json` modes (`vacua-cli`).
