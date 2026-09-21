class MovieboxTui < Formula
  VERSION = "0.1.21"
  MACOS_SHA256 = "572ea16b9ac0cdc94d4848bf04a9c3e6732c85edc6400b52dd4da8b30ba11ff3"
  LINUX_X64_SHA256 = "c6a09407713f559c3e88e52ff7ef7a3a1f1c29ee092a66c5e7d5514ec8f93f81"
  LINUX_ARM64_SHA256 = "012a2a070d9968a31710d41d94f54dac756f1ad9f4653c0422e72596b7f85919"

  desc "Stream movies, shows, anime, and live TV from your terminal"
  homepage "https://github.com/mesamirh/MovieBox-Tui"
  version VERSION
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    url "https://github.com/mesamirh/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_macOS_Universal.tar.gz"
    sha256 MACOS_SHA256
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/mesamirh/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_Linux_arm64.tar.gz"
      sha256 LINUX_ARM64_SHA256
    else
      url "https://github.com/mesamirh/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_Linux_x64.tar.gz"
      sha256 LINUX_X64_SHA256
    end
  end

  def install
    bin.install "moviebox-tui"
  end

  test do
    system "#{bin}/moviebox-tui", "--version"
  end
end
