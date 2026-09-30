class Vacua < Formula
  desc "Explainable, safety-first storage intelligence for macOS"
  homepage "https://github.com/yuanweize/vacua"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yuanweize/vacua/releases/download/v0.8.0/vacua-v0.8.0-aarch64-apple-darwin.tar.gz"
      sha256 "077f7137c1119e7928d4f2e1f711f10331a3babd065cf9f25d1b0b451b353aea"
    end
  end

  def install
    bin.install "bin/vacua"
    bin.install "bin/vacua-intelligence"
    bin.install "bin/vacua-mcp"
    bash_completion.install "share/bash-completion/completions/vacua"
    zsh_completion.install "share/zsh/site-functions/_vacua"
    fish_completion.install "share/fish/vendor_completions.d/vacua.fish"
  end

  test do
    assert_match "vacua #{version}", shell_output("#{bin}/vacua --version")
    assert_match "vacua-intelligence #{version}", shell_output("#{bin}/vacua-intelligence --version")
    assert_match "vacua-mcp #{version}", shell_output("#{bin}/vacua-mcp --version")
    assert_match "Vacua MCP Server Self-Test: OK", shell_output("#{bin}/vacua-mcp --self-test 2>&1")
  end
end
