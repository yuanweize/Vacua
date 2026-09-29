#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Building Vacua Native macOS App (SwiftUI) ==="

# 1. Build vacua-mcp helper in release mode
echo "--- Compiling vacua-mcp (release helper) ---"
cargo build --release --bin vacua-mcp --manifest-path "${REPO_ROOT}/Cargo.toml"

MCP_BIN="${REPO_ROOT}/target/release/vacua-mcp"
if [ ! -f "${MCP_BIN}" ]; then
  echo "Error: vacua-mcp binary not found at ${MCP_BIN}" >&2
  exit 1
fi

# 2. Build Vacua.app via xcodebuild
echo "--- Building Vacua.xcodeproj (Release) ---"
BUILD_DIR="${REPO_ROOT}/apps/macos/build"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

xcodebuild -project "${REPO_ROOT}/apps/macos/Vacua.xcodeproj" \
  -scheme Vacua \
  -configuration Release \
  -derivedDataPath "${BUILD_DIR}/DerivedData" \
  ARCHS="arm64" ONLY_ACTIVE_ARCH=NO \
  build CODE_SIGNING_ALLOWED=NO

BUILT_APP="${BUILD_DIR}/DerivedData/Build/Products/Release/Vacua.app"
if [ ! -d "${BUILT_APP}" ]; then
  echo "Error: Built app not found at ${BUILT_APP}" >&2
  exit 1
fi

# Copy built app to output folder
OUTPUT_APP="${BUILD_DIR}/Vacua.app"
rm -rf "${OUTPUT_APP}"
cp -R "${BUILT_APP}" "${OUTPUT_APP}"

# 3. Embed vacua-mcp into Contents/Helpers
echo "--- Embedding bundled vacua-mcp helper ---"
HELPERS_DIR="${OUTPUT_APP}/Contents/Helpers"
mkdir -p "${HELPERS_DIR}"
cp "${MCP_BIN}" "${HELPERS_DIR}/vacua-mcp"
chmod +x "${HELPERS_DIR}/vacua-mcp"

# 4. Ad-hoc codesign app bundle
echo "--- Ad-hoc signing Vacua.app ---"
codesign --force --deep --sign - "${OUTPUT_APP}"

echo "=== Vacua.app built successfully at ${OUTPUT_APP} ==="
ls -ld "${OUTPUT_APP}"
ls -lh "${HELPERS_DIR}"
