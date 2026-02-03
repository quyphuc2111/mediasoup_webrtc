# Auto-Update Setup Guide

## 1. Generate Signing Keys

Tauri updater cần signing keys để verify updates. Chạy lệnh sau để generate:

```bash
# Install Tauri CLI nếu chưa có
cargo install tauri-cli

# Generate keypair
tauri signer generate -w ~/.tauri/myapp.key
```

Lệnh này sẽ tạo:
- **Private key**: `~/.tauri/myapp.key` (GIỮ BÍ MẬT!)
- **Public key**: In ra console (copy để dùng)

## 2. Setup GitHub Secrets

Vào GitHub repository settings → Secrets and variables → Actions, thêm 2 secrets:

### TAURI_SIGNING_PRIVATE_KEY
```bash
# Đọc private key
cat ~/.tauri/myapp.key
```
Copy toàn bộ nội dung và paste vào secret.

### TAURI_SIGNING_PRIVATE_KEY_PASSWORD
Nếu bạn đặt password khi generate key, thêm password vào đây. Nếu không có password, để trống hoặc bỏ qua.

## 3. Update Public Key trong tauri.conf.json

Copy public key từ output của lệnh generate và update vào `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

## 4. Create First Release

```bash
# Commit changes
git add .
git commit -m "Setup auto-update v10.0.0"

# Create and push tag
git tag v10.0.0
git push origin v10.0.0
```

GitHub Actions sẽ tự động:
1. Build app cho macOS (Universal) và Windows
2. Sign các files với private key
3. Tạo GitHub Release với installers
4. Generate `latest.json` cho updater

## 5. Test Update

Sau khi release v10.0.0 thành công:

1. Install app từ release
2. Update version trong code lên v10.0.1
3. Commit và push tag v10.0.1
4. App sẽ tự động detect update và hiện dialog

## Cấu trúc Files

```
SmartLab ProMax v10.0.0/
├── SmartlabPromax_10.0.0_universal.dmg          # macOS installer
├── SmartlabPromax_10.0.0_universal.dmg.sig      # macOS signature
├── SmartlabPromax_10.0.0_x64_en-US.msi          # Windows installer
├── SmartlabPromax_10.0.0_x64_en-US.msi.sig      # Windows signature
├── SmartlabPromax-Student_10.0.0_universal.dmg  # Student macOS
├── SmartlabPromax-Student_10.0.0_universal.dmg.sig
├── SmartlabPromax-Student_10.0.0_x64_en-US.msi  # Student Windows
├── SmartlabPromax-Student_10.0.0_x64_en-US.msi.sig
└── latest.json                                   # Update manifest
```

## Troubleshooting

### "Invalid signature" error
- Đảm bảo public key trong tauri.conf.json khớp với private key
- Check GitHub secrets đã setup đúng

### Update không detect
- Check endpoint URL trong tauri.conf.json
- Verify latest.json có trong release
- Check version number format (phải là semantic versioning)

### Build fails
- Check Rust toolchain đã cài đặt
- Verify mediasoup-rust-server build thành công
- Check GitHub Actions logs

## Version Numbering

Sử dụng semantic versioning:
- **Major**: v10.0.0 → v11.0.0 (breaking changes)
- **Minor**: v10.0.0 → v10.1.0 (new features)
- **Patch**: v10.0.0 → v10.0.1 (bug fixes)

## Notes

- Auto-update chỉ hoạt động với signed builds (production)
- Dev builds không có updater
- macOS: Universal binary support cả Intel và Apple Silicon
- Windows: x64 only (có thể thêm ARM64 nếu cần)
