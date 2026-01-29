# Build Troubleshooting

## "Student Binary Not Found" Error
... (same as before)

## DMG Bundle Error
When running Universal Build, `bundle_dmg.sh` may fail with `failed to run ... bundle_dmg.sh`.
This is a known issue with `create-dmg` in some environments or when volume names conflict.

### Workaround
We have temporarily disabled DMG bundling in `tauri.conf.json` to ensure the build completes successfully.
The build will output `Screen Sharing Teacher.app` (Universal Application Bundle) instead of a `.dmg`.

### Output Location
`src-tauri/target/universal-apple-darwin/release/bundle/macos/Screen Sharing Teacher.app`

You can zip this file and distribute it. It works natively on both Intel and Apple Silicon Macs.
