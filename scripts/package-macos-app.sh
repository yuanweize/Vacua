#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [ -z "${1:-}" ]; then
  VERSION=$(grep -m 1 '^version = ' "${REPO_ROOT}/Cargo.toml" | cut -d '"' -f 2)
else
  VERSION="${1#v}"
fi

DIST_DIR="${REPO_ROOT}/dist"
mkdir -p "${DIST_DIR}"

echo "=== Packaging Vacua Native App v${VERSION} ==="

# 1. Run build script
"${SCRIPT_DIR}/build-macos-app.sh"

APP_PATH="${REPO_ROOT}/apps/macos/build/Vacua.app"
if [ ! -d "${APP_PATH}" ]; then
  echo "Error: Vacua.app not found at ${APP_PATH}" >&2
  exit 1
fi

# 2. Structural Verification of App Bundle
echo "--- Verifying App Bundle Structure and Binaries ---"
test -f "${APP_PATH}/Contents/Info.plist" || { echo "Missing Info.plist"; exit 1; }
test -f "${APP_PATH}/Contents/MacOS/Vacua" || { echo "Missing MacOS/Vacua executable"; exit 1; }
test -f "${APP_PATH}/Contents/Helpers/vacua" || { echo "Missing Helpers/vacua"; exit 1; }
test -f "${APP_PATH}/Contents/Helpers/vacua-mcp" || { echo "Missing Helpers/vacua-mcp"; exit 1; }
test -f "${APP_PATH}/Contents/Resources/LICENSE" || { echo "Missing Contents/Resources/LICENSE"; exit 1; }

echo "Verifying bundled helpers and build provenance identity..."
"${APP_PATH}/Contents/Helpers/vacua-mcp" --self-test

VACUA_BUILD_INFO=$("${APP_PATH}/Contents/Helpers/vacua" --build-info)
MCP_BUILD_INFO=$("${APP_PATH}/Contents/Helpers/vacua-mcp" --build-info)

VACUA_HELPER_VER=$(echo "${VACUA_BUILD_INFO}" | grep '"version":' | head -n 1 | awk -F'"' '{print $4}')
VACUA_HELPER_SHA=$(echo "${VACUA_BUILD_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')

MCP_HELPER_VER=$(echo "${MCP_BUILD_INFO}" | grep '"version":' | head -n 1 | awk -F'"' '{print $4}')
MCP_HELPER_SHA=$(echo "${MCP_BUILD_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')

BUNDLE_VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "${APP_PATH}/Contents/Info.plist")
BUNDLE_GIT_SHA=$(/usr/libexec/PlistBuddy -c "Print :VacuaGitCommit" "${APP_PATH}/Contents/Info.plist" 2>/dev/null || echo "unknown")

echo "Bundle Version:    ${BUNDLE_VERSION}"
echo "Bundle Git Commit: ${BUNDLE_GIT_SHA}"
echo "vacua Version:     ${VACUA_HELPER_VER} (${VACUA_HELPER_SHA})"
echo "vacua-mcp Version: ${MCP_HELPER_VER} (${MCP_HELPER_SHA})"

# Machine-assert version matches
if [ "${BUNDLE_VERSION}" != "${VERSION}" ]; then
  echo "Error: Bundle version mismatch: expected ${VERSION}, got ${BUNDLE_VERSION}" >&2
  exit 1
fi
if [ "${VACUA_HELPER_VER}" != "${VERSION}" ]; then
  echo "Error: vacua helper version mismatch: expected ${VERSION}, got ${VACUA_HELPER_VER}" >&2
  exit 1
fi
if [ "${MCP_HELPER_VER}" != "${VERSION}" ]; then
  echo "Error: vacua-mcp helper version mismatch: expected ${VERSION}, got ${MCP_HELPER_VER}" >&2
  exit 1
fi

# Machine-assert Git SHA matches across all binaries
if [ "${BUNDLE_GIT_SHA}" != "${VACUA_HELPER_SHA}" ]; then
  echo "Error: Provenance mismatch: App Git SHA (${BUNDLE_GIT_SHA}) != vacua helper (${VACUA_HELPER_SHA})" >&2
  exit 1
fi
if [ "${VACUA_HELPER_SHA}" != "${MCP_HELPER_SHA}" ]; then
  echo "Error: Provenance mismatch: vacua helper Git SHA (${VACUA_HELPER_SHA}) != vacua-mcp helper (${MCP_HELPER_SHA})" >&2
  exit 1
fi

echo "Verifying ad-hoc code signature..."
codesign -dv --verbose=4 "${APP_PATH}"

# 3. Create Zip Archive (preserving symlinks and permissions)
ZIP_NAME="Vacua-v${VERSION}-macos-arm64-unsigned.zip"
ZIP_PATH="${DIST_DIR}/${ZIP_NAME}"
rm -f "${ZIP_PATH}"

echo "--- Compressing Vacua.app into ${ZIP_NAME} ---"
(
  cd "${REPO_ROOT}/apps/macos/build"
  ditto -c -k --keepParent "Vacua.app" "${ZIP_PATH}"
)

# 4. Generate SHA256 Checksum
echo "--- Generating Checksum ---"
(
  cd "${DIST_DIR}"
  shasum -a 256 "${ZIP_NAME}" > "${ZIP_NAME}.sha256"
)

echo "=== App package created and verified successfully ==="
ls -lh "${DIST_DIR}/${ZIP_NAME}"*
cat "${DIST_DIR}/${ZIP_NAME}.sha256"
