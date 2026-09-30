#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [ -z "${1:-}" ]; then
  VERSION=$(grep -m 1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)
else
  VERSION="${1#v}"
fi
TARGET="aarch64-apple-darwin"
DIST_DIR="${REPO_ROOT}/dist"
PKG_NAME="vacua-v${VERSION}-${TARGET}"
PKG_DIR="${DIST_DIR}/${PKG_NAME}"
ARCHIVE="${DIST_DIR}/${PKG_NAME}.tar.gz"

echo "=== Building Vacua v${VERSION} (${TARGET}) Distribution Package ==="

# 0. Clean tree and version gates
if git -C "${REPO_ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if [ -z "${ALLOW_DIRTY:-}" ]; then
    if ! git -C "${REPO_ROOT}" diff --quiet || ! git -C "${REPO_ROOT}" diff --cached --quiet; then
      echo "Error: Working tree is dirty. Refusing to package release." >&2
      exit 1
    fi
  fi
fi

CARGO_VER=$(grep -m 1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)
if [ "${CARGO_VER}" != "${VERSION}" ]; then
  echo "Error: Cargo workspace version (${CARGO_VER}) does not match release version (${VERSION})" >&2
  exit 1
fi

SWIFT_CLI_VER=$(grep -m 1 'static let version = ' "${REPO_ROOT}/apple/VacuaIntelligence/Sources/VacuaIntelligence/VacuaIntelligenceCLI.swift" | cut -d '"' -f 2)
if [ "${SWIFT_CLI_VER}" != "${VERSION}" ]; then
  echo "Error: VacuaIntelligenceCLI version (${SWIFT_CLI_VER}) does not match release version (${VERSION})" >&2
  exit 1
fi

VACUA_GIT_SHA="${VACUA_GIT_SHA:-$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo "release")}"
echo "Release Git Commit: ${VACUA_GIT_SHA}"

# 1. Clean and prepare dist directory
rm -rf "${DIST_DIR}"
mkdir -p "${PKG_DIR}/bin"
mkdir -p "${PKG_DIR}/share/zsh/site-functions"
mkdir -p "${PKG_DIR}/share/bash-completion/completions"
mkdir -p "${PKG_DIR}/share/fish/vendor_completions.d"

# 2. Build Rust CLI & MCP Server
echo "--- Compiling vacua and vacua-mcp (release) ---"
VACUA_GIT_SHA="${VACUA_GIT_SHA}" cargo build --release --bin vacua --bin vacua-mcp --manifest-path "${REPO_ROOT}/Cargo.toml"
cp "${REPO_ROOT}/target/release/vacua" "${PKG_DIR}/bin/"
cp "${REPO_ROOT}/target/release/vacua-mcp" "${PKG_DIR}/bin/"

echo "--- Validating binary build info ---"
"${PKG_DIR}/bin/vacua" --build-info
"${PKG_DIR}/bin/vacua-mcp" --build-info

# 3. Build Swift Intelligence Helper
echo "--- Compiling vacua-intelligence (release) ---"
(
  cd "${REPO_ROOT}/apple/VacuaIntelligence"
  swift build -c release
)
SWIFT_BIN_DIR=$(cd "${REPO_ROOT}/apple/VacuaIntelligence" && swift build -c release --show-bin-path)
cp "${SWIFT_BIN_DIR}/vacua-intelligence" "${PKG_DIR}/bin/"

# 4. Generate Shell Completions
echo "--- Generating shell completions ---"
"${PKG_DIR}/bin/vacua" completions zsh > "${PKG_DIR}/share/zsh/site-functions/_vacua"
"${PKG_DIR}/bin/vacua" completions bash > "${PKG_DIR}/share/bash-completion/completions/vacua"
"${PKG_DIR}/bin/vacua" completions fish > "${PKG_DIR}/share/fish/vendor_completions.d/vacua.fish"

# 5. Copy Licenses and Metadata
cp "${REPO_ROOT}/LICENSE" "${PKG_DIR}/"

cat <<EOF > "${PKG_DIR}/README.txt"
Vacua v${VERSION}
Storage intelligence for macOS.
Website: https://github.com/yuanweize/vacua

Installation:
Move the executables in bin/ to your PATH (e.g. /usr/local/bin or ~/.local/bin):
    cp bin/vacua /usr/local/bin/
    cp bin/vacua-intelligence /usr/local/bin/
    cp bin/vacua-mcp /usr/local/bin/

Verify:
    vacua --version
    vacua doctor
    vacua intelligence status
    vacua-mcp --version
EOF

# 6. Record Build Provenance
GIT_SHA="${VACUA_GIT_SHA}"
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
