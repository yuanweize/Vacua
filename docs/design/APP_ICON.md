# Vacua App Icon Specification & Pipeline (v0.7.1)

## 1. Concept & Brand Identity

Vacua is an evidence-based macOS storage intelligence tool. Its icon intentionally rejects destructive and superficial "cleaner" tropes:
- **Rejected Clichés:** Trash cans, brooms, vacuum cleaners, rockets, lightning bolts, speedometers, glowing green shields, raw hard disk photographs.
- **Brand Attributes:** Precise, quiet, technical, trustworthy, native, spatial, evidence-driven.

### Selected Concept: Negative Space Matrix
- A disciplined slate-graphite squircle base plate.
- Arranged storage tiles/blocks forming an inner boundary.
- A deliberate, glowing electric cyan aperture ("void") at the center.
- The "void" symbolizes *Vacua*—storage visibility, available space, and reclaim opportunity discovered through mathematical proof, without prematurely promising deletion.

---

## 2. Source Files & Asset Architecture

All icon sources are original, version-controlled, and Apache-2.0 licensed:

- **Vector Master Source:** `design/icon/VacuaIcon.svg`
  - Dimensions: 1024 × 1024 px.
  - Safe Area: Inner squircle artwork centered within 832 × 832 px to comply with macOS 15 system-level rounding guidelines.
  - No baked double-rounding corners.
  - No text inside the icon.
- **Export Script:** `scripts/export_app_icon.swift`
  - Self-contained Swift script using native `CoreGraphics`, `ImageIO`, and `WebKit` / rasterization pipelines.
  - Generates the standard macOS AppIcon sizes deterministically without third-party web converters.
- **Compiled Asset Catalog:** `apps/macos/Vacua/Resources/Assets.xcassets/AppIcon.appiconset/`
  - Sizes generated:
    - 16x16 (@1x, @2x)
    - 32x32 (@1x, @2x)
    - 128x128 (@1x, @2x)
    - 256x256 (@1x, @2x)
    - 512x512 (@1x, @2x)
  - `Contents.json` specifying Xcode-compliant asset catalog layout.

---

## 3. Apple Human Interface Guidelines (macOS 15) Compliance

1. **Square Canvas with Safe Area:**
   Artwork is authored on a 1024x1024 canvas with proper margins, allowing macOS to apply the squircle mask cleanly.
2. **Layered Depth & Lighting:**
   Subtle top-down key lighting on the slate squircle with a 1px inner bezel highlight and faint drop shadow.
3. **Small-Size Legibility:**
   Verified at small system sizes (64px, 32px, 16px). The geometric contrast between the dark graphite tiles and the vivid electric cyan void ensures the silhouette remains instantly recognizable in the Dock, Finder, and Command-Tab switcher.
4. **No Text:**
   Does not print "Vacua", version numbers, or labels inside the icon canvas.

---

## 4. Build & Xcode Integration

- `scripts/generate_xcodeproj.py` is configured to register `Assets.xcassets` and configure `ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;`.
- Release build packages `Assets.car` inside `Vacua.app/Contents/Resources/`.
- `Info.plist` defines `CFBundleIconName = AppIcon` and `CFBundleIconFile = AppIcon`.

---

## 5. Licensing

All artwork, SVG paths, gradients, and scripts are 100% original work created for the Vacua project and distributed under the project's **Apache License 2.0**.
