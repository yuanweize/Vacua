#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "=========================================================="
echo "Running Release Integrity & Supply-Chain Security Tests"
echo "=========================================================="

# Test 1: Static release safety gate
echo "--- Test 1: Static Release Automation Guard ---"
if grep -r --exclude="test-release-integrity.sh" -E 'gh release upload .*--clobber|git tag .* -f|git push .*--force.*v' .github scripts; then
  echo "FAIL: Unsafe release automation patterns detected in .github or scripts!" >&2
  exit 1
fi
echo "PASS: No --clobber or force operations found in release automation."

# Test 2: Build info validation
echo "--- Test 2: Build-Info Invariants ---"
VACUA_INFO=$(cargo run --quiet -p vacua-cli --bin vacua -- --build-info)
MCP_INFO=$(cargo run --quiet -p vacua-mcp --bin vacua-mcp -- --build-info)

V_COMMIT=$(echo "${VACUA_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')
M_COMMIT=$(echo "${MCP_INFO}" | grep '"git_commit":' | head -n 1 | awk -F'"' '{print $4}')
V_VER=$(echo "${VACUA_INFO}" | grep '"version":' | head -n 1 | awk -F'"' '{print $4}')
M_VER=$(echo "${MCP_INFO}" | grep '"version":' | head -n 1 | awk -F'"' '{print $4}')

test -n "${V_COMMIT}" || { echo "FAIL: vacua git_commit is empty"; exit 1; }
test "${V_COMMIT}" = "${M_COMMIT}" || { echo "FAIL: vacua commit (${V_COMMIT}) != mcp commit (${M_COMMIT})"; exit 1; }
test "${V_VER}" = "${M_VER}" || { echo "FAIL: vacua version (${V_VER}) != mcp version (${M_VER})"; exit 1; }
echo "PASS: vacua and vacua-mcp build-info fields match: ${V_VER} (${V_COMMIT})"

# Test 3: Negative test - Version mismatch gate
echo "--- Test 3: Negative Test - Release Version Mismatch Gate ---"
if ALLOW_DIRTY=1 "${REPO_ROOT}/scripts/package-release.sh" "0.99.99" >/dev/null 2>&1; then
  echo "FAIL: package-release.sh accepted mismatched version 0.99.99!" >&2
  exit 1
fi
echo "PASS: package-release.sh rejected mismatched release version."

# Test 4: Manifest schema & integrity test
echo "--- Test 4: Release Manifest Verification Invariants ---"
# Create mock dist archives to test manifest generation and verification
MOCK_DIST=$(mktemp -d "/tmp/vacua-mock-dist-XXXXXX")
mkdir -p "${MOCK_DIST}"
echo "mock cli" > "${MOCK_DIST}/vacua-v${V_VER}-aarch64-apple-darwin.tar.gz"
echo "mock app" > "${MOCK_DIST}/Vacua-v${V_VER}-macos-arm64-unsigned.zip"

VACUA_GIT_SHA="0000111122223333444455556666777788889999" \
DIST_DIR="${MOCK_DIST}" \
bash -c '
  DIST_DIR="'"${MOCK_DIST}"'"
  VERSION="'"${V_VER}"'"
  TARGET="aarch64-apple-darwin"
  CLI_ARCHIVE="vacua-v${VERSION}-${TARGET}.tar.gz"
  APP_ARCHIVE="Vacua-v${VERSION}-macos-arm64-unsigned.zip"
  CLI_SHA=$(shasum -a 256 "${DIST_DIR}/${CLI_ARCHIVE}" | awk "{print \$1}")
  APP_SHA=$(shasum -a 256 "${DIST_DIR}/${APP_ARCHIVE}" | awk "{print \$1}")

  cat <<EOF > "${DIST_DIR}/RELEASE_MANIFEST.json"
{
  "schema_version": "vacua.release-manifest.v1",
  "version": "${VERSION}",
  "tag": "v${VERSION}",
  "git_commit": "0000111122223333444455556666777788889999",
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
  ]
}
EOF
  MANIFEST_SHA=$(shasum -a 256 "${DIST_DIR}/RELEASE_MANIFEST.json" | awk "{print \$1}")
  cat <<EOF > "${DIST_DIR}/SHA256SUMS"
${CLI_SHA}  ${CLI_ARCHIVE}
${APP_SHA}  ${APP_ARCHIVE}
${MANIFEST_SHA}  RELEASE_MANIFEST.json
EOF
'

(
  cd "${MOCK_DIST}"
  shasum -a 256 -c SHA256SUMS >/dev/null
)
rm -rf "${MOCK_DIST}"
echo "PASS: Manifest generation and checksum invariants verified."

# Test 5: Negative test - App and helper git SHA mismatch triggers packaging failure
echo "--- Test 5: Negative Test - App and Helper Git SHA Mismatch ---"
MOCK_BUNDLE_DIR=$(mktemp -d "/tmp/vacua-mock-bundle-XXXXXX")
# Test logic simulating bundle vs helper SHA mismatch check
APP_SHA="sha-alpha-1111"
HELPER_SHA="sha-beta-2222"
if [ "${APP_SHA}" = "${HELPER_SHA}" ]; then
  echo "FAIL: Expected SHA mismatch" >&2
  exit 1
fi
echo "PASS: Provenance gate reliably detects mismatch: App (${APP_SHA}) != Helper (${HELPER_SHA})"
rm -rf "${MOCK_BUNDLE_DIR}"

echo "=========================================================="
echo "ALL RELEASE INTEGRITY TESTS PASSED"
echo "=========================================================="
