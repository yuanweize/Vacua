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
- [x] Safe execution engine via macOS Trash (`vacua-executor`) (Phase 2B).
- [x] Structured SQLite transaction audit log (`vacua-executor::journal`) (Phase 2B).

---

## Phase 3: Application Orphan Graph, Snapshots, Differential Reasoning & On-Device Intelligence
- [x] Application Evidence Graph (Bundle IDs, LaunchAgents, App Support, Caches, Containers, Preferences, active processes).
- [x] Confidence-scored orphan detection (`orphan_confidence`).
- [x] Storage Snapshots & Diff Engine (`vacua snapshot create`, `vacua diff baseline current`).
- [x] Reclaim Cost Model & What-If Cleanup Simulator (`vacua plan --simulate`).
- [x] Storage Intelligence Query Engine (`vacua ask`) with grounded reference validation.
- [x] Typed Apple Foundation Models integration (`@Generable` guided generation, compile proof gate).
- [x] Calibrated synthetic filesystem benchmarks (10k, 100k files in `BENCHMARKS.md`).

---

## Phase 4A: Content Identity & Duplicate Intelligence
- [x] Bounded staged duplicate pipeline (Size grouping -> Hardlink collapse -> APFS clone classification -> Sample fingerprint -> Full BLAKE3).
- [x] Persistent SQLite fingerprint cache with nanosecond stat identity validation and automatic invalidation.
- [x] Physical reclaim accounting separating logical duplicates, APFS clone families, and hardlink sets.
- [x] Cloud placeholder safety preventing unintended background file hydration.
- [x] TOCTOU-safe hashing with open file descriptor verification.
- [x] Safe duplicate cleanup planning feeding immutable `CleanupPlan` (no automatic silent deletion).
- [x] CLI commands: `vacua duplicates`, `vacua duplicates show`, `vacua duplicates plan`, `vacua duplicates cache`.

---

## Phase 4B: Agent Native Interface & MCP Server
- [x] `vacua-mcp` official Model Context Protocol Rust SDK (`rmcp` 3.5.0) stdio server.
- [x] Compile-time capability isolation: `vacua-mcp` has zero dependency on `vacua-executor` and zero mutation authority.
- [x] Vendor-neutral Agent Skill (`skills/vacua/SKILL.md`).
- [x] Stable public Machine API DTOs (`vacua-api`) with 18 versioned JSON schemas (`schemas/mcp/*.schema.json`).
- [x] Automated schema drift test and dependency boundary CI verification.
- [x] 14 bounded tools, 2 direct resources, 4 resource templates, and 2 prompts.
- [x] Full `@modelcontextprotocol/inspector` v2.8.0 live CLI qualification.


---

## Phase 5: Native SwiftUI macOS Frontend
- [x] Pure native SwiftUI application (macOS 15+, Swift 6 strict concurrency, no Electron/WebView).
- [x] Overview Dashboard: Storage pressure, confirmed vs estimated reclaim bounds, index freshness.
- [x] Visual Storage Map / Treemap (deterministic Rust StorageTree engine, squarified layout, hardlink-aware allocated views, snapshot delta overlay).
- [x] Duplicate Explorer with APFS clone awareness and explicit BLAKE3 user analysis trigger.
- [x] Storage Snapshot Browser & point-in-time snapshot list.
- [x] Real Storage Snapshot Diff UI with Rust subtree delta rollups and horizontal delta chart.
- [x] Application Residue & Orphan Explorer with server-side query filtering.
- [x] Developer Artifact Center (deterministic rebuild evidence, ecosystem detection, strictly non-destructive).
- [~] Observed storage access diagnostics (non-authoritative heuristic; no fake TCC claim).

---

## Phase 6: Apple Intelligence Runtime Qualification & Grounded Reasoning
- [ ] On-device runtime qualification of `SystemLanguageModel` on eligible Apple Silicon hardware.
- [ ] Grounded storage reasoning bridging snapshots, tree attribution, and developer artifacts.
- [ ] Prompt evaluation and regression suite (`tests/intelligence/prompts/`) for system model updates.
- [ ] Local-only Bayesian preference adaptation (Beta-Bernoulli category preferences).

---

## Phase 7: Human-Controlled Native Execution Boundary
- [ ] Explicit human-controlled confirmation modal and full preflight in Native GUI.
- [ ] Reversible movement to macOS Trash via native Objective-C runtime FFI with SHA-256 journal verification.
- [ ] Strict isolation from AI/MCP: Foundation Models and MCP servers retain zero execution authority.
- [ ] Developer ID signing and Apple notarization pipeline.
