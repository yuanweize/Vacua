#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

VERSION="${1:-}"
if [ -z "${VERSION}" ]; then
  VERSION=$(grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f 2)
else
  VERSION="${VERSION#v}"
fi

echo "=========================================================="
echo "Starting Permanent Vacua Release Preflight Suite v${VERSION}"
echo "=========================================================="
echo "Commit: $(git rev-parse HEAD)"
echo "Date:   $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
echo "Policy: PROOF ONLY — NO TAGS, NO PUBLISH, NO BRANCH MODS"
echo "=========================================================="

# 1. Version Alignment Invariants
echo ">>> [1/11] Checking version alignment across all subsystems..."
./scripts/check-version-alignment.sh

# 2. License Alignment Invariants (Apache-2.0)
echo ">>> [2/11] Checking license alignment & dependency compliance..."
./scripts/check-license-alignment.sh

# 3. Rustfmt and Clippy Lints
echo ">>> [3/11] Running Cargo format & clippy gates..."
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 4. Workspace Test Suite
echo ">>> [4/11] Running Rust workspace tests..."
cargo test --workspace --all-features

# 5. Capability Isolation Boundary
echo ">>> [5/11] Verifying architecture isolation boundaries..."
if cargo tree -p vacua-mcp | grep -q "vacua-executor"; then
  echo "FAIL: vacua-mcp links vacua-executor! Capability isolation violated." >&2
  exit 1
fi
echo "PASS: vacua-mcp has no linkage to vacua-executor."

# 6. Swift Intelligence Module Proof
echo ">>> [6/11] Verifying Swift intelligence compilation and capability proof..."
(
  cd apple/VacuaIntelligence
  swift run foundation-models-proof
  swift build
  swift run vacua-intelligence capabilities
  swift run vacua-intelligence test
)

# 7. VacuaClient Package Tests
echo ">>> [7/11] Running VacuaClient package tests..."
swift test --package-path apps/macos/Packages/VacuaClient

# 8. Xcode Project Generation & App Release Build
echo ">>> [8/11] Building native Vacua.app in Release configuration..."
python3 scripts/generate_xcodeproj.py
xcodebuild -project apps/macos/Vacua.xcodeproj \
  -scheme Vacua \
  -configuration Release \
  ARCHS="arm64" ONLY_ACTIVE_ARCH=NO \
  build CODE_SIGNING_ALLOWED=NO

# 9. Build and Package Release Artifacts
echo ">>> [9/11] Packaging Release artifacts (tarball & App zip)..."
export VACUA_GIT_SHA=$(git rev-parse HEAD)
./scripts/package-release.sh "${VERSION}"
./scripts/package-macos-app.sh "${VERSION}"
./scripts/generate-release-manifest.sh "${VERSION}"

# 10. Self-Test Packaged Binaries & Build Info
echo ">>> [10/11] Validating packaged binaries, helpers, and build-info..."
DIST_DIR="${REPO_ROOT}/dist"
tar -xzf "${DIST_DIR}/vacua-v${VERSION}-aarch64-apple-darwin.tar.gz" -C /tmp/
/tmp/vacua-v${VERSION}-aarch64-apple-darwin/bin/vacua-mcp --self-test
/tmp/vacua-v${VERSION}-aarch64-apple-darwin/bin/vacua --build-info
rm -rf "/tmp/vacua-v${VERSION}-aarch64-apple-darwin"

# 11. Run Complete Release Integrity Suite
echo ">>> [11/11] Running Release Integrity & Supply-Chain Guard..."
./scripts/test-release-integrity.sh

echo "=========================================================="
echo "RELEASE PREFLIGHT PASSED: COMMIT IS RELEASE-READY FOR v${VERSION}"
echo "NO TAGS CREATED. NO ASSETS PUBLISHED."
echo "=========================================================="
