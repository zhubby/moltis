#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SWIFT_POC_DIR="${REPO_ROOT}/examples/swift-poc"
DERIVED_DATA_DIR="${SWIFT_POC_DIR}/.derivedData"
APP_PATH="${DERIVED_DATA_DIR}/Build/Products/Debug/Moltis.app"

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
  -derivedDataPath "${DERIVED_DATA_DIR}" \
  build

if [ ! -d "${APP_PATH}" ]; then
  echo "error: expected app bundle not found at ${APP_PATH}" >&2
  exit 1
fi

open "${APP_PATH}"

echo "Launched ${APP_PATH}"
