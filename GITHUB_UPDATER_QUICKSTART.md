# 🚀 GitHub Auto Updater - Quick Start

## Setup nhanh (5 phút)

### Bước 1: Generate keypair

```bash
npm run updater:generate-key
```

Copy **public key** (dòng bắt đầu bằng `dW50cnVzdGVk...`)

### Bước 2: Cấu hình tauri.conf.json

Mở `src-tauri/tauri.conf.json`:

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
- `YOUR_USERNAME/YOUR_REPO` → Repo GitHub của bạn
- `PASTE_YOUR_PUBLIC_KEY_HERE` → Public key từ bước 1

**Ví dụ:**
```json
"endpoints": [
  "https://github.com/zenadev/smartlab-promax/releases/latest/download/latest.json"
]
```

### Bước 3: Cài đặt GitHub CLI

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
# Build và release
./scripts/release-github.sh 0.1.0 YOUR_USERNAME/YOUR_REPO "Initial release"
```

**Ví dụ:**
```bash
./scripts/release-github.sh 0.1.0 zenadev/smartlab-promax "Initial release"
```

## Release tiếp theo

```bash
# 1. Cập nhật version trong src-tauri/tauri.conf.json
# 2. Release
./scripts/release-github.sh 0.2.0 YOUR_USERNAME/YOUR_REPO "New features"
```

## Hoặc dùng GitHub Actions (Tự động)

### Setup GitHub Actions

1. **Thêm Secret:**
   - Vào repo Settings → Secrets → Actions
   - New secret: `TAURI_PRIVATE_KEY`
   - Value: Nội dung file `~/.tauri/smartlab.key`

```bash
# Copy private key
cat ~/.tauri/smartlab.key | pbcopy
# Paste vào GitHub Secrets
```

2. **Push tag để trigger release:**

```bash
git add .
git commit -m "Release v0.2.0"
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions sẽ tự động:
- ✅ Build app
- ✅ Sign packages
- ✅ Generate manifest
- ✅ Create release
- ✅ Upload files

## Kiểm tra

### Test endpoint

```bash
# Check manifest
curl https://github.com/YOUR_USERNAME/YOUR_REPO/releases/latest/download/latest.json
```

### Test update

1. Cài đặt version cũ (v0.1.0)
2. Tạo release mới (v0.2.0)
3. Mở app v0.1.0
4. AutoUpdater sẽ tự động hiển thị dialog update

## Scripts

```bash
# Generate keypair (chỉ làm 1 lần)
npm run updater:generate-key

# Sign packages
npm run updater:sign

# Generate GitHub manifest
npm run updater:github-manifest <version> <repo> [notes]

# Release to GitHub (all-in-one)
npm run updater:release-github <version> <repo> [notes]
```

## Workflow

### Manual Release

```bash
# 1. Update version
# Edit src-tauri/tauri.conf.json

# 2. Build
npm run build:teacher

# 3. Sign
npm run updater:sign

# 4. Generate manifest
npm run updater:github-manifest 0.2.0 YOUR_USERNAME/YOUR_REPO "Release notes"

# 5. Create release
gh release create v0.2.0 \
  --title "v0.2.0" \
  --notes "Release notes" \
  latest.json \
  src-tauri/target/release/bundle/**/*.tar.gz \
  src-tauri/target/release/bundle/**/*.sig
```

### Automated Release (GitHub Actions)

```bash
# Just push a tag
git tag v0.2.0
git push origin v0.2.0

# Done! GitHub Actions handles everything
```

## Troubleshooting

### "404 Not Found"
→ Đảm bảo file `latest.json` đã được upload vào release

### "Invalid signature"
→ Public key trong config phải khớp với private key dùng để ký

### GitHub Actions failed
→ Check TAURI_PRIVATE_KEY secret đã được thêm chưa

## Checklist

- [ ] Generate keypair
- [ ] Add public key to tauri.conf.json
- [ ] Configure GitHub endpoint
- [ ] Install GitHub CLI
- [ ] Test manual release
- [ ] Setup GitHub Actions (optional)
- [ ] Add TAURI_PRIVATE_KEY secret
- [ ] Test auto update flow

## Tài liệu đầy đủ

- [GitHub Setup Guide](./GITHUB_UPDATER_SETUP.md) - Chi tiết
- [Auto Updater Guide](./AUTO_UPDATER_GUIDE.md) - Tổng quan

## Done! 🎉

Bây giờ app của bạn có thể tự động update từ GitHub Releases!
