# Hướng dẫn Auto Updater cho SmartLab ProMax

## Tổng quan

Auto Updater cho phép ứng dụng tự động kiểm tra và cài đặt các phiên bản mới mà không cần người dùng tải xuống thủ công.

## Cài đặt

### 1. Cài đặt dependencies

```bash
npm install
```

Các package đã được thêm:
- `@tauri-apps/plugin-updater` - Plugin updater cho Tauri
- `@tauri-apps/plugin-process` - Plugin để relaunch app sau khi update

### 2. Tạo keypair để ký updates

Tauri yêu cầu tất cả updates phải được ký bằng private key để đảm bảo bảo mật.

```bash
# Tạo keypair mới
npm run tauri signer generate -- -w ~/.tauri/myapp.key

# Output sẽ hiển thị:
# - Private key: Lưu vào file ~/.tauri/myapp.key (GIỮ BÍ MẬT!)
# - Public key: Dán vào tauri.conf.json
```

**LƯU Ý QUAN TRỌNG:**
- Private key phải được giữ bí mật tuyệt đối
- Không commit private key vào git
- Public key được dùng để verify updates

### 3. Cấu hình tauri.conf.json

Cập nhật file `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://your-update-server.com/{{target}}/{{arch}}/{{current_version}}"
      ],
      "dialog": true,
      "pubkey": "YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

**Thay thế:**
- `YOUR_PUBLIC_KEY_HERE` → Public key từ bước 2
- `https://your-update-server.com` → URL server của bạn

**Placeholders:**
- `{{target}}` - Platform (darwin, windows, linux)
- `{{arch}}` - Architecture (x86_64, aarch64)
- `{{current_version}}` - Phiên bản hiện tại

## Build và Deploy

### 1. Build ứng dụng với updater artifacts

```bash
npm run build
npm run tauri:build
```

Khi build với `createUpdaterArtifacts: true`, Tauri sẽ tạo:
- `.app` / `.exe` / `.AppImage` - Installer chính
- `.tar.gz` / `.zip` - Update package (nhỏ hơn)
- `.sig` - Signature file

### 2. Ký update package

```bash
# Ký file update
npm run tauri signer sign ~/.tauri/myapp.key /path/to/update.tar.gz

# Output: update.tar.gz.sig
```

### 3. Tạo update manifest

Tạo file JSON trên server với format:

```json
{
  "version": "0.2.0",
  "notes": "- Thêm tính năng mới\n- Sửa lỗi\n- Cải thiện hiệu suất",
  "pub_date": "2024-01-15T12:00:00Z",
  "platforms": {
    "darwin-x86_64": {
      "signature": "SIGNATURE_FROM_SIG_FILE",
      "url": "https://your-server.com/updates/SmartlabPromax_0.2.0_x64.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "SIGNATURE_FROM_SIG_FILE",
      "url": "https://your-server.com/updates/SmartlabPromax_0.2.0_aarch64.app.tar.gz"
    },
    "windows-x86_64": {
      "signature": "SIGNATURE_FROM_SIG_FILE",
      "url": "https://your-server.com/updates/SmartlabPromax_0.2.0_x64-setup.nsis.zip"
    }
  }
}
```

**Lấy signature:**
```bash
cat /path/to/update.tar.gz.sig
```

### 4. Upload lên server

Upload các file sau lên server:
- Update manifest JSON
- Update packages (.tar.gz / .zip)
- Signature files (.sig)

**Cấu trúc thư mục đề xuất:**
```
your-server.com/
├── darwin/
│   ├── x86_64/
│   │   └── 0.1.0  → returns JSON manifest
│   └── aarch64/
│       └── 0.1.0  → returns JSON manifest
├── windows/
│   └── x86_64/
│       └── 0.1.0  → returns JSON manifest
└── updates/
    ├── SmartlabPromax_0.2.0_x64.app.tar.gz
    ├── SmartlabPromax_0.2.0_x64.app.tar.gz.sig
    └── ...
```

## Cách hoạt động

### 1. Kiểm tra update

Khi app khởi động, `AutoUpdater` component sẽ:
1. Gọi API endpoint với current version
2. Server trả về manifest nếu có version mới
3. Hiển thị dialog thông báo update

### 2. Download và install

Khi user nhấn "Cập nhật ngay":
1. Download update package từ URL trong manifest
2. Verify signature với public key
3. Extract và install update
4. Relaunch app

### 3. Rollback

Nếu update thất bại, Tauri tự động rollback về version cũ.

## Testing

### Test trong development

```bash
# 1. Build version 0.1.0
npm run tauri:build

# 2. Cập nhật version trong tauri.conf.json → 0.2.0
# 3. Build lại
npm run tauri:build

# 4. Setup local server với manifest
# 5. Chạy app version 0.1.0 và test update
```

### Test với local server

```javascript
// Simple Express server for testing
const express = require('express');
const app = express();

app.get('/darwin/x86_64/:version', (req, res) => {
  res.json({
    version: '0.2.0',
    notes: 'Test update',
    pub_date: new Date().toISOString(),
    platforms: {
      'darwin-x86_64': {
        signature: 'YOUR_SIGNATURE',
        url: 'http://localhost:3000/updates/app.tar.gz'
      }
    }
  });
});

app.use('/updates', express.static('updates'));
app.listen(3000);
```

## Troubleshooting

### Update không hoạt động

1. **Kiểm tra public key** - Đảm bảo public key trong tauri.conf.json đúng
2. **Kiểm tra signature** - Verify signature file được tạo đúng
3. **Kiểm tra URL** - Đảm bảo endpoint trả về JSON đúng format
4. **Kiểm tra CORS** - Server phải cho phép CORS nếu test từ localhost

### Lỗi signature verification

```
Error: Invalid signature
```

→ Signature không khớp với public key. Đảm bảo:
- Dùng đúng private key để ký
- Public key trong config khớp với private key
- Signature file không bị corrupt

### Update không tự động check

→ Kiểm tra:
- Plugin updater đã được init trong lib.rs
- AutoUpdater component đã được thêm vào App.tsx
- Không có lỗi trong console

## Best Practices

### 1. Versioning

Sử dụng semantic versioning (MAJOR.MINOR.PATCH):
- MAJOR: Breaking changes
- MINOR: New features
- PATCH: Bug fixes

### 2. Release Notes

Viết release notes rõ ràng:
```
- ✨ Thêm tính năng phân phối tài liệu
- 🐛 Sửa lỗi kết nối WebRTC
- ⚡ Cải thiện hiệu suất screen capture
- 🔒 Tăng cường bảo mật với LDAP
```

### 3. Staged Rollout

Không deploy update cho tất cả user cùng lúc:
1. Deploy cho 10% users trước
2. Monitor errors
3. Tăng dần lên 50%, 100%

### 4. Backup

Luôn giữ backup của:
- Private key
- Previous versions
- Update manifests

## Security

### Bảo vệ Private Key

```bash
# Set permissions
chmod 600 ~/.tauri/myapp.key

# Backup encrypted
gpg -c ~/.tauri/myapp.key
```

### HTTPS Only

Luôn dùng HTTPS cho update server:
- Ngăn man-in-the-middle attacks
- Bảo vệ update packages

### Verify Downloads

Tauri tự động verify signature, nhưng nên:
- Monitor download logs
- Alert nếu có signature failures
- Track update success rate

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Setup Node
        uses: actions/setup-node@v2
        
      - name: Install dependencies
        run: npm install
        
      - name: Build
        run: npm run tauri:build
        
      - name: Sign update
        env:
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
        run: |
          echo "$TAURI_PRIVATE_KEY" > private.key
          npm run tauri signer sign private.key src-tauri/target/release/bundle/macos/*.app.tar.gz
          
      - name: Upload to S3
        run: |
          aws s3 cp src-tauri/target/release/bundle/macos/*.tar.gz s3://updates/
          aws s3 cp src-tauri/target/release/bundle/macos/*.sig s3://updates/
```

## Monitoring

### Track Update Metrics

```typescript
// Add analytics to AutoUpdater.tsx
const trackUpdate = (event: string, data?: any) => {
  // Send to analytics service
  console.log('Update event:', event, data);
};

// Track events:
trackUpdate('update_check_started');
trackUpdate('update_available', { version: update.version });
trackUpdate('update_download_started');
trackUpdate('update_installed');
trackUpdate('update_failed', { error: err.message });
```

## Resources

- [Tauri Updater Docs](https://tauri.app/v1/guides/distribution/updater)
- [Signing Updates](https://tauri.app/v1/guides/distribution/sign-updates)
- [Update Server Setup](https://tauri.app/v1/guides/distribution/updater-server)

## Support

Nếu gặp vấn đề:
1. Check console logs
2. Verify configuration
3. Test với local server
4. Check Tauri Discord/GitHub issues
