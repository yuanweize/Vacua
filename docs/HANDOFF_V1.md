# Vacua v1.0 Architectural Handoff & Next-Generation Roadmap

This document outlines the strategic directions and technical boundaries reserved for **Vacua v1.0**.

As established in v0.9.0, **Vacua solves the user's problem before attempting to impress the developer.** The v0.9.0 release delivers the core promise:
> Evidence-first storage intelligence: Diagnose → Explain → Prepare → Review → One Decision → Execute.

---

## 1. Core Principles to Preserve in v1.0

1. **Deterministic Safety Primacy**:
   - `SAFE_TO_RECLAIM`, `REVIEW_REQUIRED`, `PROTECTED` must remain derived exclusively from deterministic filesystem, compiler, and OS evidence.
   - AI and MCP maintain **zero mutation authority**.
   - `vacua-executor` remains the sole execution authority following explicit human approval.

2. **Storage Truth Over Illusion**:
   - Never equate logical bytes with physical APFS reclaim.
   - Never report planned reclaim as actual filesystem free-space deltas without measuring before/after kernel volume statistics.
   - Material discrepancies between volume usage and attributed user domains must remain reported truthfully as system-managed/unattributed.

3. **Self-Contained Distribution**:
   - The native macOS application must remain completely self-contained with bundled helpers in `Contents/Helpers/`.
   - Never depend on developer environments (`DerivedData`, Cargo trees, Xcode projects, or Homebrew).

---

## 2. Planned v1.0 Research & Feature Directions

### A. Longitudinal Storage Prediction & Trend Modeling
- **Context**: While v0.8/v0.9 track historical point-in-time snapshot differences (`vacua diff`), v1.0 can introduce predictive modeling.
- **Goal**: Analyze weekly growth velocity across domains (e.g. Docker images, Xcode DerivedData, simulator runtimes) to alert users *before* storage pressure reaches Critical.
- **Boundary**: Predictions are informational heuristics; they do not elevate risk scores or auto-trigger cleanups.

### B. Personalized Reclaim Preference Profiles
- **Context**: Different developers have different rebuild tolerances (e.g., fiber internet allows rapid re-download of dependency caches, while offline travel demands preservation).
- **Goal**: Allow users to configure deterministic preference profiles (e.g. "Frequent Builder", "Offline Traveler", "Minimalist").
- **Boundary**: Preference profiles can filter which *eligible safe groups* are selected by default, but can **never** reclassify `PROTECTED` or `REVIEW_REQUIRED` items into `SAFE`.

### C. Extended Ecosystem Integrations
- **Docker & Container Storage**: Read-only inspection of Docker disk images (`Docker.raw`) using safe metadata inspection without spawning uncontrolled background daemons.
- **Homebrew Cask / Cellar Analysis**: Extended package cache attribution.
- **Virtual Machines**: Parallels, UTM, and OrbStack image attribution.

### D. Advanced Apple Foundation Models & Multi-Turn Storage Guidance
- **Context**: As Apple updates on-device Foundation Models across macOS releases, prompt qualification and structured `@Generable` schemas should expand.
- **Goal**: Support natural conversational exploration of storage history ("Which project grew the most since last Tuesday?").
- **Boundary**: Tool calls remain strictly read-only query tools. Mutation tools are forbidden.

### E. Native Background Storage Pressure Monitor (Optional Agent)
- Lightweight LaunchAgent or menu bar extra that monitors `statfs` pressure events without waking the high-performance scanning engine until pressure crosses configured thresholds.

---

## 3. Non-Goals for v1.0

- **No Autonomous / Silent Background Deletion**: All destructive actions must continue to require explicit human plan confirmation.
- **No Cloud Upload or File-Content Telemetry**: Vacua remains strictly local and privacy-first.
- **No Dual Licensing**: Vacua remains strictly under `Apache-2.0`.
