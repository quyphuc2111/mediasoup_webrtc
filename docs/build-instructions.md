# Build Instructions

## Universal macOS Build (Intel + Apple Silicon)
We have added a new script to build a single application bundle that runs natively on both Intel and Apple Silicon Macs.

### Command
```bash
npm run build:teacher:universal
```

### What it does
1. Installs Rust targets for both architectures: `x86_64-apple-darwin` and `aarch64-apple-darwin`.
2. Builds the frontend (`npm run build`).
3. Compiles the Rust backend for both architectures.
4. Lipo-s (stitches) them together into a Universal binary.

### Output
The resulting `.app` and `.dmg` will be in:
`src-tauri/target/universal-apple-darwin/release/bundle/`

## Other Build Options
- **Current Architecture Only** (Default): `npm run build:teacher`
- **Intel Only**: `npm run build:teacher:intel`
