# 🚀 Auto Updater với GitHub Releases

## ✅ Hoàn thành

Chức năng auto updater đã được tích hợp hoàn toàn với GitHub Releases!

## 🎯 Tính năng

- ✅ **Tự động check updates** khi app khởi động
- ✅ **Beautiful UI** với dialog hiện đại
- ✅ **Progress bar** khi download
- ✅ **GitHub Releases** làm CDN miễn phí
- ✅ **GitHub Actions** tự động build và release
- ✅ **Secure** với signature verification
- ✅ **Cross-platform** (macOS, Windows, Linux)

## 📁 Files đã tạo

### Scripts
```
scripts/
├── generate-github-manifest.js   # Tạo manifest cho GitHub
├── release-github.sh              # Release lên GitHub (all-in-one)
└── sign-updates.sh                # Ký packages
```

### GitHub Actions
```
.github/workflows/
└── release.yml                    # Tự động build và release
```

### Documentation
```
docs/
├── GITHUB_UPDATER_SETUP.md        # Hướng dẫn chi tiết
├── GITHUB_UPDATER_QUICKSTART.md   # Quick start
├── AUTO_UPDATER_GUIDE.md          # Tổng quan auto updater
└── UPDATER_GITHUB_README.md       # File này
```

### Examples
```
tauri.conf.github.example.json     # Example config
update-manifest.example.json       # Example manifest
```

## 🚀 Quick Start

### 1. Generate keypair (1 lần duy nhất)

```bash
npm run updater:generate-key
```

Output:
```
Your keypair was generated successfully
Private: [saved to ~/.tauri/smartlab.key]
Public: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...
```

**Copy public key!**

### 2. Cấu hình tauri.conf.json

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/YOUR_USERNAME/YOUR_REPO/releases/latest/download/latest.json"
      ],
      "pubkey": "PASTE_YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

**Thay thế:**
- `YOUR_USERNAME/YOUR_REPO` → Repo của bạn (vd: `zenadev/smartlab-promax`)
- `PASTE_YOUR_PUBLIC_KEY_HERE` → Public key từ bước 1

### 3. Install GitHub CLI

```bash
# macOS
brew install gh

# Windows
winget install --id GitHub.cli

# Login
gh auth login
```

### 4. Release đầu tiên

```bash
./scripts/release-github.sh 0.1.0 YOUR_USERNAME/YOUR_REPO "Initial release"
```

**Done!** 🎉

## 📊 Workflow

### Option 1: Manual Release (Nhanh)

```bash
# 1. Update version trong tauri.conf.json
# 2. Run script
./scripts/release-github.sh 0.2.0 YOUR_USERNAME/YOUR_REPO "New features"
```

Script sẽ tự động:
1. Build app
2. Sign packages
3. Generate manifest
4. Create GitHub release
5. Upload files

### Option 2: GitHub Actions (Tự động)

**Setup (1 lần):**

1. Vào repo Settings → Secrets → Actions
2. Add secret: `TAURI_PRIVATE_KEY`
3. Value: Nội dung file `~/.tauri/smartlab.key`

```bash
cat ~/.tauri/smartlab.key | pbcopy
```

**Release:**

```bash
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions tự động làm tất cả!

## 🎨 User Experience

1. **App khởi động** → Tự động check updates
2. **Có update mới** → Hiển thị dialog đẹp
3. **User click "Cập nhật"** → Download với progress bar
4. **Download xong** → Tự động install và relaunch
5. **Done!** → App đã được update

## 🔐 Security

- ✅ Tất cả updates phải được ký
- ✅ Signature được verify trước khi install
- ✅ Private key không bao giờ được commit
- ✅ HTTPS mặc định (GitHub)
- ✅ Rollback tự động nếu thất bại

## 📦 GitHub Release Structure

Mỗi release có:

```
v0.2.0/
├── latest.json                                    # Manifest
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
npm run updater:github-manifest <version> <repo> [notes]

# Release to GitHub (all-in-one)
./scripts/release-github.sh <version> <repo> [notes]
```

## 📝 Examples

### Manual release

```bash
# Build version 0.1.0
./scripts/release-github.sh 0.1.0 zenadev/smartlab-promax "Initial release"

# Build version 0.2.0
./scripts/release-github.sh 0.2.0 zenadev/smartlab-promax "Added document distribution"
```

### GitHub Actions

```bash
# Update version in tauri.conf.json to 0.2.0
git add .
git commit -m "Release v0.2.0"
git tag v0.2.0
git push origin v0.2.0

# GitHub Actions will automatically:
# - Build for macOS and Windows
# - Sign packages
# - Create release
# - Upload files
```

## 🧪 Testing

### Test endpoint

```bash
curl https://github.com/YOUR_USERNAME/YOUR_REPO/releases/latest/download/latest.json
```

Should return:
```json
{
  "version": "0.2.0",
  "notes": "Release notes",
  "pub_date": "2024-01-15T12:00:00Z",
  "platforms": {
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://github.com/.../SmartlabPromax_0.2.0_x64.app.tar.gz"
    }
  }
}
```

### Test update flow

1. Build và install version 0.1.0
2. Create release 0.2.0 trên GitHub
3. Mở app version 0.1.0
4. AutoUpdater sẽ hiển thị dialog
5. Click "Cập nhật ngay"
6. App sẽ download, install và relaunch

## 🐛 Troubleshooting

### "404 Not Found"
→ Đảm bảo `latest.json` đã được upload vào release

### "Invalid signature"
→ Public key trong config phải khớp với private key

### GitHub Actions failed
→ Check `TAURI_PRIVATE_KEY` secret đã được thêm

### Update không tự động check
→ Check console logs, có thể endpoint sai

## ✨ Advantages của GitHub

| Feature | GitHub | Self-hosted |
|---------|--------|-------------|
| Cost | ✅ Free | ❌ Paid |
| CDN | ✅ Global | ⚠️ Depends |
| Setup | ✅ Easy | ❌ Complex |
| Maintenance | ✅ None | ❌ Required |
| Reliability | ✅ 99.9% | ⚠️ Varies |
| HTTPS | ✅ Default | ⚠️ Setup needed |

## 📚 Documentation

- **Quick Start**: [GITHUB_UPDATER_QUICKSTART.md](./GITHUB_UPDATER_QUICKSTART.md)
- **Full Guide**: [GITHUB_UPDATER_SETUP.md](./GITHUB_UPDATER_SETUP.md)
- **Auto Updater**: [AUTO_UPDATER_GUIDE.md](./AUTO_UPDATER_GUIDE.md)

## 🎯 Best Practices

### 1. Versioning
```
v0.1.0 → Initial release
v0.2.0 → New features
v0.2.1 → Bug fixes
v1.0.0 → Major release
```

### 2. Release Notes
```markdown
## 🎉 Version 0.2.0

### ✨ New Features
- Document distribution system
- Auto updater

### 🐛 Bug Fixes
- Fixed WebRTC issues

### ⚡ Improvements
- Better performance
```

### 3. Testing
- Test update flow trước khi release
- Monitor update success rate
- Keep backup của previous versions

### 4. Security
- Backup private key an toàn
- Không commit private key vào git
- Dùng GitHub Secrets cho CI/CD

## 📊 Monitoring

Track update metrics:

```typescript
// In AutoUpdater.tsx
trackEvent('update_available', { version });
trackEvent('update_downloaded', { version });
trackEvent('update_installed', { version });
trackEvent('update_failed', { error });
```

## 🎉 Kết luận

Auto updater với GitHub Releases đã sẵn sàng!

**Advantages:**
- ✅ Miễn phí hoàn toàn
- ✅ Setup đơn giản (5 phút)
- ✅ CDN toàn cầu nhanh
- ✅ Tự động với GitHub Actions
- ✅ Bảo mật với signature
- ✅ UI đẹp, UX tốt

**Next steps:**
1. Generate keypair
2. Configure tauri.conf.json
3. Release first version
4. Test update flow
5. Setup GitHub Actions (optional)

Done! 🚀
