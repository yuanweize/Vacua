#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"

echo "=========================================================="
echo "Checking Project License Alignment (Apache-2.0)"
echo "=========================================================="

# 1. Check canonical LICENSE existence and legacy deletion
if [ ! -f "LICENSE" ]; then
  echo "FAIL: Canonical LICENSE file is missing!" >&2
  exit 1
fi

if [ -f "LICENSE-MIT" ] || [ -f "LICENSE-APACHE" ]; then
  echo "FAIL: Legacy dual-license files (LICENSE-MIT or LICENSE-APACHE) still exist!" >&2
  exit 1
fi
echo "PASS: Canonical LICENSE exists; legacy license files removed."

# 2. Check Cargo.toml workspace license
WORKSPACE_LIC=$(grep -m 1 '^license = ' Cargo.toml | cut -d '"' -f 2)
if [ "${WORKSPACE_LIC}" != "Apache-2.0" ]; then
  echo "FAIL: Cargo.toml workspace license is '${WORKSPACE_LIC}', expected 'Apache-2.0'!" >&2
  exit 1
fi
echo "PASS: Cargo.toml workspace license is Apache-2.0."

# 3. Check Homebrew formula template
if ! grep -q 'license "Apache-2.0"' packaging/homebrew/vacua.rb.template; then
  echo "FAIL: packaging/homebrew/vacua.rb.template does not declare license \"Apache-2.0\"!" >&2
  exit 1
fi
echo "PASS: Homebrew formula template declares Apache-2.0."

# 4. Check for forbidden dual-license strings in project files
FORBIDDEN_PATTERNS=(
  'MIT OR Apache-2.0'
  'license all_of: \["MIT", "Apache-2.0"\]'
  'LICENSE-MIT'
  'LICENSE-APACHE'
)

for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
  # Search project code/config/scripts, excluding docs/adr/ and qualification reports discussing historical state
  if git grep -E "${pattern}" -- \
      'Cargo.toml' \
      'crates/' \
      'apps/' \
      'apple/' \
      'packaging/' \
      'scripts/' \
      'README.md' \
      ':(exclude)scripts/check-license-alignment.sh' >/dev/null 2>&1; then
    echo "FAIL: Forbidden legacy license pattern '${pattern}' found in project metadata or code!" >&2
    git grep -n -E "${pattern}" -- \
      'Cargo.toml' \
      'crates/' \
      'apps/' \
      'apple/' \
      'packaging/' \
      'scripts/' \
      'README.md' \
      ':(exclude)scripts/check-license-alignment.sh' >&2
    exit 1
  fi
done
echo "PASS: No forbidden dual-license patterns found in active project sources."

# 5. Check cargo-deny if available
if command -v cargo-deny >/dev/null 2>&1; then
  echo "--- Running cargo-deny license check ---"
  cargo deny check licenses
  echo "PASS: cargo-deny license policy verified."
else
  echo "NOTE: cargo-deny not installed on this system; skipped live dependency license tree scan."
fi

echo "=========================================================="
echo "ALL LICENSE ALIGNMENT CHECKS PASSED"
echo "=========================================================="
