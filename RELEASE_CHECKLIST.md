# Release Checklist v10.0.0

## Pre-Release Setup (Chỉ làm 1 lần)

- [ ] Generate signing keys: `npm run generate:keys`
- [ ] Backup private key: `~/.tauri/smartlab.key`
- [ ] Copy public key từ output
- [ ] Update `src-tauri/tauri.conf.json` với public key
- [ ] Vào GitHub Settings → Secrets → Actions
- [ ] Add secret: `TAURI_SIGNING_PRIVATE_KEY` (paste private key content)
- [ ] Add secret: `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (để trống nếu không có)
- [ ] Commit changes: `git add . && git commit -m "Setup auto-update"`

## Release v10.0.0

- [ ] Verify version trong `package.json`: `10.0.0`
- [ ] Verify version trong `src-tauri/tauri.conf.json`: `10.0.0`
- [ ] Test build locally: `npm run build:teacher`
- [ ] Commit all changes: `git add . && git commit -m "Release v10.0.0"`
- [ ] Push to main: `git push origin main`
- [ ] Create tag: `git tag v10.0.0`
- [ ] Push tag: `git push origin v10.0.0`
- [ ] Monitor GitHub Actions: https://github.com/quyphuc2111/mediasoup_webrtc/actions
- [ ] Wait for build completion (~15-20 minutes)
- [ ] Verify release: https://github.com/quyphuc2111/mediasoup_webrtc/releases
- [ ] Check files:
  - [ ] SmartlabPromax_10.0.0_universal.dmg
  - [ ] SmartlabPromax_10.0.0_universal.dmg.sig
  - [ ] SmartlabPromax_10.0.0_x64_en-US.msi
  - [ ] SmartlabPromax_10.0.0_x64_en-US.msi.sig
  - [ ] SmartlabPromax-Student_10.0.0_universal.dmg
  - [ ] SmartlabPromax-Student_10.0.0_universal.dmg.sig
  - [ ] SmartlabPromax-Student_10.0.0_x64_en-US.msi
  - [ ] SmartlabPromax-Student_10.0.0_x64_en-US.msi.sig
  - [ ] latest.json

## Post-Release Testing

- [ ] Download macOS installer
- [ ] Install on macOS
- [ ] Test Teacher app functionality
- [ ] Test Student app functionality
- [ ] Download Windows installer
- [ ] Install on Windows
- [ ] Test Teacher app functionality
- [ ] Test Student app functionality

## Test Auto-Update (v10.0.1)

- [ ] Update version to `10.0.1` in:
  - [ ] `package.json`
  - [ ] `src-tauri/tauri.conf.json`
- [ ] Add changelog/features
- [ ] Commit: `git add . && git commit -m "Release v10.0.1"`
- [ ] Push: `git push origin main`
- [ ] Tag: `git tag v10.0.1 && git push origin v10.0.1`
- [ ] Wait for build
- [ ] Open v10.0.0 app
- [ ] Wait for update notification (or restart app)
- [ ] Click "Update Now"
- [ ] Verify download progress
- [ ] Verify app restarts
- [ ] Verify version is now 10.0.1

## Troubleshooting

### Build Fails
- [ ] Check GitHub Actions logs
- [ ] Verify Rust toolchain
- [ ] Check mediasoup-rust-server builds
- [ ] Verify all dependencies installed

### Invalid Signature Error
- [ ] Verify public key matches private key
- [ ] Check GitHub secrets are correct
- [ ] Regenerate keys if needed

### Update Not Detected
- [ ] Check `latest.json` exists in release
- [ ] Verify endpoint URL in tauri.conf.json
- [ ] Check version format (must be x.y.z)
- [ ] Check app has internet connection

### macOS: "App is damaged"
- [ ] User: Right-click → Open (first time)
- [ ] Or run: `xattr -cr /Applications/SmartlabPromax.app`

### Windows: SmartScreen Warning
- [ ] Click "More info" → "Run anyway"
- [ ] Normal for unsigned apps
- [ ] Consider code signing certificate for production

## Notes

- Always backup private key before releasing
- Never commit private key to git
- Use semantic versioning (x.y.z)
- Test on both platforms before release
- Document breaking changes in release notes
