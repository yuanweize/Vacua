# Vacua App Icon Identity & Specification

## 1. Brand Concept & Rationale

Vacua is a high-performance native macOS application for spatial storage intelligence and deterministic capacity analysis.

### Avoided Tropes
- **No Trash Cans / Brooms / Rockets / Lightning Bolts**: Vacua is an evidence-driven analysis engine, not a destructive or speculative "cleaner/booster" utility.
- **No Text**: No "Vacua", "V0.7", or "Clean" badges inside the icon artwork.

### Chosen Direction: Concept A — Negative Space Matrix
- **Theme**: Geometric storage blocks arranged around a deliberate negative-space "void" (representing *Vacua* — space, observation, and reclaim opportunity).
- **Base**: A slate/graphite macOS squircle canvas (`#242933` to `#0E1015`) with a 0.5px technical edge bezel.
- **Storage Blocks**: 4 precision partitions representing hierarchical storage allocation, with specular bevel highlights.
- **Central Aperture (Void)**: Cold electric cyan (`#00E5FF`) and teal ambient glow radiating from the center, focusing on a precision evidence sensor node.

---

## 2. Size Hierarchy & Legibility Verification

| Render Size | Usage Context | Evaluation |
|---|---|---|
| **1024 × 1024** | App Store / Retina Marketing / Icon Composer | Rich specular lighting, fine partition grid, subtle glow radius. |
| **512 × 512** | Finder Preview / Large Spotlight | Sharp bevel highlights, distinct aperture void. |
| **256 × 256** | Finder Icon Grid / Launchpad | Strong matrix silhouette, clear cyan core. |
| **128 × 128** | Dock (Default Resolution) | High contrast between slate blocks and cyan sensor. |
| **32 × 32** | Finder Column View / Window Titlebar | Four-block layout and central glow remain distinct. |
| **16 × 16** | Menu bar / Small List View | Central cyan aperture anchors the icon against dark and light backgrounds. |

---

## 3. Source & Reproduction Pipeline

- **Master Vector Artwork**: `design/icon/VacuaIcon.svg`
- **Asset Catalog Target**: `apps/macos/Vacua/Resources/Assets.xcassets/AppIcon.appiconset/`
- **Regeneration Script**:
  ```bash
  swift scripts/export_app_icon.swift
  ```
- **License**: Apache-2.0, original project artwork.
