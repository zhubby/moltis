# Moltis Swift POC

This proof-of-concept embeds Moltis Rust code into a native macOS SwiftUI app.

On first launch, the app opens an AppKit onboarding wizard to configure identity
and model defaults before entering the chat UI.

## Prerequisites

- `xcodegen`
- `swiftlint`
- `cbindgen`
- Xcode command line tools

## Build flow

From the repository root:

```bash
just swift-poc-build-rust
just swift-poc-generate
just swift-poc-lint
just swift-poc-build
just swift-poc-run
```

Run the app from Xcode:

```bash
just swift-poc-open
```
