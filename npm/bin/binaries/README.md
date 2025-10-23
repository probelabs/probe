# Bundled Probe Binaries

This directory contains pre-compiled probe binaries for all supported platforms, bundled with the npm package to enable offline installation.

## Expected Files

The CI/CD pipeline should place the following compressed binaries here before publishing to npm:

- `probe-v{VERSION}-x86_64-unknown-linux-musl.tar.gz` - Linux x64 (static)
- `probe-v{VERSION}-aarch64-unknown-linux-musl.tar.gz` - Linux ARM64 (static)
- `probe-v{VERSION}-x86_64-apple-darwin.tar.gz` - macOS Intel
- `probe-v{VERSION}-aarch64-apple-darwin.tar.gz` - macOS Apple Silicon
- `probe-v{VERSION}-x86_64-pc-windows-msvc.zip` - Windows x64

## File Size

Each compressed binary is approximately 5MB, totaling ~25MB for all 5 platforms.

## Installation Flow

1. **Postinstall script** (`scripts/postinstall.js`) detects the current platform
2. **Extraction** (`src/extractor.js`) extracts the matching bundled binary
3. **Fallback**: If no bundled binary is found, downloads from GitHub releases

## CI Integration

The release workflow (`.github/workflows/release.yml`) should:

1. Build binaries for all 5 platforms
2. Create compressed archives (`.tar.gz` or `.zip`)
3. Copy them to `npm/bin/binaries/` before running `npm publish`

Example CI step:
```yaml
- name: Copy binaries to npm package
  run: |
    mkdir -p npm/bin/binaries
    cp dist/probe-v$VERSION-*.tar.gz npm/bin/binaries/
    cp dist/probe-v$VERSION-*.zip npm/bin/binaries/
```
