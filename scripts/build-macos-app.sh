#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Building Vacua Native macOS App (SwiftUI) ==="

VACUA_GIT_SHA="${VACUA_GIT_SHA:-$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo "release")}"
echo "Build Git SHA: ${VACUA_GIT_SHA}"

# 1. Build vacua and vacua-mcp helper in release mode
echo "--- Compiling vacua and vacua-mcp (release helpers) ---"
VACUA_GIT_SHA="${VACUA_GIT_SHA}" cargo build --release --bin vacua --bin vacua-mcp --manifest-path "${REPO_ROOT}/Cargo.toml"

MCP_BIN="${REPO_ROOT}/target/release/vacua-mcp"
VACUA_BIN="${REPO_ROOT}/target/release/vacua"
if [ ! -f "${MCP_BIN}" ] || [ ! -f "${VACUA_BIN}" ]; then
  echo "Error: Binaries not found at ${MCP_BIN} or ${VACUA_BIN}" >&2
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

# 3. Inject build provenance into Info.plist
echo "--- Injecting build provenance to Info.plist ---"
/usr/libexec/PlistBuddy -c "Set :VacuaGitCommit ${VACUA_GIT_SHA}" "${OUTPUT_APP}/Contents/Info.plist" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Add :VacuaGitCommit string ${VACUA_GIT_SHA}" "${OUTPUT_APP}/Contents/Info.plist"

# 4. Embed vacua and vacua-mcp into Contents/Helpers
echo "--- Embedding bundled helpers ---"
HELPERS_DIR="${OUTPUT_APP}/Contents/Helpers"
mkdir -p "${HELPERS_DIR}"
cp "${MCP_BIN}" "${HELPERS_DIR}/vacua-mcp"
cp "${VACUA_BIN}" "${HELPERS_DIR}/vacua"
chmod +x "${HELPERS_DIR}/vacua-mcp" "${HELPERS_DIR}/vacua"

# 4. Ad-hoc codesign app bundle
echo "--- Ad-hoc signing Vacua.app ---"
codesign --force --deep --sign - "${OUTPUT_APP}"

echo "=== Vacua.app built successfully at ${OUTPUT_APP} ==="
ls -ld "${OUTPUT_APP}"
ls -lh "${HELPERS_DIR}"
