# 🚀 Auto Updater cho SmartLab ProMax

Chức năng tự động cập nhật ứng dụng đã được tích hợp thành công!

## ✅ Đã hoàn thành

### 1. Backend (Rust)
- ✅ Thêm `tauri-plugin-updater` vào Cargo.toml
- ✅ Khởi tạo updater plugin trong lib.rs
- ✅ Cấu hình `createUpdaterArtifacts: true` trong tauri.conf.json

### 2. Frontend (React)
- ✅ Component `AutoUpdater.tsx` với UI đẹp
- ✅ Tự động check updates khi app khởi động
- ✅ Dialog thông báo update với progress bar
- ✅ Download và install tự động
- ✅ Tích hợp vào App.tsx

### 3. Configuration
- ✅ Cấu hình updater trong tauri.conf.json
- ✅ Placeholder cho public key và endpoint
- ✅ Dependencies đã được cài đặt

### 4. Scripts & Tools
- ✅ `npm run updater:generate-key` - Tạo keypair
- ✅ `npm run updater:sign` - Ký update packages
- ✅ `npm run updater:manifest` - Tạo update manifest
- ✅ Example update server cho testing

### 5. Documentation
- ✅ `AUTO_UPDATER_QUICKSTART.md` - Hướng dẫn nhanh
- ✅ `AUTO_UPDATER_GUIDE.md` - Hướng dẫn chi tiết
- ✅ Scripts với comments đầy đủ

## 📋 Các bước tiếp theo

### Bước 1: Tạo keypair (Chỉ làm 1 lần)

```bash
npm run updater:generate-key
```

**Output sẽ hiển thị:**
```
Your keypair was generated successfully
Private: dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5...
Public: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEFCQ0RFRkc...

Keys saved to: ~/.tauri/smartlab.key
```

**⚠️ QUAN TRỌNG:**
- Copy **Public key** (dòng bắt đầu bằng `dW50cnVzdGVk...`)
- Private key đã được lưu tự động vào `~/.tauri/smartlab.key`
- **KHÔNG BAO GIỜ** commit private key vào git!

### Bước 2: Cấu hình public key

Mở `src-tauri/tauri.conf.json` và thay thế:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "PASTE_YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

Paste public key từ bước 1 vào.

### Bước 3: Cấu hình update server

Trong `src-tauri/tauri.conf.json`, cập nhật endpoint:

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://your-server.com/{{target}}/{{arch}}/{{current_version}}"
      ]
    }
  }
}
```

**Cho testing local:**
```json
"endpoints": [
  "http://localhost:3030/{{target}}/{{arch}}/{{current_version}}"
]
```

### Bước 4: Build và test

```bash
# Build version đầu tiên (0.1.0)
npm run build:teacher

# Cập nhật version trong tauri.conf.json → 0.2.0
# Build version mới
npm run build:teacher

# Ký packages
npm run updater:sign

# Tạo manifest
npm run updater:manifest 0.2.0 "Thêm tính năng mới"

# Test với local server
node scripts/update-server-example.js

# Cài đặt version 0.1.0 và test update
```

## 🎯 Tính năng

### Auto Check
- Tự động check updates khi app khởi động
- Không làm gián đoạn user experience
- Chỉ hiển thị dialog khi có update

### Beautiful UI
- Dialog hiện đại với gradient
- Progress bar khi download
- Release notes hiển thị rõ ràng
- Error handling với thông báo thân thiện

### Security
- Tất cả updates phải được ký bằng private key
- Verify signature trước khi install
- HTTPS recommended cho production
- Rollback tự động nếu update thất bại

### User Control
- User có thể chọn "Để sau"
- Hoặc "Cập nhật ngay"
- Không force update (có thể customize)

## 📁 Files đã tạo

```
src/
├── components/
│   └── AutoUpdater.tsx          # Component auto updater
scripts/
├── generate-update-manifest.js  # Tạo manifest
├── sign-updates.sh              # Ký packages
└── update-server-example.js     # Test server
docs/
├── AUTO_UPDATER_QUICKSTART.md   # Quick start
├── AUTO_UPDATER_GUIDE.md        # Chi tiết
└── AUTO_UPDATER_README.md       # File này
```

## 🔧 Scripts

```bash
# Tạo keypair (chỉ làm 1 lần)
npm run updater:generate-key

# Ký tất cả update packages
npm run updater:sign

# Tạo update manifest
npm run updater:manifest <version> "<notes>"

# Example:
npm run updater:manifest 0.2.0 "Bug fixes and improvements"
```

## 🌐 Update Server Options

### Option 1: Local Testing
```bash
node scripts/update-server-example.js
```

### Option 2: Static Hosting
- Vercel
- Netlify
- GitHub Pages

### Option 3: CDN
- CloudFlare
- AWS CloudFront
- Azure CDN

### Option 4: Object Storage
- AWS S3
- Google Cloud Storage
- Azure Blob Storage

## 📊 Workflow

```
1. Develop new features
   ↓
2. Update version in tauri.conf.json
   ↓
3. Build: npm run build:teacher
   ↓
4. Sign: npm run updater:sign
   ↓
5. Generate manifest: npm run updater:manifest
   ↓
6. Upload to server:
   - update-manifest.json
   - *.tar.gz / *.zip
   - *.sig
   ↓
7. Users get auto-update notification
   ↓
8. Monitor update success rate
```

## 🔐 Security Checklist

- [ ] Private key được backup an toàn
- [ ] Private key KHÔNG được commit vào git
- [ ] Public key đã được thêm vào tauri.conf.json
- [ ] Update server dùng HTTPS (production)
- [ ] Signatures được verify trước khi install
- [ ] Monitor failed updates

## 📈 Monitoring

Thêm analytics vào `AutoUpdater.tsx`:

```typescript
// Track update events
trackEvent('update_available', { version: update.version });
trackEvent('update_installed', { version: update.version });
trackEvent('update_failed', { error: err.message });
```

## 🐛 Troubleshooting

### "Invalid signature"
→ Public key không khớp với private key dùng để ký

### "No update available"
→ Kiểm tra version trong manifest > version hiện tại

### Update không tự động check
→ Kiểm tra console logs, có thể endpoint không accessible

### "Failed to download"
→ Kiểm tra URL trong manifest có đúng không

## 📚 Tài liệu

- [Quick Start](./AUTO_UPDATER_QUICKSTART.md) - Bắt đầu nhanh
- [Full Guide](./AUTO_UPDATER_GUIDE.md) - Hướng dẫn đầy đủ
- [Tauri Docs](https://tauri.app/v1/guides/distribution/updater) - Official docs

## 💡 Tips

1. **Versioning**: Dùng semantic versioning (MAJOR.MINOR.PATCH)
2. **Release Notes**: Viết rõ ràng những gì thay đổi
3. **Staged Rollout**: Deploy cho 10% users trước, sau đó mở rộng
4. **Backup**: Luôn giữ backup của private key và previous versions
5. **Testing**: Test update flow trước khi deploy production

## 🎉 Hoàn thành!

Auto updater đã sẵn sàng sử dụng. Chỉ cần:
1. Generate keypair
2. Cấu hình public key
3. Build và deploy

Users sẽ tự động nhận được updates mới! 🚀
