#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SWIFT_POC_DIR="${REPO_ROOT}/examples/swift-poc"

if ! command -v xcodegen >/dev/null 2>&1; then
  echo "error: xcodegen is required (install with: brew install xcodegen)" >&2
  exit 1
fi

cd "${SWIFT_POC_DIR}"
xcodegen generate --spec project.yml

echo "Generated ${SWIFT_POC_DIR}/Moltis.xcodeproj"
