class MovieboxTui < Formula
  VERSION = "0.1.20"
  MACOS_SHA256 = "0ecfb5542b8207c6d5c6c1d8f804a7925e395bc96023c647225189f0c5b54147"
  LINUX_X64_SHA256 = "d13b9e574446bef110c195c435d118c1f192657dec0fecefaa9ea2d3ae9b70a4"
  LINUX_ARM64_SHA256 = "614a9c7de50d7081ace296f9ffc8286568f62554ddabc72199be4b184008246b"

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
