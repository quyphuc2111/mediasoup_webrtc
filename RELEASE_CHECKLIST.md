# ✅ Release Checklist - mediasoup_webrtc

## Status

- ✅ Keypair generated (~/.tauri/smartlab.key)
- ✅ Public key added to tauri.conf.json
- ✅ Endpoint configured for quyphuc2111/mediasoup_webrtc
- ✅ GitHub CLI installed
- ⏳ GitHub CLI login needed
- ⏳ First release

## Bước 1: Login GitHub CLI

```bash
gh auth login
```

Chọn:
1. GitHub.com
2. HTTPS
3. Login with a web browser
4. Copy one-time code và paste vào browser

## Bước 2: Verify login

```bash
gh auth status
```

Should show:
```
✓ Logged in to github.com as YOUR_USERNAME
```

## Bước 3: Test GitHub access

```bash
gh repo view quyphuc2111/mediasoup_webrtc
```

## Bước 4: Build và Release

```bash
./scripts/release-github.sh 0.1.0 quyphuc2111/mediasoup_webrtc "Initial release"
```

Script sẽ:
1. Build mediasoup-server
2. Build Tauri app
3. Sign packages
4. Generate manifest
5. Create GitHub release
6. Upload files

## Bước 5: Verify Release

1. Vào https://github.com/quyphuc2111/mediasoup_webrtc/releases
2. Kiểm tra release v0.1.0 đã được tạo
3. Kiểm tra files:
   - ✅ latest.json
   - ✅ SmartlabPromax_0.1.0_*.tar.gz
   - ✅ SmartlabPromax_0.1.0_*.sig

## Bước 6: Test Update Endpoint

```bash
curl https://github.com/quyphuc2111/mediasoup_webrtc/releases/latest/download/latest.json
```

Should return JSON with version info.

## Troubleshooting

### "gh: command not found"
```bash
brew install gh
```

### "Not logged in"
```bash
gh auth login
```

### "Permission denied"
```bash
chmod +x scripts/release-github.sh
```

### "Build failed"
```bash
# Check dependencies
npm install
cd mediasoup-server && npm install
```

## Next Release

Sau khi release 0.1.0 thành công:

```bash
# 1. Update version in tauri.conf.json to 0.2.0
# 2. Release
./scripts/release-github.sh 0.2.0 quyphuc2111/mediasoup_webrtc "Added features"
```

## GitHub Actions (Optional)

Để tự động release khi push tag:

1. Vào https://github.com/quyphuc2111/mediasoup_webrtc/settings/secrets/actions
2. Add secret: `TAURI_PRIVATE_KEY`
3. Value:
```bash
cat ~/.tauri/smartlab.key | pbcopy
# Paste vào GitHub
```

Sau đó:
```bash
git tag v0.2.0
git push origin v0.2.0
# GitHub Actions tự động build và release
```

## Current Configuration

**Repo:** quyphuc2111/mediasoup_webrtc
**Endpoint:** https://github.com/quyphuc2111/mediasoup_webrtc/releases/latest/download/latest.json
**Public Key:** ✅ Configured
**Private Key:** ~/.tauri/smartlab.key

## Ready to Release! 🚀

Chỉ cần:
```bash
gh auth login
./scripts/release-github.sh 0.1.0 quyphuc2111/mediasoup_webrtc "Initial release"
```
