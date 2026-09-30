# Vacua for macOS (Native SwiftUI Client)

Vacua includes a first-party native macOS desktop application (`apps/macos`) built with pure SwiftUI, Swift 6 (Strict Concurrency), and AppKit.

## Architecture: "Rust Owns Truth, Swift Owns Presentation"

Vacua follows a strict architectural boundary:

```text
┌────────────────────────────────────────────────────────┐
│               Vacua Native App (SwiftUI)               │
│  - NavigationSplitView & Modern macOS UI               │
│  - Storage Pressure Overview & Allocation Progress Bar │
│  - Candidate Table & Evidence Vector Inspector         │
│  - Duplicate Inspector & APFS Physical Truth Display   │
│  - Application Residue & Storage Snapshot Views        │
│  - Proposal & Simulation Inspection Sheets             │
└───────────────────────────▲────────────────────────────┘
                            │ JSON-RPC 2.0 over stdio
                            │ (Model Context Protocol)
┌───────────────────────────▼────────────────────────────┐
│          Bundled Helper Subprocess (vacua-mcp)         │
│  - Rust Storage Truth Engine                           │
│  - APFS Clone & Snapshot Analysis                      │
│  - Evidence Graph & Risk Classification                │
│  - Reclaim Bounds (Confirmed Lower vs Estimated Upper) │
│  - Zero Mutation Authority (Read/Analyze/Propose Only) │
└────────────────────────────────────────────────────────┘
```

### Core Design Guarantees

1. **Zero Execution / Mutation Authority**:
   - The native app client and its bundled helper have strictly zero deletion privileges.
   - `mutation_authority == false` and `executor_linked == false` are verified at runtime during the MCP handshake.
   - The app can simulate cleanups and compile verified proposals, but cannot delete or alter files.

2. **APFS Physical Storage Truth**:
   - Understands APFS copy-on-write clones: duplicate files sharing disk blocks are never falsely counted as reclaimable multiple times.
   - When APFS kernel private bytes cannot be measured directly without a snapshot diff, the UI explicitly displays **Unknown** rather than faking 0 or reporting misleading sums.
   - Confirmed lower bounds (guaranteed safe reclaimable space) are displayed distinctly from estimated upper bounds.

3. **No Embedded Web Views**:
   - Zero Electron, Tauri, or WebViews.
   - High performance, minimal memory footprint, and native macOS look and feel.

4. **Transparent Permissions**:
   - Standard user privileges by default.
   - Transparently diagnoses system Full Disk Access requirements without mock indicators.

## Running Locally

To build and run the native app during development:

```bash
# 1. Compile helper in debug mode
cargo build -p vacua-mcp

# 2. Run Swift package contract & transport tests
swift test --package-path apps/macos/Packages/VacuaClient

# 3. Build and run app via Xcode or xcodebuild
xcodebuild -project apps/macos/Vacua.xcodeproj -scheme Vacua -configuration Debug build
```

## Release Packaging & Security Notice

To build a standalone, ad-hoc signed distribution bundle:

```bash
./scripts/package-macos-app.sh 0.6.1
```

Output archive will be generated in `dist/Vacua-v0.6.1-macos-arm64-unsigned.zip`.

### Gatekeeper & Security Notice (Ad-Hoc Signing)

> [!IMPORTANT]
> **Ad-Hoc Signed Preview**: This native app preview release is ad-hoc signed (`codesign --sign -`) and is **not Developer ID signed or notarized** by Apple.

Because the binary does not carry an Apple Developer ID signature, macOS Gatekeeper (macOS Sequoia 15+) will display a security warning when launching the downloaded application.

**Recommended Way to Open**:
1. Open **System Settings → Privacy & Security**.
2. Scroll down to the **Security** section.
3. Locate the notice: *"Vacua was blocked from use because it is not from an identified developer"*.
4. Click **Open Anyway** and confirm with your macOS password or Touch ID.

Alternatively, if needed via terminal, explicitly scope quarantine attribute removal to the application bundle:
```bash
xattr -d com.apple.quarantine /Applications/Vacua.app
```
*(Never run recursive quarantine removal on `/` or root directories).*
