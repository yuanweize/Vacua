#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

VERSION="0.1.0"
TARGET="aarch64-apple-darwin"
DIST_DIR="${REPO_ROOT}/dist"
PKG_NAME="vacua-v${VERSION}-${TARGET}"
PKG_DIR="${DIST_DIR}/${PKG_NAME}"
ARCHIVE="${DIST_DIR}/${PKG_NAME}.tar.gz"

echo "=== Building Vacua v${VERSION} (${TARGET}) Distribution Package ==="

# 1. Clean and prepare dist directory
rm -rf "${DIST_DIR}"
mkdir -p "${PKG_DIR}/bin"
mkdir -p "${PKG_DIR}/share/zsh/site-functions"
mkdir -p "${PKG_DIR}/share/bash-completion/completions"
mkdir -p "${PKG_DIR}/share/fish/vendor_completions.d"

# 2. Build Rust CLI
echo "--- Compiling vacua (release) ---"
cargo build --release --bin vacua --manifest-path "${REPO_ROOT}/Cargo.toml"
cp "${REPO_ROOT}/target/release/vacua" "${PKG_DIR}/bin/"

# 3. Build Swift Intelligence Helper
echo "--- Compiling vacua-intelligence (release) ---"
(
  cd "${REPO_ROOT}/apple/VacuaIntelligence"
  swift build -c release
)
cp "${REPO_ROOT}/apple/VacuaIntelligence/.build/release/vacua-intelligence" "${PKG_DIR}/bin/"

# 4. Generate Shell Completions
echo "--- Generating shell completions ---"
"${PKG_DIR}/bin/vacua" completions zsh > "${PKG_DIR}/share/zsh/site-functions/_vacua"
"${PKG_DIR}/bin/vacua" completions bash > "${PKG_DIR}/share/bash-completion/completions/vacua"
"${PKG_DIR}/bin/vacua" completions fish > "${PKG_DIR}/share/fish/vendor_completions.d/vacua.fish"

# 5. Copy Licenses and Metadata
cp "${REPO_ROOT}/LICENSE-MIT" "${PKG_DIR}/"
cp "${REPO_ROOT}/LICENSE-APACHE" "${PKG_DIR}/"

cat <<EOF > "${PKG_DIR}/README.txt"
Vacua v${VERSION}
Storage intelligence for macOS.
Website: https://github.com/yuanweize/vacua

Installation:
Move the executables in bin/ to your PATH (e.g. /usr/local/bin or ~/.local/bin):
    cp bin/vacua /usr/local/bin/
    cp bin/vacua-intelligence /usr/local/bin/

Verify:
    vacua --version
    vacua doctor
    vacua intelligence status
EOF

# 6. Record Build Provenance
GIT_SHA=$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo "release")
RUSTC_VER=$(rustc --version)
SWIFT_VER=$(swift --version | head -n 1)

cat <<EOF > "${PKG_DIR}/BUILD-INFO.txt"
Product: Vacua
Version: ${VERSION}
Target: ${TARGET}
Git Commit: ${GIT_SHA}
Rustc: ${RUSTC_VER}
Swift: ${SWIFT_VER}
Build Timestamp: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
EOF

# 7. Create Tarball
echo "--- Creating tarball: ${ARCHIVE} ---"
(
  cd "${DIST_DIR}"
  tar -czf "${PKG_NAME}.tar.gz" "${PKG_NAME}"
)

# 8. Compute Checksums
(
  cd "${DIST_DIR}"
  shasum -a 256 "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
  shasum -a 256 "${PKG_NAME}.tar.gz" > "SHA256SUMS"
)

echo "=== Package created successfully ==="
ls -lh "${DIST_DIR}"
cat "${DIST_DIR}/SHA256SUMS"
