#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== 1. Building release binary ==="
cargo build --release --bin vacua --manifest-path "${REPO_ROOT}/Cargo.toml"

VACUA_BIN="${REPO_ROOT}/target/release/vacua"

echo "=== 2. Generating synthetic demo fixture ==="
"${SCRIPT_DIR}/generate-demo-fixture.sh" >/dev/null

echo "=== 3. Executing deterministic demonstration ==="
DEMO_RAW="${REPO_ROOT}/fixtures/demo-output-raw.txt"
DEMO_OUT="${REPO_ROOT}/fixtures/demo-output.txt"
rm -f "${DEMO_RAW}" "${DEMO_OUT}"

CANDIDATE_ID=$("${VACUA_BIN}" --json candidates "${REPO_ROOT}/fixtures/demo-home" --risk safe | python3 -c "
import sys, json
data = json.load(sys.stdin)
for item in data:
    if 'node--22.2.0' in item.get('path', ''):
        print(item['id'])
        break
else:
    if data:
        print(data[0]['id'])
")

{
  echo "$ vacua scan ~/demo-home"
  "${VACUA_BIN}" scan "${REPO_ROOT}/fixtures/demo-home"
  echo ""

  echo "$ vacua candidates ~/demo-home --risk safe"
  "${VACUA_BIN}" candidates "${REPO_ROOT}/fixtures/demo-home" --risk safe
  echo ""

  echo "$ vacua explain ${CANDIDATE_ID} --path ~/demo-home"
  "${VACUA_BIN}" explain --path "${REPO_ROOT}/fixtures/demo-home" "${CANDIDATE_ID}"
  echo ""

  echo "$ vacua plan ~/demo-home -o plan.json"
  "${VACUA_BIN}" plan "${REPO_ROOT}/fixtures/demo-home" -o "${REPO_ROOT}/fixtures/demo-plan.json"
  echo ""

  echo "$ vacua execute plan.json --dry-run"
  "${VACUA_BIN}" execute "${REPO_ROOT}/fixtures/demo-plan.json" --dry-run
} > "${DEMO_RAW}"

# Sanitize local path prefixes to keep demo output portable and leak-free
sed -e "s|${REPO_ROOT}/fixtures/demo-home|~/demo-home|g" \
    -e "s|${REPO_ROOT}/fixtures/demo-plan.json|plan.json|g" \
    "${DEMO_RAW}" > "${DEMO_OUT}"
rm -f "${DEMO_RAW}" "${REPO_ROOT}/fixtures/demo-plan.json"

echo "=== Demo output refreshed successfully (${DEMO_OUT}) ==="
cat "${DEMO_OUT}"
