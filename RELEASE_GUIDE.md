# Quick Release Guide

## Bước 1: Generate Signing Keys (Chỉ làm 1 lần)

```bash
# Install Tauri CLI
npm install -g @tauri-apps/cli

# Generate keypair
npm run tauri signer generate -w ~/.tauri/smartlab.key
```

**Output sẽ có dạng:**
```
Private key written to ~/.tauri/smartlab.key
Public key: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...
```

## Bước 2: Setup GitHub Secrets

Vào: https://github.com/quyphuc2111/mediasoup_webrtc/settings/secrets/actions

Thêm 2 secrets:

### 1. TAURI_SIGNING_PRIVATE_KEY
```bash
cat ~/.tauri/smartlab.key
```
Copy toàn bộ và paste vào secret.

### 2. TAURI_SIGNING_PRIVATE_KEY_PASSWORD
Để trống (nếu không set password khi generate).

## Bước 3: Update Public Key

Copy public key từ output ở Bước 1 và update vào `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "PASTE_YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

## Bước 4: Create Release

```bash
# Install dependencies
npm install

# Commit all changes
git add .
git commit -m "Release v10.0.0"

# Create and push tag
git tag v10.0.0
git push origin main
git push origin v10.0.0
```

## Bước 5: Monitor Build

Vào: https://github.com/quyphuc2111/mediasoup_webrtc/actions

Đợi ~15-20 phút cho build hoàn thành.

## Bước 6: Verify Release

Vào: https://github.com/quyphuc2111/mediasoup_webrtc/releases

Kiểm tra files:
- ✅ SmartlabPromax_10.0.0_universal.dmg (macOS)
- ✅ SmartlabPromax_10.0.0_universal.dmg.sig
- ✅ SmartlabPromax_10.0.0_x64_en-US.msi (Windows)
- ✅ SmartlabPromax_10.0.0_x64_en-US.msi.sig
- ✅ SmartlabPromax-Student_10.0.0_universal.dmg
- ✅ SmartlabPromax-Student_10.0.0_universal.dmg.sig
- ✅ SmartlabPromax-Student_10.0.0_x64_en-US.msi
- ✅ SmartlabPromax-Student_10.0.0_x64_en-US.msi.sig
- ✅ latest.json

## Bước 7: Test Update

1. Download và install v10.0.0
2. Update version trong code lên v10.0.1
3. Commit và push tag v10.0.1
4. Mở app v10.0.0 → sẽ hiện notification update

## Các lần release sau

```bash
# Update version trong:
# - package.json
# - src-tauri/tauri.conf.json

# Commit và tag
git add .
git commit -m "Release v10.0.1"
git tag v10.0.1
git push origin main
git push origin v10.0.1
```

## Troubleshooting

### Build fails: "Invalid signature"
→ Check GitHub secrets đã setup đúng

### Update không detect
→ Verify `latest.json` có trong release
→ Check version format (phải là semantic versioning: x.y.z)

### macOS: "App is damaged"
→ User cần right-click → Open lần đầu
→ Hoặc: `xattr -cr /Applications/SmartlabPromax.app`

### Windows: SmartScreen warning
→ Normal cho unsigned apps, click "More info" → "Run anyway"
→ Để tránh: cần code signing certificate ($$$)
