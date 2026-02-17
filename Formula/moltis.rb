class Moltis < Formula
  desc "Rust-powered bot framework with LLM agents, plugins, and gateway"
  homepage "https://github.com/moltis-org/moltis"
  url "https://github.com/moltis-org/moltis.git",
      tag:      "v0.1.0",
      revision: ""
  license "MIT"
  head "https://github.com/moltis-org/moltis.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--manifest-path", "crates/cli/Cargo.toml",
                    "--no-default-features",
                    "--features",
                    "file-watcher,local-llm,metrics,prometheus,push-notifications,qmd,tailscale,tls,voice,web-ui"
    libexec.install "target/release/moltis"
    pkgshare.install "crates/gateway/src/assets" => "assets"
    (bin/"moltis").write_env_script libexec/"moltis", MOLTIS_ASSETS_DIR: "#{pkgshare}/assets"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/moltis --version", 2)
  end
end
