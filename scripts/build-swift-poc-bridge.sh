#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BRIDGE_CRATE_DIR="${REPO_ROOT}/crates/swift-bridge"
OUTPUT_DIR="${REPO_ROOT}/examples/swift-poc/Generated"
UNIVERSAL_DIR="${REPO_ROOT}/target/universal-macos/release"

if ! command -v cbindgen >/dev/null 2>&1; then
  echo "error: cbindgen is required (install with: cargo install cbindgen)" >&2
  exit 1
fi

if ! command -v lipo >/dev/null 2>&1; then
  echo "error: lipo is required (install Xcode command line tools)" >&2
  exit 1
fi

rustup target add x86_64-apple-darwin aarch64-apple-darwin

cargo build -p moltis-swift-bridge --release --target x86_64-apple-darwin
cargo build -p moltis-swift-bridge --release --target aarch64-apple-darwin

mkdir -p "${UNIVERSAL_DIR}" "${OUTPUT_DIR}"

lipo -create \
  "${REPO_ROOT}/target/x86_64-apple-darwin/release/libmoltis_swift_bridge.a" \
  "${REPO_ROOT}/target/aarch64-apple-darwin/release/libmoltis_swift_bridge.a" \
  -output "${UNIVERSAL_DIR}/libmoltis_bridge.a"

cbindgen "${BRIDGE_CRATE_DIR}" \
  --config "${BRIDGE_CRATE_DIR}/cbindgen.toml" \
  --crate moltis-swift-bridge \
  --output "${OUTPUT_DIR}/moltis_bridge.h"

cp "${UNIVERSAL_DIR}/libmoltis_bridge.a" "${OUTPUT_DIR}/libmoltis_bridge.a"

echo "Built Rust bridge artifacts in ${OUTPUT_DIR}"
