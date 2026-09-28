class Vacua < Formula
  desc "Explainable, safety-first storage intelligence for macOS"
  homepage "https://github.com/yuanweize/vacua"
  license all_of: ["MIT", "Apache-2.0"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yuanweize/vacua/releases/download/v0.2.0/vacua-v0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "ca5423606dea17b40d6840670f25a1296365e8a6e4c66067c8e2697a33fc683c"
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
