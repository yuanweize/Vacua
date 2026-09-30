# Vacua Visual System & Design Architecture (v0.7.1)

Vacua is a native macOS storage intelligence application designed for technical users who need precision, performance, and clear truth boundaries. This document outlines the visual system, typography, semantic color rules, iconography, and accessibility invariants governing the macOS application.

---

## 1. Brand Attributes & Product Principles

Vacua's visual identity reflects:
- **Precise & Evidence-Driven:** Visualizations represent mathematical disk metrics (allocated blocks vs. logical length), never speculative or unmeasured estimates disguised as truth.
- **Quiet & Native:** Standard macOS components, materials, and typography rather than web-style custom chrome or heavy skins.
- **Trustworthy & Safe:** Clear boundaries around safety. Vacua is strictly an analysis and proposal tool in v0.7.1—it never uses destructive metaphors (e.g. trash cans, brooms, lightning bolts, fireworks).
- **Spatial & Accessible:** Information density is balanced with breathing room; color is always reinforced by symbols, patterns, and numeric labels.

---

## 2. Typography

Vacua relies exclusively on the Apple System Font (`San Francisco`) and standard dynamic system text styles to guarantee native look-and-feel and accessibility:

| Role | Font Style / Weight | Usage |
| :--- | :--- | :--- |
| **Window / Screen Title** | `.title2.bold()` | Primary screen title in headers |
| **Section Header** | `.headline` | Card and grouping titles |
| **Metric Value (Hero)** | `.system(size: 28, weight: .bold, design: .rounded)` | Large numerical storage values (e.g. Free Space, Total Scanned) |
| **Metric Value (Standard)** | `.body.monospacedDigit()` | Table and inspector byte amounts |
| **Body Text** | `.body` | Explanatory copy and descriptive guidance |
| **Secondary Metadata** | `.subheadline` / `.secondary` | Secondary details, timestamps, path sub-labels |
| **Badges & Indicators** | `.caption.weight(.medium)` | Status chips, risk indicators, delta symbols |
| **Treemap Micro-Labels** | `.system(size: 9...11, weight: .medium)` | Geometry-constrained Treemap cell labels |

---

## 3. Semantic Colors & Palette Hierarchy

Vacua avoids arbitrary hardcoded RGB values. All colors belong to distinct semantic tiers:

### 3.1. Brand Accent
- **Primary Brand:** Electric Cyan (`#00A6D9` / `RGB(0.0, 0.65, 0.85)`). Used for key brand accents, primary focus outlines, and the central negative-space aperture in the app icon.

### 3.2. Hierarchical Grouping (Treemap Type Mode)
In normal Treemap visualization, rectangles are colored using a restrained slate/indigo/teal palette derived deterministically from the node's path hash:
- **Invariant:** Grouping color implies **spatial hierarchy only**. It **never** implies reclaimability, risk, or deletion safety.
- Colors adapt smoothly across Dark and Light appearances.

### 3.3. Snapshot Delta Semantics
When in *Change Since Snapshot* mode, the authoritative Rust delta determines cell treatment:
- **Grown (`↑`):** Red (`Color.red`). File or directory allocation increased.
- **Shrunk (`↓`):** Green (`Color.green`). File or directory allocation decreased.
- **New (`●`):** Purple (`Color.purple`). Node was created after the base snapshot.
- **Unchanged (`—`):** Gray (`Color.secondary`). No change in allocation.

### 3.4. Risk & Safety Semantics
- **Protected (`shield.fill`):** Blue. System-critical or application-managed paths.
- **Review (`eye`):** Orange. User confirmation required before proposing action.
- **Caution (`exclamationmark.triangle`):** Yellow / Orange. Reclaim carries side-effects or cache invalidations.
- **Safe (`checkmark.circle`):** Green. Low risk, confirmed disposable candidate.
- **Unknown (`questionmark.circle`):** Gray. Unclassified or unmeasured.

---

## 4. Iconography (SF Symbols)

All UI icons are sourced from native **SF Symbols** compatible with macOS 15.0+:

| Destination / Semantic | SF Symbol | Rationale |
| :--- | :--- | :--- |
| **Overview** | `gauge.with.dots.needle.bottom.50percent` | System storage telemetry & pressure |
| **Storage Map** | `square.grid.2x2` | Treemap spatial partition |
| **Candidates** | `checklist` | Evidence review list (never `trash` or destructive metaphor) |
| **Duplicates** | `square.on.square` | Overlapping document copies |
| **Applications** | `app.badge` | Bundle identification and residual data |
| **Snapshots** | `clock.arrow.circlepath` | Temporal comparison (never camera photography) |
| **Engine / Diagnostics**| `waveform.path.ecg` / `gearshape` | Rust daemon telemetry and supervisor state |

---

## 5. Spacing & Layout Metrics

Centralized in `VacuaMetrics`:
- **Screen Padding:** 24 pt horizontal margin for primary content.
- **Card Spacing:** 16 pt between logical sections and overview cards.
- **Corner Radii:**
  - Card / Panel: 10 pt (native macOS rounded rectangle).
  - Badge / Tag: 4 pt.
  - Button / Interactive Control: 6 pt.
- **Inspector Width:** Minimum 280 pt, ideal 340 pt, maximum 450 pt. Allows collapsing on compact windows.

---

## 6. Accessibility & Invariants

1. **Differentiate Without Color:**
   - Every risk badge, delta state, and health indicator pairs color with a distinct text label and SF Symbol/glyph (e.g. `↑`, `↓`, `●`, `—`).
2. **Increase Contrast:**
   - Evaluated via `\.colorSchemeContrast`. Borders and selection outlines thicken and shift to high-contrast monochrome when enabled.
3. **Reduce Motion:**
   - Controlled by `\.accessibilityReduceMotion`. Disables complex spatial geometry transitions during Treemap drill-down.
4. **Full Keyboard Navigation:**
   - Treemap supports `Return` for drill-down and `Escape` for navigation back up. List View is provided as a 100% accessible fallback for screen readers (VoiceOver).
