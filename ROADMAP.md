# Project Roadmap

The development of Vacua is divided into 7 distinct, sequential phases. Per our engineering principles, no feature will be marked as complete or verified without automated tests, benchmarks, and functional builds.

---

## Phase 0: Research, Architecture & Safety Model
- [x] Comprehensive competitive analysis (Mole, disky, Pearcleaner, CleanMyMac, ncdu).
- [x] macOS filesystem and storage API audit (`statfs`, `st_blocks`, APFS clones, FSEvents, TCC/FDA).
- [x] Architecture Threat Review and Threat Model (`docs/THREAT_MODEL.md`).
- [x] Definition of inviolable safety invariants and hard-coded protected boundaries (`SAFETY.md`).
- [x] Architecture Decision Records:
  - `ADR-0001`: Core Language Choice (Rust Core + Planned Swift GUI).
  - `ADR-0002`: Deterministic Risk Scoring & Value Separation.
  - `ADR-0003`: SQLite-backed Incremental Index & Migration Strategy.
  - `ADR-0004`: Intelligence Boundary & Schema Validation.
  - `ADR-0005`: Rust ↔ Swift IPC Protocol.
- [x] Candidate name research and collision check (Adopted: `Vacua`).

---

## Phase 1: Minimal Running Core & Streaming Scanner
- [x] Bounded-concurrency filesystem scanner with mount boundary and symlink safety (`vacua-scan`).
- [x] Allocated-block APFS accounting (`logical_size` vs `allocated_size` via `st_blocks * 512`) with explicit clone extent uncertainty.
- [x] Inode tracking and hard-link deduplication.
- [x] Declarative TOML rules engine with process running guards (`vacua-rules`).
- [x] Deterministic candidate and evidence data models (`vacua-core`).
- [x] Deterministic risk & value evaluation with invariant enforcement (`vacua-risk`).
- [x] Storage pressure model (`vacua-core::pressure`).
- [x] Immutable cleanup plan compiler with SHA-256 integrity hashing and TOCTOU pre-verification (`vacua-plan`).
- [x] CLI commands: `scan`, `candidates`, `explain`, `plan`, `doctor` with full `--json` support (`vacua-cli`).
- [x] In-sandbox integration test suite.

---

## Phase 2: Incremental Indexing, FSEvents & Intelligence Layer
- [x] SQLite metadata index for cached filesystem fingerprints (`vacua-index`).
- [x] macOS `FSEvents` stream listener foundation for surgical incremental dirty-tree detection (`vacua-index::fsevents`).
- [x] StructuredIntent shared JSON schema (`schemas/structured-intent-v1.json`).
- [x] Apple Foundation Models on-device intelligence prototype (`apple/VacuaIntelligence`).
- [x] Provider-neutral intelligence abstraction architecture (`docs/INTELLIGENCE.md`).
- [ ] Safe execution engine via macOS Trash (`FileManager.trashItem`) (Phase 2B).
- [ ] Structured SQLite transaction audit log (Phase 2B).

---

## Phase 3: Application Orphan Graph & Duplicate Engine
- [ ] Application Evidence Graph (Bundle IDs, LaunchAgents, App Support, Caches, Containers).
- [ ] Confidence-scored orphan detection (`ownership_confidence`, `orphan_confidence`).
- [ ] Staged duplicate detection (Size grouping -> Inode collapse -> Partial hash -> Full hash).
- [ ] BLAKE3 hashing engine with APFS clone awareness.
- [ ] Scan diffing (`vacua diff` between historical snapshots).
- [ ] Synthetic filesystem benchmarks (10k, 100k, 1M file trees).

---

## Phase 4: Agent Native Interface & MCP Server
- [ ] `vacua-mcp` read-only tool server (exposing discovery, inspection, and plan creation).
- [ ] Vendor-neutral Agent Skill (`skills/vacua/SKILL.md`).
- [ ] Structured CLI JSON schema specification (`schemas/vacua-v1.json`).

---

## Phase 5: Native SwiftUI macOS Frontend
- [ ] Pure native SwiftUI application (no Electron, no WebView, no local web server).
- [ ] Overview Dashboard: Storage pressure, safe reclaimable space, reviewable space.
- [ ] Visual Storage Map / Treemap (logical vs allocated block views).
- [ ] Application Management & Orphan Explorer.
- [ ] Developer Artifact Center.
- [ ] Full Disk Access (FDA) permission onboarding and status monitoring.

---

## Phase 6: On-Device Intelligence & Intent Translator
- [ ] Local-only Bayesian preference adaptation (Beta-Bernoulli category preferences).
- [ ] Full system integration of on-device Apple Intelligence models for natural language queries.
- [ ] Strict translation of NL prompts to deterministic `StructuredIntent` (never executing raw instructions).
