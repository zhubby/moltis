#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SWIFT_POC_DIR="${REPO_ROOT}/examples/swift-poc"

if [ ! -d "${SWIFT_POC_DIR}/Moltis.xcodeproj" ]; then
  echo "error: Moltis.xcodeproj does not exist. Run: just swift-poc-generate" >&2
  exit 1
fi

if [ -z "${DEVELOPER_DIR:-}" ] && [ -d "/Applications/Xcode.app/Contents/Developer" ]; then
  export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
fi

xcodebuild \
  -project "${SWIFT_POC_DIR}/Moltis.xcodeproj" \
  -scheme Moltis \
  -destination "platform=macOS" \
  test

echo "xcodebuild tests succeeded for Moltis"
