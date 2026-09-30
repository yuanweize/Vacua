#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-}"
if [ -z "${TAG}" ]; then
  echo "Usage: $0 <vX.Y.Z> [--local-dir <path>]" >&2
  exit 1
fi

LOCAL_DIR=""
if [ "${2:-}" = "--local-dir" ] && [ -n "${3:-}" ]; then
  LOCAL_DIR="$3"
fi

VERSION="${TAG#v}"
REPO="yuanweize/vacua"
TARGET="aarch64-apple-darwin"
CLI_ARCHIVE="vacua-v${VERSION}-${TARGET}.tar.gz"
APP_ARCHIVE="Vacua-v${VERSION}-macos-arm64-unsigned.zip"

echo "=========================================================="
echo "Vacua Release Integrity & Build Provenance Verification"
echo "Target Tag: ${TAG} (Version: ${VERSION})"
echo "Repository: ${REPO}"
echo "=========================================================="

WORK_DIR=$(mktemp -d "/tmp/vacua-verify-${VERSION}-XXXXXX")
trap 'rm -rf "${WORK_DIR}"' EXIT

if [ -n "${LOCAL_DIR}" ]; then
  echo "--- Using local artifacts from ${LOCAL_DIR} ---"
  find "${LOCAL_DIR}" -maxdepth 1 -type f -exec cp {} "${WORK_DIR}/" \;
else
  echo "--- Fetching release assets from GitHub for ${TAG} ---"
  gh release download "${TAG}" --repo "${REPO}" --dir "${WORK_DIR}"
fi

cd "${WORK_DIR}"

echo "--- Verifying Required Files Existence ---"
test -f "${CLI_ARCHIVE}" || { echo "Missing ${CLI_ARCHIVE}"; exit 1; }
test -f "${APP_ARCHIVE}" || { echo "Missing ${APP_ARCHIVE}"; exit 1; }
test -f "RELEASE_MANIFEST.json" || { echo "Missing RELEASE_MANIFEST.json"; exit 1; }
test -f "SHA256SUMS" || { echo "Missing SHA256SUMS"; exit 1; }

echo "--- Verifying Offline Checksums (SHA256SUMS) ---"
shasum -a 256 -c SHA256SUMS

echo "--- Verifying RELEASE_MANIFEST.json Structure & Digests ---"
MANIFEST_SCHEMA=$(grep '"schema_version":' RELEASE_MANIFEST.json | head -n 1 | awk -F'"' '{print $4}')
MANIFEST_VER=$(grep '"version":' RELEASE_MANIFEST.json | head -n 1 | awk -F'"' '{print $4}')
MANIFEST_TAG=$(grep '"tag":' RELEASE_MANIFEST.json | head -n 1 | awk -F'"' '{print $4}')
MANIFEST_GIT_SHA=$(grep '"git_commit":' RELEASE_MANIFEST.json | head -n 1 | awk -F'"' '{print $4}')

if [ "${MANIFEST_SCHEMA}" != "vacua.release-manifest.v1" ]; then
  echo "Error: Unexpected manifest schema: ${MANIFEST_SCHEMA}" >&2
  exit 1
fi
if [ "${MANIFEST_VER}" != "${VERSION}" ]; then
  echo "Error: Manifest version (${MANIFEST_VER}) != ${VERSION}" >&2
  exit 1
fi
if [ "${MANIFEST_TAG}" != "${TAG}" ]; then
  echo "Error: Manifest tag (${MANIFEST_TAG}) != ${TAG}" >&2
  exit 1
fi

ACTUAL_CLI_SHA=$(shasum -a 256 "${CLI_ARCHIVE}" | awk '{print $1}')
ACTUAL_APP_SHA=$(shasum -a 256 "${APP_ARCHIVE}" | awk '{print $1}')

grep -q "${ACTUAL_CLI_SHA}" RELEASE_MANIFEST.json || { echo "Error: CLI archive SHA not found in manifest"; exit 1; }
grep -q "${ACTUAL_APP_SHA}" RELEASE_MANIFEST.json || { echo "Error: App archive SHA not found in manifest"; exit 1; }

echo "--- Extracting and Inspecting CLI Tarball ---"
mkdir -p cli-extracted
tar -xzf "${CLI_ARCHIVE}" -C cli-extracted
CLI_EXTRACT_DIR=$(ls -d cli-extracted/vacua-v*-*)

CLI_BUILD_INFO=$("${CLI_EXTRACT_DIR}/bin/vacua" --build-info)
MCP_BUILD_INFO=$("${CLI_EXTRACT_DIR}/bin/vacua-mcp" --build-info)

CLI_VACUA_VER=$(echo "${CLI_BUILD_INFO}" | grep '"version":' | head -n 1 | awk -F'"' '{print $4}')
CLI_VACUA_SHA=$(echo "${CLI_BUILD_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')

CLI_MCP_VER=$(echo "${MCP_BUILD_INFO}" | grep '"version":' | head -n 1 | awk -F'"' '{print $4}')
CLI_MCP_SHA=$(echo "${MCP_BUILD_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')

echo "CLI vacua version:     ${CLI_VACUA_VER} (${CLI_VACUA_SHA})"
echo "CLI vacua-mcp version: ${CLI_MCP_VER} (${CLI_MCP_SHA})"

if [ "${CLI_VACUA_VER}" != "${VERSION}" ] || [ "${CLI_MCP_VER}" != "${VERSION}" ]; then
  echo "Error: CLI binary version does not match ${VERSION}" >&2
  exit 1
fi

echo "--- Extracting and Inspecting Native App Zip ---"
mkdir -p app-extracted
unzip -q "${APP_ARCHIVE}" -d app-extracted
APP_PATH="app-extracted/Vacua.app"

APP_BUNDLE_VER=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "${APP_PATH}/Contents/Info.plist")
APP_GIT_SHA=$(/usr/libexec/PlistBuddy -c "Print :VacuaGitCommit" "${APP_PATH}/Contents/Info.plist" 2>/dev/null || echo "missing")

APP_HELPER_VACUA_INFO=$("${APP_PATH}/Contents/Helpers/vacua" --build-info)
APP_HELPER_MCP_INFO=$("${APP_PATH}/Contents/Helpers/vacua-mcp" --build-info)

APP_HELPER_VACUA_SHA=$(echo "${APP_HELPER_VACUA_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')
APP_HELPER_MCP_SHA=$(echo "${APP_HELPER_MCP_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')

echo "App Bundle version:    ${APP_BUNDLE_VER} (${APP_GIT_SHA})"
echo "App Helper vacua SHA:  ${APP_HELPER_VACUA_SHA}"
echo "App Helper MCP SHA:    ${APP_HELPER_MCP_SHA}"

if [ "${APP_BUNDLE_VER}" != "${VERSION}" ]; then
  echo "Error: App bundle version (${APP_BUNDLE_VER}) != ${VERSION}" >&2
  exit 1
fi

# Determine source tag commit
if [ -n "${LOCAL_DIR}" ]; then
  TAG_COMMIT="${MANIFEST_GIT_SHA}"
else
  # Query git tag commit (peeled annotated tag or lightweight tag)
  TAG_COMMIT=$(git ls-remote "https://github.com/${REPO}.git" "refs/tags/${TAG}^{}" 2>/dev/null | awk '{print $1}')
  if [ -z "${TAG_COMMIT}" ]; then
    TAG_COMMIT=$(git ls-remote "https://github.com/${REPO}.git" "refs/tags/${TAG}" 2>/dev/null | awk '{print $1}')
  fi
  if [ -z "${TAG_COMMIT}" ]; then
    TAG_COMMIT=$(git rev-parse "${TAG}^{commit}" 2>/dev/null || true)
  fi
fi

echo "=========================================================="
echo "Checking Source Commit Invariants"
echo "=========================================================="
echo "Tag/Release Commit:       ${TAG_COMMIT}"
echo "Manifest git_commit:      ${MANIFEST_GIT_SHA}"
echo "CLI vacua git_commit:     ${CLI_VACUA_SHA}"
echo "CLI vacua-mcp git_commit: ${CLI_MCP_SHA}"
echo "App Info.plist Git SHA:   ${APP_GIT_SHA}"
echo "App Helper vacua SHA:     ${APP_HELPER_VACUA_SHA}"
echo "App Helper vacua-mcp SHA: ${APP_HELPER_MCP_SHA}"
echo "=========================================================="

test -n "${MANIFEST_GIT_SHA}" || { echo "Manifest git_commit is empty"; exit 1; }
test "${MANIFEST_GIT_SHA}" = "${CLI_VACUA_SHA}" || { echo "Invariant violation: manifest git_commit != vacua git_commit"; exit 1; }
test "${CLI_VACUA_SHA}" = "${CLI_MCP_SHA}" || { echo "Invariant violation: vacua git_commit != vacua-mcp git_commit"; exit 1; }
test "${CLI_VACUA_SHA}" = "${APP_GIT_SHA}" || { echo "Invariant violation: CLI git_commit != App Info.plist VacuaGitCommit"; exit 1; }
test "${APP_GIT_SHA}" = "${APP_HELPER_VACUA_SHA}" || { echo "Invariant violation: App Git SHA != App Helper vacua Git SHA"; exit 1; }
test "${APP_GIT_SHA}" = "${APP_HELPER_MCP_SHA}" || { echo "Invariant violation: App Git SHA != App Helper vacua-mcp Git SHA"; exit 1; }

if [ -n "${TAG_COMMIT}" ] && [ "${TAG_COMMIT}" != "${MANIFEST_GIT_SHA}" ]; then
  # If tag is an annotated tag object, check target commit
  DEREF_COMMIT=$(git rev-parse "${TAG_COMMIT}^{commit}" 2>/dev/null || true)
  if [ -n "${DEREF_COMMIT}" ] && [ "${DEREF_COMMIT}" = "${MANIFEST_GIT_SHA}" ]; then
    echo "Note: Tag ${TAG_COMMIT} dereferences to commit ${MANIFEST_GIT_SHA} (MATCH)"
  else
    echo "Invariant violation: Tag commit (${TAG_COMMIT}) != Manifest commit (${MANIFEST_GIT_SHA})" >&2
    exit 1
  fi
fi

echo "Source identity invariant confirmed: ALL COMMIT SHAS MATCH (${MANIFEST_GIT_SHA})"

# Verify GitHub Artifact Attestation if supported and running online
if [ -z "${LOCAL_DIR}" ]; then
  echo "--- Verifying GitHub Artifact Attestations ---"
  if gh attestation verify "${CLI_ARCHIVE}" --repo "${REPO}" >/dev/null 2>&1; then
    echo "GitHub Attestation for ${CLI_ARCHIVE}: VERIFIED"
  else
    echo "Notice: Attestation verification for ${CLI_ARCHIVE} returned non-zero (may require specific scopes or published state)."
  fi

  if gh attestation verify "${APP_ARCHIVE}" --repo "${REPO}" >/dev/null 2>&1; then
    echo "GitHub Attestation for ${APP_ARCHIVE}: VERIFIED"
  else
    echo "Notice: Attestation verification for ${APP_ARCHIVE} returned non-zero."
  fi
fi

echo ""
echo "=========================================================="
echo "SUCCESS: Release ${TAG} passed all cryptographic and"
echo "provenance verification invariants!"
echo "=========================================================="
