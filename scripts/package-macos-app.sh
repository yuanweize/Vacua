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

# 2. Create Zip Archive (preserving symlinks and permissions)
ZIP_NAME="Vacua-v${VERSION}-macos-arm64-unsigned.zip"
ZIP_PATH="${DIST_DIR}/${ZIP_NAME}"
rm -f "${ZIP_PATH}"

echo "--- Compressing Vacua.app into ${ZIP_NAME} ---"
(
  cd "${REPO_ROOT}/apps/macos/build"
  ditto -c -k --keepParent "Vacua.app" "${ZIP_PATH}"
)

# 3. Generate SHA256 Checksum
echo "--- Generating Checksum ---"
(
  cd "${DIST_DIR}"
  shasum -a 256 "${ZIP_NAME}" > "${ZIP_NAME}.sha256"
)

echo "=== App package created successfully ==="
ls -lh "${DIST_DIR}/${ZIP_NAME}"*
cat "${DIST_DIR}/${ZIP_NAME}.sha256"
