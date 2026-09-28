#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-0.1.0}"
TARBALL="${2:-dist/vacua-v${VERSION}-aarch64-apple-darwin.tar.gz}"

if [ ! -f "${TARBALL}" ]; then
  echo "Error: Release tarball not found at ${TARBALL}"
  echo "Run ./scripts/package-release.sh first."
  exit 1
fi

SHA256=$(shasum -a 256 "${TARBALL}" | awk '{print $1}')
URL="https://github.com/yuanweize/vacua/releases/download/v${VERSION}/vacua-v${VERSION}-aarch64-apple-darwin.tar.gz"

echo "=== Updating Homebrew Formula ==="
echo "Version: ${VERSION}"
echo "URL:     ${URL}"
echo "SHA256:  ${SHA256}"

FORMULA_DIR="Formula"
mkdir -p "${FORMULA_DIR}"

cat <<EOF > "${FORMULA_DIR}/vacua.rb"
class Vacua < Formula
  desc "Explainable, safety-first storage intelligence for macOS"
  homepage "https://github.com/yuanweize/vacua"
  version "${VERSION}"
  license all_of: ["MIT", "Apache-2.0"]

  on_macos do
    if Hardware::CPU.arm?
      url "${URL}"
      sha256 "${SHA256}"
    end
  end

  def install
    bin.install "bin/vacua"
    bin.install "bin/vacua-intelligence"
    bash_completion.install "share/bash-completion/completions/vacua"
    zsh_completion.install "share/zsh/site-functions/_vacua"
    fish_completion.install "share/fish/vendor_completions.d/vacua.fish"
  end

  test do
    assert_match "vacua #{version}", shell_output("#{bin}/vacua --version")
    assert_match "vacua-intelligence #{version}", shell_output("#{bin}/vacua-intelligence --version")
  end
end
EOF

echo "Generated ${FORMULA_DIR}/vacua.rb successfully."
cat "${FORMULA_DIR}/vacua.rb"
