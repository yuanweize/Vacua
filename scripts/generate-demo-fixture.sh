#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
FIXTURE_DIR="${REPO_ROOT}/fixtures/demo-home"

echo "Creating synthetic demo fixture at ${FIXTURE_DIR}..."
rm -rf "${FIXTURE_DIR}"
mkdir -p "${FIXTURE_DIR}"

# 1. Xcode DerivedData (Safe - reconstructable build artifact)
DERIVED_DATA="${FIXTURE_DIR}/Library/Developer/Xcode/DerivedData/VacuaApp-bwefnxkzps"
mkdir -p "${DERIVED_DATA}/Build/Products/Debug"
mkdir -p "${DERIVED_DATA}/Index.noindex/DataStore"
dd if=/dev/zero of="${DERIVED_DATA}/Build/Products/Debug/VacuaApp.o" bs=1024 count=1024 2>/dev/null || dd if=/dev/zero of="${DERIVED_DATA}/Build/Products/Debug/VacuaApp.o" bs=1k count=1024 2>/dev/null
dd if=/dev/zero of="${DERIVED_DATA}/Index.noindex/DataStore/v5.idx" bs=1024 count=512 2>/dev/null || dd if=/dev/zero of="${DERIVED_DATA}/Index.noindex/DataStore/v5.idx" bs=1k count=512 2>/dev/null

# 2. Homebrew Cache (Safe - reconstructable package download)
BREW_CACHE="${FIXTURE_DIR}/Library/Caches/Homebrew/downloads"
mkdir -p "${BREW_CACHE}"
dd if=/dev/zero of="${BREW_CACHE}/node--22.2.0.arm64_sequoia.bottle.tar.gz" bs=1024 count=2048 2>/dev/null || dd if=/dev/zero of="${BREW_CACHE}/node--22.2.0.arm64_sequoia.bottle.tar.gz" bs=1k count=2048 2>/dev/null

# 3. Cargo Target directory (Review - developer build artifact)
CARGO_TARGET="${FIXTURE_DIR}/Projects/web-service/target"
mkdir -p "${CARGO_TARGET}/debug/incremental"
dd if=/dev/zero of="${CARGO_TARGET}/debug/web-service" bs=1024 count=4096 2>/dev/null || dd if=/dev/zero of="${CARGO_TARGET}/debug/web-service" bs=1k count=4096 2>/dev/null

# 4. Protected User Credentials (.ssh)
SSH_DIR="${FIXTURE_DIR}/.ssh"
mkdir -p "${SSH_DIR}"
chmod 700 "${SSH_DIR}"
echo "-----BEGIN OPENSSH PRIVATE KEY-----" > "${SSH_DIR}/id_ed25519"
echo "b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW" >> "${SSH_DIR}/id_ed25519"
echo "-----END OPENSSH PRIVATE KEY-----" >> "${SSH_DIR}/id_ed25519"
chmod 600 "${SSH_DIR}/id_ed25519"

# 5. Protected User Photo Library
PHOTOS_DIR="${FIXTURE_DIR}/Pictures/Photos.photoslibrary"
mkdir -p "${PHOTOS_DIR}/database"
echo "photos-metadata" > "${PHOTOS_DIR}/database/Photos.sqlite"

# 6. Unknown User Document (Never touched)
DOCS_DIR="${FIXTURE_DIR}/Documents"
mkdir -p "${DOCS_DIR}"
echo "Vacua quarterly strategy notes and internal planning" > "${DOCS_DIR}/Strategy_2026.pdf"

echo "Demo fixture generated successfully:"
find "${FIXTURE_DIR}" -type f -o -type d | sort
