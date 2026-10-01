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

# 1b. Build vacua-intelligence helper in release mode
echo "--- Compiling vacua-intelligence helper (release) ---"
(cd "${REPO_ROOT}/apple/VacuaIntelligence" && swift build -c release)
INTELLIGENCE_BIN="${REPO_ROOT}/apple/VacuaIntelligence/.build/release/vacua-intelligence"
if [ ! -f "${INTELLIGENCE_BIN}" ]; then
  echo "Error: Binary not found at ${INTELLIGENCE_BIN}" >&2
  exit 1
fi

BUILD_DIR="${REPO_ROOT}/apps/macos/build"
OUTPUT_APP="${BUILD_DIR}/Vacua.app"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

USE_XCODEBUILD=0
if command -v xcodebuild >/dev/null 2>&1; then
  if xcodebuild -version >/dev/null 2>&1; then
    USE_XCODEBUILD=1
  fi
fi

if [ "${USE_XCODEBUILD}" -eq 1 ]; then
  echo "--- Building Vacua.xcodeproj via xcodebuild (Release) ---"
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
  cp -R "${BUILT_APP}" "${OUTPUT_APP}"
else
  echo "--- Building Vacua native executable via SwiftPM (Release) ---"
  (cd "${REPO_ROOT}/apps/macos" && swift build -c release)
  SPM_BIN="${REPO_ROOT}/apps/macos/.build/release/Vacua"
  if [ ! -f "${SPM_BIN}" ]; then
    echo "Error: Built binary not found at ${SPM_BIN}" >&2
    exit 1
  fi
  mkdir -p "${OUTPUT_APP}/Contents/MacOS"
  mkdir -p "${OUTPUT_APP}/Contents/Resources"
  cp "${SPM_BIN}" "${OUTPUT_APP}/Contents/MacOS/Vacua"
  cp "${REPO_ROOT}/apps/macos/Vacua/Resources/Info.plist" "${OUTPUT_APP}/Contents/Info.plist"
  
  # Copy resource bundles if generated
  if [ -d "${REPO_ROOT}/apps/macos/.build/arm64-apple-macosx/release/VacuaApp_Vacua.bundle" ]; then
    cp -R "${REPO_ROOT}/apps/macos/.build/arm64-apple-macosx/release/VacuaApp_Vacua.bundle" "${OUTPUT_APP}/Contents/Resources/"
  fi
fi

# 3. Inject build provenance into Info.plist
echo "--- Injecting build provenance to Info.plist ---"
/usr/libexec/PlistBuddy -c "Set :VacuaGitCommit ${VACUA_GIT_SHA}" "${OUTPUT_APP}/Contents/Info.plist" 2>/dev/null || \
/usr/libexec/PlistBuddy -c "Add :VacuaGitCommit string ${VACUA_GIT_SHA}" "${OUTPUT_APP}/Contents/Info.plist"

# 4. Embed vacua, vacua-mcp, and vacua-intelligence into Contents/Helpers
echo "--- Embedding bundled helpers ---"
HELPERS_DIR="${OUTPUT_APP}/Contents/Helpers"
mkdir -p "${HELPERS_DIR}"
cp "${MCP_BIN}" "${HELPERS_DIR}/vacua-mcp"
cp "${VACUA_BIN}" "${HELPERS_DIR}/vacua"
cp "${INTELLIGENCE_BIN}" "${HELPERS_DIR}/vacua-intelligence"
chmod +x "${HELPERS_DIR}/vacua-mcp" "${HELPERS_DIR}/vacua" "${HELPERS_DIR}/vacua-intelligence"

# 5. Embed project license into Contents/Resources
echo "--- Embedding project license ---"
RESOURCES_DIR="${OUTPUT_APP}/Contents/Resources"
mkdir -p "${RESOURCES_DIR}"
cp "${REPO_ROOT}/LICENSE" "${RESOURCES_DIR}/LICENSE"

# 6. Ad-hoc codesign app bundle
echo "--- Ad-hoc signing Vacua.app ---"
codesign --force --deep --sign - "${OUTPUT_APP}"

echo "=== Vacua.app built successfully at ${OUTPUT_APP} ==="
ls -ld "${OUTPUT_APP}"
ls -lh "${HELPERS_DIR}"
