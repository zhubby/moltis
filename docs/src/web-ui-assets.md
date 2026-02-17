# Web UI Assets

Moltis supports two deployment models for the web UI static files (JS/CSS/icons/manifest/service worker):

1. Embedded in the binary (`include_dir!`)
2. External files on disk (for package-manager and container layouts)

By default, Moltis keeps assets embedded for a single-binary experience.

## Feature Flags

`moltis-gateway` now separates UI runtime and asset embedding:

- `web-ui`: enables web UI routes and templates
- `web-ui-embedded-assets`: enables embedded static assets

Default behavior includes both flags.

## Runtime Asset Resolution

When serving static assets, Moltis checks paths in this order:

1. `MOLTIS_ASSETS_DIR` (runtime environment)
2. `MOLTIS_DEFAULT_ASSETS_DIR` (compile-time default via `option_env!`)
3. Auto-detected source tree path (`crates/gateway/src/assets`, for local `cargo run`)
4. Embedded assets (only if built with `web-ui-embedded-assets`)

### Required Files

For non-embedded builds, the assets directory must include at least:

- `style.css`
- `js/app.js`
- `js/onboarding-app.js`
- `manifest.json`
- `sw.js`

If Moltis is built **without** `web-ui-embedded-assets` and assets are missing,
startup fails fast with an actionable error.

## Build Profiles

### Single-Binary (Default)

Embedded assets (default release behavior):

```bash
cargo build --release -p moltis
```

### External Assets (Packaging / Distro)

Build without embedded assets and provide a compiled default path:

```bash
MOLTIS_DEFAULT_ASSETS_DIR=/usr/share/moltis/assets \
cargo build --release -p moltis \
  --no-default-features \
  --features "file-watcher,local-llm,metrics,prometheus,push-notifications,qmd,tailscale,tls,voice,web-ui"
```

Then install/copy `crates/gateway/src/assets` to the same directory:

```bash
install -d /usr/share/moltis
cp -R crates/gateway/src/assets /usr/share/moltis/assets
```

### Runtime Override

To override assets path at runtime:

```bash
MOLTIS_ASSETS_DIR=/custom/assets/path moltis
```

## Packaging Conventions

Current packaging defaults use external assets in distro/container contexts:

- Docker image copies assets to `/usr/share/moltis/assets`
- deb/rpm package metadata installs assets to `/usr/share/moltis/assets`
- Homebrew formula installs assets under `pkgshare/assets` and sets `MOLTIS_ASSETS_DIR`

## Caching Behavior

- Local dev source-tree assets use no-cache behavior.
- Packaged filesystem assets use hashed versioned asset URLs and immutable cache headers.
- Embedded assets use hashed versioned URLs as before.

## Troubleshooting

If startup fails with an assets error:

1. Verify the directory exists and is readable.
2. Verify required files listed above are present.
3. Check whether the binary was built without embedded assets.
4. Set `MOLTIS_ASSETS_DIR` explicitly to the assets directory and restart.
