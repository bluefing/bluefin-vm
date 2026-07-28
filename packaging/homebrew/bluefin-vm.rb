# Homebrew formula for the bluefin-vm tool. Canonical copy lives here; it is
# published to the tap at github.com/bluefing/homebrew-tap as
# Formula/bluefin-vm.rb (install: `brew install bluefing/tap/bluefin-vm`).
#
# It ships the TOOL only -- a prebuilt arm64 binary from a GitHub Release, built
# by .github/workflows/release.yml. The installed tool downloads the VM seed at
# runtime, so the seed's hosting stays independent of this formula. Bump
# `version`, `url`, and `sha256` per release; release.yml prints the exact url +
# sha256 to paste in.
class BluefinVm < Formula
  desc "Download, import, and run a Bluefin VM on Apple Silicon"
  homepage "https://github.com/bluefing/bluefin-vm"
  url "https://github.com/bluefing/bluefin-vm/releases/download/v0.1.0/bluefin-vm-0.1.0-aarch64-apple-darwin.tar.gz"
  version "0.1.0"
  # Placeholder until the first release is tagged; release.yml emits the real
  # value. `brew fetch` verifies the download against this.
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "Apache-2.0"

  # Apple Silicon only: the tool drives Apple's Virtualisation framework, and
  # the release ships an arm64 binary.
  depends_on arch: :arm64
  # Imports and runs the VM by shelling out to tart, so brew must pull it in.
  depends_on "openai/tools/tart"

  def install
    bin.install "bluefin-vm"
  end

  test do
    assert_match "bluefin-vm #{version}", shell_output("#{bin}/bluefin-vm --version")
  end
end
