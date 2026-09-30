#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "=========================================================="
echo "Checking Authoritative Version Alignment Across Subsystems"
echo "=========================================================="

# 1. Authoritative primary version from Cargo.toml
EXPECTED_VER=$(grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f 2)
echo "Primary Workspace Version: ${EXPECTED_VER}"

ERRORS=0

fail() {
  echo "FAIL: $1" >&2
  ERRORS=$((ERRORS + 1))
}

# 2. Check Cargo.lock workspace member versions
CARGO_PKG_VER=$(cargo metadata --format-version 1 --no-deps | python3 -c '
import sys, json
data = json.load(sys.stdin)
for pkg in data["packages"]:
    name = pkg["name"]
    ver = pkg["version"]
    print(f"{name}:{ver}")
')

while IFS=':' read -r name ver; do
  if [ "${ver}" != "${EXPECTED_VER}" ]; then
    fail "Crate ${name} has version ${ver}, expected ${EXPECTED_VER}"
  fi
done <<< "${CARGO_PKG_VER}"
echo "PASS: All workspace Cargo crates match ${EXPECTED_VER}."

# 3. Check VacuaIntelligenceCLI.swift
SWIFT_CLI_FILE="apple/VacuaIntelligence/Sources/VacuaIntelligence/VacuaIntelligenceCLI.swift"
if [ -f "${SWIFT_CLI_FILE}" ]; then
  SWIFT_CLI_VER=$(grep 'static let version =' "${SWIFT_CLI_FILE}" | head -n 1 | cut -d '"' -f 2)
  if [ "${SWIFT_CLI_VER}" != "${EXPECTED_VER}" ]; then
    fail "VacuaIntelligenceCLI.swift declared version '${SWIFT_CLI_VER}', expected '${EXPECTED_VER}'"
  else
    echo "PASS: VacuaIntelligenceCLI version matches ${EXPECTED_VER}."
  fi
else
  fail "Missing ${SWIFT_CLI_FILE}"
fi

# 4. Check Vacua.app Info.plist CFBundleShortVersionString
INFO_PLIST="apps/macos/Vacua/Resources/Info.plist"
if [ -f "${INFO_PLIST}" ]; then
  PLIST_VER=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "${INFO_PLIST}" 2>/dev/null || echo "")
  if [ "${PLIST_VER}" != "${EXPECTED_VER}" ]; then
    fail "Info.plist CFBundleShortVersionString is '${PLIST_VER}', expected '${EXPECTED_VER}'"
  else
    echo "PASS: Vacua.app Info.plist matches ${EXPECTED_VER}."
  fi
else
  fail "Missing ${INFO_PLIST}"
fi

# 5. Check Xcode Project MARKETING_VERSION
PBXPROJ="apps/macos/Vacua.xcodeproj/project.pbxproj"
if [ -f "${PBXPROJ}" ]; then
  PBX_VERS=$(grep 'MARKETING_VERSION =' "${PBXPROJ}" | awk '{print $3}' | tr -d ';' | sort -u)
  for v in ${PBX_VERS}; do
    if [ "${v}" != "${EXPECTED_VER}" ]; then
      fail "Xcode project.pbxproj contains MARKETING_VERSION '${v}', expected '${EXPECTED_VER}'"
    fi
  done
  echo "PASS: Xcode project.pbxproj MARKETING_VERSION matches ${EXPECTED_VER}."
else
  fail "Missing ${PBXPROJ}"
fi

# 6. Check scripts/generate_xcodeproj.py template
GEN_PY="scripts/generate_xcodeproj.py"
if [ -f "${GEN_PY}" ]; then
  PY_VERS=$(grep 'MARKETING_VERSION = ' "${GEN_PY}" | awk '{print $3}' | tr -d ';' | sort -u)
  for v in ${PY_VERS}; do
    if [ "${v}" != "${EXPECTED_VER}" ]; then
      fail "scripts/generate_xcodeproj.py generates MARKETING_VERSION '${v}', expected '${EXPECTED_VER}'"
    fi
  done
  echo "PASS: scripts/generate_xcodeproj.py template matches ${EXPECTED_VER}."
else
  fail "Missing ${GEN_PY}"
fi

# 7. Check fixtures/api/capabilities-v1.json
CAP_FIXTURE="fixtures/api/capabilities-v1.json"
if [ -f "${CAP_FIXTURE}" ]; then
  FIXTURE_VER=$(python3 -c 'import json; print(json.load(open("'"${CAP_FIXTURE}"'"))["server_version"])')
  if [ "${FIXTURE_VER}" != "${EXPECTED_VER}" ]; then
    fail "${CAP_FIXTURE} declared server_version '${FIXTURE_VER}', expected '${EXPECTED_VER}'"
  else
    echo "PASS: ${CAP_FIXTURE} matches ${EXPECTED_VER}."
  fi
else
  fail "Missing ${CAP_FIXTURE}"
fi

# 8. Check VacuaClient tests and mock client
REAL_TEST="apps/macos/Packages/VacuaClient/Tests/VacuaClientTests/RealEngineIntegrationTests.swift"
if [ -f "${REAL_TEST}" ]; then
  TEST_VER=$(grep '#expect(caps.server_version ==' "${REAL_TEST}" | head -n 1 | cut -d '"' -f 2)
  if [ "${TEST_VER}" != "${EXPECTED_VER}" ]; then
    fail "${REAL_TEST} expected server_version '${TEST_VER}', expected '${EXPECTED_VER}'"
  else
    echo "PASS: RealEngineIntegrationTests expectation matches ${EXPECTED_VER}."
  fi
fi

CLIENT_FILE="apps/macos/Packages/VacuaClient/Sources/VacuaClient/Client/MCPVacuaEngineClient.swift"
if [ -f "${CLIENT_FILE}" ]; then
  CLIENT_VER=$(grep '"version":' "${CLIENT_FILE}" | head -n 1 | cut -d '"' -f 4)
  if [ "${CLIENT_VER}" != "${EXPECTED_VER}" ]; then
    fail "${CLIENT_FILE} protocol version '${CLIENT_VER}', expected '${EXPECTED_VER}'"
  else
    echo "PASS: MCPVacuaEngineClient protocol version matches ${EXPECTED_VER}."
  fi
fi

if [ "${ERRORS}" -gt 0 ]; then
  echo "=========================================================="
  echo "VERSION ALIGNMENT FAILED WITH ${ERRORS} MISMATCH(ES)"
  echo "=========================================================="
  exit 1
fi

echo "=========================================================="
echo "ALL AUTHORITATIVE VERSION DECLARATIONS ALIGNED (${EXPECTED_VER})"
echo "=========================================================="
