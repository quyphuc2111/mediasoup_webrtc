# 🚀 Auto Updater Setup - mediasoup_webrtc

## ✅ Đã cấu hình

Endpoint đã được cấu hình cho repo: **quyphuc2111/mediasoup_webrtc**

```
https://github.com/quyphuc2111/mediasoup_webrtc/releases/latest/download/latest.json
```

## 📋 Các bước tiếp theo

### Bước 1: Generate keypair (Chỉ làm 1 lần)

```bash
npm run updater:generate-key
```

**Output:**
```
Your keypair was generated successfully
Private: [saved to ~/.tauri/smartlab.key]
Public: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...
```

**⚠️ QUAN TRỌNG:** Copy dòng **Public key** (bắt đầu bằng `dW50cnVzdGVk...`)

### Bước 2: Cập nhật public key

Mở `src-tauri/tauri.conf.json` và thay thế:

```json
"pubkey": "YOUR_PUBLIC_KEY_HERE"
```

Thành:

```json
"pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6..."
```

(Paste public key từ bước 1)

### Bước 3: Install GitHub CLI

```bash
# macOS
brew install gh

# Windows
winget install --id GitHub.cli

# Login
gh auth login
```

### Bước 4: Release đầu tiên

```bash
./scripts/release-github.sh 0.1.0 quyphuc2111/mediasoup_webrtc "Initial release"
```

## 🎯 Release workflow

### Manual Release

```bash
# Release version mới
./scripts/release-github.sh 0.2.0 quyphuc2111/mediasoup_webrtc "Added document distribution and auto updater"
```

Script sẽ tự động:
1. ✅ Build app (với mediasoup-server)
2. ✅ Sign packages
3. ✅ Generate manifest
4. ✅ Create GitHub release
5. ✅ Upload files

### GitHub Actions (Tự động)

**Setup (1 lần):**

1. Vào https://github.com/quyphuc2111/mediasoup_webrtc/settings/secrets/actions
2. Click "New repository secret"
3. Name: `TAURI_PRIVATE_KEY`
4. Value: Copy nội dung file private key

```bash
# Copy private key to clipboard
cat ~/.tauri/smartlab.key | pbcopy
```

5. Click "Add secret"

**Release:**

```bash
# Commit changes
git add .
git commit -m "Release v0.2.0"

# Create and push tag
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions sẽ tự động build và release! 🎉

## 🧪 Testing

### Test endpoint

```bash
curl https://github.com/quyphuc2111/mediasoup_webrtc/releases/latest/download/latest.json
```

### Test update flow

1. Build và install version 0.1.0
2. Create release 0.2.0 trên GitHub
3. Mở app version 0.1.0
4. AutoUpdater sẽ tự động hiển thị dialog update

## 📦 Release Structure

Mỗi release sẽ có:

```
v0.2.0/
├── latest.json                                    # Update manifest
├── SmartlabPromax_0.2.0_x64.app.tar.gz           # macOS Intel
├── SmartlabPromax_0.2.0_x64.app.tar.gz.sig       # Signature
├── SmartlabPromax_0.2.0_aarch64.app.tar.gz       # macOS Apple Silicon
├── SmartlabPromax_0.2.0_aarch64.app.tar.gz.sig   # Signature
├── SmartlabPromax_0.2.0_x64-setup.nsis.zip       # Windows
└── SmartlabPromax_0.2.0_x64-setup.nsis.zip.sig   # Signature
```

## 🛠️ Commands

```bash
# Generate keypair (1 lần)
npm run updater:generate-key

# Sign packages
npm run updater:sign

# Generate GitHub manifest
npm run updater:github-manifest 0.2.0 quyphuc2111/mediasoup_webrtc "Release notes"

# Release to GitHub (all-in-one)
./scripts/release-github.sh 0.2.0 quyphuc2111/mediasoup_webrtc "Release notes"
```

## 🔐 Security Checklist

- [ ] Generate keypair
- [ ] Add public key to tauri.conf.json
- [ ] Backup private key (~/.tauri/smartlab.key)
- [ ] Add TAURI_PRIVATE_KEY to GitHub Secrets
- [ ] Test update flow
- [ ] Never commit private key to git

## 📚 Documentation

- [Quick Start](./GITHUB_UPDATER_QUICKSTART.md)
- [Full Setup Guide](./GITHUB_UPDATER_SETUP.md)
- [Auto Updater Guide](./AUTO_UPDATER_GUIDE.md)

## 🎉 Ready!

Endpoint đã được cấu hình cho repo của bạn. Chỉ cần:

1. Generate keypair
2. Add public key vào tauri.conf.json
3. Release!

```bash
npm run updater:generate-key
# Copy public key vào tauri.conf.json
./scripts/release-github.sh 0.1.0 quyphuc2111/mediasoup_webrtc "Initial release"
```

Done! 🚀
