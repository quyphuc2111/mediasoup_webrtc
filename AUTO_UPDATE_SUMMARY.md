# Auto-Update Setup Summary

## ✅ Đã hoàn thành

### 1. Version Update
- ✅ `package.json`: v10.0.0
- ✅ `src-tauri/tauri.conf.json`: v10.0.0

### 2. Updater Configuration
- ✅ Thêm updater config vào `tauri.conf.json`
- ✅ Endpoint: GitHub releases
- ✅ Dialog: enabled (hiện popup khi có update)

### 3. GitHub Actions Workflow
- ✅ File: `.github/workflows/release.yml`
- ✅ Platforms: macOS (Universal) + Windows (x64)
- ✅ Builds: Teacher + Student versions
- ✅ Auto-sign với private key từ GitHub Secrets

### 4. Frontend UI
- ✅ Component: `src/components/UpdateChecker.tsx`
- ✅ Features:
  - Auto-check mỗi giờ
  - Download progress bar
  - Install và restart
  - Error handling

### 5. Dependencies
- ✅ `@tauri-apps/plugin-updater`: ^2
- ✅ `@tauri-apps/plugin-process`: ^2
- ✅ Cargo: `tauri-plugin-updater`, `tauri-plugin-process`

## 📋 Cần làm tiếp

### 1. Generate Signing Keys
```bash
npm run tauri signer generate -w ~/.tauri/smartlab.key
```

### 2. Setup GitHub Secrets
- `TAURI_SIGNING_PRIVATE_KEY`: Private key content
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Password (nếu có)

### 3. Update Public Key
Copy public key vào `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`

### 4. First Release
```bash
git add .
git commit -m "Setup auto-update v10.0.0"
git tag v10.0.0
git push origin main
git push origin v10.0.0
```

## 📁 Files Created/Modified

### New Files
- `.github/workflows/release.yml` - GitHub Actions workflow
- `src/components/UpdateChecker.tsx` - Update UI component
- `UPDATER_SETUP.md` - Detailed setup guide
- `RELEASE_GUIDE.md` - Quick release guide
- `AUTO_UPDATE_SUMMARY.md` - This file

### Modified Files
- `package.json` - Version + dependencies
- `src-tauri/tauri.conf.json` - Version + updater config
- `src-tauri/Cargo.toml` - Updater dependencies
- `src-tauri/src/lib.rs` - Register updater plugin
- `src/App.tsx` - Add UpdateChecker component

## 🚀 How It Works

1. **Developer**: Push tag `v10.0.x`
2. **GitHub Actions**: 
   - Build macOS + Windows apps
   - Sign with private key
   - Create GitHub Release
   - Upload installers + signatures
   - Generate `latest.json`
3. **User App**:
   - Check `latest.json` every hour
   - Compare versions
   - Show update notification
   - Download + install + restart

## 📦 Release Artifacts

Mỗi release sẽ có:
- `SmartlabPromax_10.0.0_universal.dmg` (macOS Intel + Apple Silicon)
- `SmartlabPromax_10.0.0_universal.dmg.sig`
- `SmartlabPromax_10.0.0_x64_en-US.msi` (Windows 64-bit)
- `SmartlabPromax_10.0.0_x64_en-US.msi.sig`
- `SmartlabPromax-Student_10.0.0_universal.dmg`
- `SmartlabPromax-Student_10.0.0_universal.dmg.sig`
- `SmartlabPromax-Student_10.0.0_x64_en-US.msi`
- `SmartlabPromax-Student_10.0.0_x64_en-US.msi.sig`
- `latest.json` (update manifest)

## 🔐 Security

- ✅ Signed updates với Ed25519 keypair
- ✅ Private key stored in GitHub Secrets
- ✅ Public key embedded in app
- ✅ Signature verification before install
- ✅ HTTPS download từ GitHub

## 📖 Documentation

- `UPDATER_SETUP.md` - Chi tiết setup từng bước
- `RELEASE_GUIDE.md` - Quick reference cho release
- Repository: https://github.com/quyphuc2111/mediasoup_webrtc

## 🎯 Next Steps

1. Đọc `RELEASE_GUIDE.md`
2. Generate signing keys
3. Setup GitHub Secrets
4. Update public key
5. Create first release v10.0.0
6. Test update flow

## ⚠️ Important Notes

- **KHÔNG commit private key** vào git
- **BACKUP private key** ở nơi an toàn
- Mất private key = không thể release updates
- Public key phải match với private key
- Version phải follow semantic versioning (x.y.z)
