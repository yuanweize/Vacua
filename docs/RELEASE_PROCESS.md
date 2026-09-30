# Vacua Release Process & Engineering Discipline

## 1. Core Principles & Immutability Invariants

1. **Tag Immutability**:
   A Git release tag (`vX.Y.Z`) is cryptographically signed/annotated and marks an immutable historical checkpoint. Once a tag is created and pushed, it must **never** be deleted, moved, retargeted, or force-pushed.
2. **Deterministic Failure Policy**:
   If a deterministic build, test, packaging, or release failure is discovered after a release tag has been created, the failure is **always repaired by a new patch version** (e.g. `v0.7.2` or `v0.8.1`), never by moving or recreating the tag.
3. **Branch & Tag Protection Inviolability**:
   Branch protection rules, tag rulesets, and release workflows must **never** be temporarily disabled or bypassed with admin overrides (`--admin`, `--clobber`, `--force`) to force a release. If an automation check fails, the underlying defect must be resolved via standard pull requests.
4. **Permanent Release Preflight Required**:
   Before any tag is created, the candidate commit must pass the end-to-end release preflight script (`scripts/release-preflight.sh`), ensuring version synchronization across all subsystems, clean workspace, and artifact generation proof.

---

## 2. Engineering Lesson: The v0.7.1 Incident

During the preparation of the v0.7.1 release, an uncommitted version bump in `VacuaIntelligenceCLI` caused the initial release pipeline run to fail after the `v0.7.1` tag was pushed. Under operational pressure, the tag was deleted, tag rulesets temporarily bypassed, and the tag moved to an updated commit.

### Retrospective Insights
- **Root Cause**: The release preflight validation was executed partially and manually rather than enforced deterministically across all version sources (including Swift CLI helpers).
- **Consequence**: Moving a release tag disrupts downstream packagers (such as Homebrew tap caches), breaks reproducible build provenance attestations, and compromises the cryptographic integrity expected of release artifacts.
- **Architectural Remedy**:
  1. Permanent version alignment test (`scripts/check-version-alignment.sh`) added to CI.
  2. Permanent non-publishing preflight runner (`scripts/release-preflight.sh` & `.github/workflows/release-preflight.yml`).
  3. Absolute rule: **A failure after tag push produces a patch release, never a moved tag.**

---

## 3. Authoritative Version Sources Matrix

All of the following version declarations must align exactly before a release:
- `Cargo.toml` (`[workspace.package].version`)
- All workspace member crates inheriting or specifying `version`
- `crates/vacua-cli` (`--version`)
- `crates/vacua-mcp` (`--version` and `--build-info`)
- `apple/VacuaIntelligence/Package.swift` and `VacuaIntelligenceCLI` (`--version`)
- `apps/macos/Vacua/Info.plist` (`CFBundleShortVersionString`)
- Xcode project `MARKETING_VERSION` in `apps/macos/Vacua.xcodeproj/project.pbxproj`
- `packaging/homebrew/vacua.rb.template`
- Contract fixtures and documentation matrices

---

## 4. Step-by-Step Release Workflow

### Step 1: Pre-Release PR & Version Bump
1. Create a release preparation branch: `git checkout -b chore/prepare-vX.Y.Z`.
2. Update versions across all authoritative sources.
3. Update `CHANGELOG.md` with structured release notes.
4. Run version alignment check:
   ```bash
   ./scripts/check-version-alignment.sh
   ```
5. Run full local release preflight:
   ```bash
   ./scripts/release-preflight.sh X.Y.Z
   ```
6. Open PR against `main`, ensure all CI checks pass cleanly, and merge without `--admin`.

### Step 2: Verification on Merged Main
1. Checkout `main` and pull fast-forward:
   ```bash
   git checkout main && git pull --ff-only
   ```
2. Verify tag does not already exist:
   ```bash
   git tag -l vX.Y.Z
   git ls-remote origin refs/tags/vX.Y.Z
   gh release view vX.Y.Z || true
   ```
3. Run release preflight on the exact merged commit:
   ```bash
   ./scripts/release-preflight.sh X.Y.Z
   ```

### Step 3: Tag Creation Exactly Once
Create the annotated release tag once preflight succeeds:
```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z - <Release Title>"
git push origin vX.Y.Z
```

### Step 4: GitHub Actions Release Workflow & Attestation
The tag push triggers `.github/workflows/release.yml`. It:
- Builds release binaries and bundles the native `Vacua.app`.
- Generates `RELEASE_MANIFEST.json` and `SHA256SUMS`.
- Signs and notarizes (if configured), and generates Sigstore/SLSA build attestations (`gh attestation`).
- Publishes the GitHub Release.

### Step 5: Post-Release Qualification & Homebrew Update
1. Download live artifacts from GitHub Releases and run:
   ```bash
   ./scripts/verify-release-integrity.sh vX.Y.Z
   ```
2. Verify GitHub attestations:
   ```bash
   gh attestation verify vacua-vX.Y.Z-aarch64-apple-darwin.tar.gz --repo yuanweize/vacua
   gh attestation verify Vacua-vX.Y.Z-macos-arm64-unsigned.zip --repo yuanweize/vacua
   ```
3. Generate the qualification report using `scripts/generate-release-qualification.sh`.
4. Update the Homebrew tap formula (`yuanweize/homebrew-tap/Formula/vacua.rb`) with the live artifact SHA256 and test:
   ```bash
   brew update && brew upgrade vacua && vacua --build-info
   ```
