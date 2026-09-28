# Project Roadmap

The development of Project Reclaim is divided into 7 distinct, sequential phases. Per our engineering principles, no feature will be marked as complete or verified without automated tests, benchmarks, and functional builds.

---

## Phase 0: Research, Architecture & Safety Model
- [x] Comprehensive competitive analysis (tw93/Mole, Pearcleaner, CleanMyMac, GrandPerspective, disky, ncdu).
- [x] macOS filesystem and storage API audit (`statfs`, `st_blocks`, APFS clones, FSEvents, TCC/FDA).
- [x] Architecture Threat Review and Threat Model (`docs/THREAT_MODEL.md`).
- [x] Definition of inviolable safety invariants and hard-coded protected boundaries (`SAFETY.md`).
- [x] Architecture Decision Records:
  - `ADR-0001`: Core Language Choice (Rust Core + Swift GUI).
  - `ADR-0002`: Deterministic Risk Scoring & Value Separation.
- [x] Candidate name research and selection.

---

## Phase 1: Minimal Running Core & Streaming Scanner
- [ ] Bounded-concurrency filesystem scanner with mount boundary and symlink safety.
- [ ] Exact APFS allocation accounting (`logical_size` vs `allocated_size` via `st_blocks`).
- [ ] Inode tracking and hard-link deduplication.
- [ ] Declarative TOML rules engine with process running guards.
- [ ] Deterministic candidate and evidence data models.
- [ ] CLI commands: `scan`, `candidates`, `explain`, `doctor` (with `--json` support).
- [ ] Dry-run execution mode.
- [ ] In-sandbox integration test suite.

---

## Phase 2: Incremental Indexing, Risk Engine & Cleanup Plans
- [ ] SQLite metadata index for cached filesystem fingerprints.
- [ ] macOS `FSEvents` stream listener for surgical incremental updates.
- [ ] Comprehensive Evidence Aggregator and deterministic Risk/Value scoring.
- [ ] Immutable `CleanupPlan` generation with SHA-256 integrity hash.
- [ ] Pre-execution TOCTOU safety validator (re-verifying inode, size, mtime, and guards).
- [ ] Safe execution engine via macOS Trash (`FileManager.trashItem`).
- [ ] Structured SQLite transaction audit log.
- [ ] Advanced developer artifact rules (Xcode, Cargo, npm, venv, Homebrew).

---

## Phase 3: Application Orphan Graph & Duplicate Engine
- [ ] Application Evidence Graph (Bundle IDs, LaunchAgents, App Support, Caches, Containers).
- [ ] Confidence-scored orphan detection (`ownership_confidence`, `orphan_confidence`).
- [ ] Staged duplicate detection (Size grouping -> Inode collapse -> Partial hash -> Full hash).
- [ ] BLAKE3 hashing engine with APFS clone awareness.
- [ ] Scan diffing (`reclaim diff` between historical snapshots).
- [ ] Synthetic filesystem benchmarks (10k, 100k, 1M file trees).

---

## Phase 4: Agent Native Interface & MCP Server
- [ ] `reclaim-mcp` read-only tool server (exposing discovery, inspection, and plan creation).
- [ ] Vendor-neutral Agent Skill (`skills/reclaim/SKILL.md`).
- [ ] Structured CLI JSON schema specification (`schemas/reclaim-v1.json`).

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
- [ ] Optional macOS Foundation Models / Apple Intelligence integration for natural language queries.
- [ ] Strict translation of NL prompts to deterministic `StructuredIntent` (never executing raw instructions).
