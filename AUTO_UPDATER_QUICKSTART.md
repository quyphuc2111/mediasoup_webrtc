# Auto Updater - Quick Start

## Bước 1: Cài đặt dependencies

```bash
npm install
```

## Bước 2: Tạo keypair

```bash
# Tạo private/public key pair
npm run updater:generate-key

# Output sẽ hiển thị public key, copy nó
```

## Bước 3: Cấu hình

Mở `src-tauri/tauri.conf.json` và cập nhật:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "PASTE_YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

**Lưu ý:** Thay `PASTE_YOUR_PUBLIC_KEY_HERE` bằng public key từ bước 2.

## Bước 4: Build version đầu tiên

```bash
# Build version 0.1.0
npm run build:teacher
```

## Bước 5: Tạo version mới

1. Cập nhật version trong `src-tauri/tauri.conf.json`:
```json
{
  "version": "0.2.0"
}
```

2. Build lại:
```bash
npm run build:teacher
```

## Bước 6: Ký update packages

```bash
npm run updater:sign
```

## Bước 7: Tạo manifest

```bash
npm run updater:manifest 0.2.0 "Thêm tính năng mới"
```

File `update-manifest.json` sẽ được tạo.

## Bước 8: Setup update server

### Option A: Test với local server

```bash
# Install express và cors
npm install express cors

# Tạo thư mục updates
mkdir updates

# Copy update packages vào updates/
cp src-tauri/target/release/bundle/macos/*.tar.gz updates/
cp src-tauri/target/release/bundle/macos/*.sig updates/

# Chạy server
node scripts/update-server-example.js
```

### Option B: Deploy lên production

Upload các file sau lên server của bạn:
- `update-manifest.json` → Endpoint trả về JSON này
- `updates/*.tar.gz` → Update packages
- `updates/*.sig` → Signature files

## Bước 9: Cấu hình endpoint

Cập nhật `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "http://localhost:3030/{{target}}/{{arch}}/{{current_version}}"
      ]
    }
  }
}
```

**Production:** Thay `http://localhost:3030` bằng URL server thực.

## Bước 10: Test

1. Cài đặt version 0.1.0
2. Chạy app
3. AutoUpdater sẽ tự động check và hiển thị dialog update
4. Click "Cập nhật ngay" để test

## Workflow cho releases tiếp theo

```bash
# 1. Cập nhật version trong tauri.conf.json
# 2. Build
npm run build:teacher

# 3. Ký packages
npm run updater:sign

# 4. Tạo manifest
npm run updater:manifest <version> "<release notes>"

# 5. Upload lên server
# - update-manifest.json
# - updates/*.tar.gz
# - updates/*.sig

# 6. Test với version cũ
```

## Troubleshooting

### "Invalid signature"
→ Đảm bảo public key trong config khớp với private key dùng để ký

### "No update available"
→ Kiểm tra:
- Version trong manifest > version hiện tại
- Endpoint URL đúng
- Server trả về JSON đúng format

### Update không tự động check
→ Kiểm tra console logs, có thể endpoint không accessible

## Production Checklist

- [ ] Private key được backup an toàn
- [ ] Public key đã được thêm vào tauri.conf.json
- [ ] Update server dùng HTTPS
- [ ] Manifest endpoint trả về đúng format
- [ ] Update packages đã được ký
- [ ] Test update flow hoàn chỉnh
- [ ] Monitor update success rate

## Các lệnh hữu ích

```bash
# Tạo keypair mới
npm run updater:generate-key

# Ký tất cả packages
npm run updater:sign

# Tạo manifest
npm run updater:manifest <version> "<notes>"

# Build với updater artifacts
npm run build:teacher

# Test local server
node scripts/update-server-example.js
```

## Tài liệu đầy đủ

Xem `AUTO_UPDATER_GUIDE.md` để biết chi tiết về:
- Security best practices
- CI/CD integration
- Monitoring và analytics
- Advanced configuration
