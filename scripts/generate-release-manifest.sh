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
TARGET="aarch64-apple-darwin"
CLI_ARCHIVE="vacua-v${VERSION}-${TARGET}.tar.gz"
APP_ARCHIVE="Vacua-v${VERSION}-macos-arm64-unsigned.zip"

echo "=== Generating Release Manifest for v${VERSION} ==="

if [ ! -f "${DIST_DIR}/${CLI_ARCHIVE}" ]; then
  echo "Error: CLI archive not found at ${DIST_DIR}/${CLI_ARCHIVE}" >&2
  exit 1
fi

if [ ! -f "${DIST_DIR}/${APP_ARCHIVE}" ]; then
  echo "Error: App archive not found at ${DIST_DIR}/${APP_ARCHIVE}" >&2
  exit 1
fi

CLI_SHA=$(shasum -a 256 "${DIST_DIR}/${CLI_ARCHIVE}" | awk '{print $1}')
APP_SHA=$(shasum -a 256 "${DIST_DIR}/${APP_ARCHIVE}" | awk '{print $1}')

GIT_SHA="${VACUA_GIT_SHA:-$(git -C "${REPO_ROOT}" rev-parse HEAD 2>/dev/null || echo "unknown")}"
RUSTC_VER=$(rustc --version 2>/dev/null | head -n 1 | tr -d '\r\n')
SWIFT_VER=$(swift --version 2>/dev/null | head -n 1 | tr -d '\r\n')
if command -v xcodebuild >/dev/null 2>&1; then
  XCODE_VER=$(xcodebuild -version 2>/dev/null | head -n 1 | tr -d '\r\n')
else
  XCODE_VER="unknown"
fi
RUN_ID="${GITHUB_RUN_ID:-}"

MANIFEST_PATH="${DIST_DIR}/RELEASE_MANIFEST.json"

cat <<EOF > "${MANIFEST_PATH}"
{
  "schema_version": "vacua.release-manifest.v1",
  "version": "${VERSION}",
  "tag": "v${VERSION}",
  "git_commit": "${GIT_SHA}",
  "repository": "yuanweize/vacua",
  "target": "${TARGET}",
  "artifacts": [
    {
      "name": "${CLI_ARCHIVE}",
      "sha256": "${CLI_SHA}"
    },
    {
      "name": "${APP_ARCHIVE}",
      "sha256": "${APP_SHA}"
    }
  ],
  "rustc_version": "${RUSTC_VER}",
  "swift_version": "${SWIFT_VER}",
  "xcode_version": "${XCODE_VER}"$(if [ -n "${RUN_ID}" ]; then echo ","; echo "  \"github_run_id\": \"${RUN_ID}\""; else echo ""; fi)
}
EOF

# Calculate checksum of RELEASE_MANIFEST.json and generate master SHA256SUMS
MANIFEST_SHA=$(shasum -a 256 "${MANIFEST_PATH}" | awk '{print $1}')

cat <<EOF > "${DIST_DIR}/SHA256SUMS"
${CLI_SHA}  ${CLI_ARCHIVE}
${APP_SHA}  ${APP_ARCHIVE}
${MANIFEST_SHA}  RELEASE_MANIFEST.json
EOF

echo "Generated ${MANIFEST_PATH}:"
cat "${MANIFEST_PATH}"
echo ""
echo "Generated ${DIST_DIR}/SHA256SUMS:"
cat "${DIST_DIR}/SHA256SUMS"
