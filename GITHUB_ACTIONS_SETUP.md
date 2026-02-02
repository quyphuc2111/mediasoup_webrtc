# 🤖 GitHub Actions Auto Release Setup

## Tổng quan

GitHub Actions sẽ tự động build cho cả macOS (Intel + Apple Silicon) và Windows khi bạn push tag.

## Setup (Chỉ làm 1 lần)

### Bước 1: Thêm Private Key vào GitHub Secrets

1. Copy private key:
```bash
cat ~/.tauri/smartlab.key | pbcopy
```

2. Vào GitHub repo:
```
https://github.com/quyphuc2111/mediasoup_webrtc/settings/secrets/actions
```

3. Click "New repository secret"

4. Điền:
   - **Name**: `TAURI_PRIVATE_KEY`
   - **Value**: Paste private key từ clipboard
   
5. Click "Add secret"

### Bước 2: Verify Workflow File

File `.github/workflows/release.yml` đã được tạo và sẽ:
- ✅ Build macOS (Intel + Apple Silicon)
- ✅ Build Windows
- ✅ Sign tất cả packages
- ✅ Generate manifest
- ✅ Create GitHub release
- ✅ Upload files

## Cách sử dụng

### Release mới

```bash
# 1. Commit changes
git add .
git commit -m "Release v0.2.0"

# 2. Update version trong tauri.conf.json
# Edit: "version": "0.2.0"

# 3. Create and push tag
git tag v0.2.0
git push origin v0.2.0
```

**Xong!** GitHub Actions sẽ tự động:
1. Build cho macOS (Intel + Apple Silicon)
2. Build cho Windows
3. Sign tất cả packages
4. Create release
5. Upload files

### Theo dõi build

1. Vào: https://github.com/quyphuc2111/mediasoup_webrtc/actions
2. Click vào workflow run mới nhất
3. Xem progress của từng job

### Kết quả

Sau ~15-20 phút, release sẽ có:

```
v0.2.0/
├── latest.json                                    # Manifest
├── SmartlabPromax_0.2.0_aarch64.app.tar.gz       # macOS Apple Silicon
├── SmartlabPromax_0.2.0_aarch64.app.tar.gz.sig   # Signature
├── SmartlabPromax_0.2.0_x64.app.tar.gz           # macOS Intel
├── SmartlabPromax_0.2.0_x64.app.tar.gz.sig       # Signature
├── SmartlabPromax_0.2.0_x64-setup.nsis.zip       # Windows
└── SmartlabPromax_0.2.0_x64-setup.nsis.zip.sig   # Signature
```

## Workflow Details

### Build Matrix

```yaml
matrix:
  include:
    - platform: macos-latest
      target: aarch64-apple-darwin    # Apple Silicon
    - platform: macos-latest
      target: x86_64-apple-darwin     # Intel
    - platform: windows-latest
      target: x86_64-pc-windows-msvc  # Windows
```

### Build Steps

1. **Checkout code**
2. **Setup Node.js 18**
3. **Setup Rust** với target cụ thể
4. **Install dependencies** (npm + mediasoup-server)
5. **Build mediasoup-server**
6. **Prepare binaries**
7. **Build frontend**
8. **Build Tauri app** với signing
9. **Rename artifacts** với version và arch
10. **Upload artifacts**

### Release Step

1. **Download tất cả artifacts**
2. **Organize files**
3. **Generate manifest**
4. **Create GitHub release** với files và release notes

## Troubleshooting

### Build failed

**Check logs:**
```
https://github.com/quyphuc2111/mediasoup_webrtc/actions
```

**Common issues:**

1. **Missing TAURI_PRIVATE_KEY**
   → Add secret theo Bước 1

2. **Build timeout**
   → Bình thường, GitHub Actions có thể chậm
   → Retry workflow

3. **Signing failed**
   → Check private key format
   → Đảm bảo không có newline thừa

### Release không có files

→ Check artifacts trong workflow run
→ Verify upload step succeeded

### Manifest không đúng

→ Check generate-github-manifest.cjs
→ Verify file paths trong organize step

## Manual Release (Backup)

Nếu GitHub Actions không work, dùng script local:

```bash
./scripts/quick-release.sh 0.2.0 quyphuc2111/mediasoup_webrtc "Release notes"
```

## Comparison

| Method | macOS Intel | macOS ARM | Windows | Time | Effort |
|--------|-------------|-----------|---------|------|--------|
| **GitHub Actions** | ✅ | ✅ | ✅ | ~20min | Low |
| **Local Script** | ❌ | ✅ | ❌ | ~5min | Medium |

## Best Practice

1. **Test locally first** với quick-release.sh
2. **Push tag** để trigger GitHub Actions
3. **Monitor build** trong Actions tab
4. **Verify release** có đủ files
5. **Test update** từ previous version

## Security

- ✅ Private key stored in GitHub Secrets (encrypted)
- ✅ Only accessible during workflow runs
- ✅ Never exposed in logs
- ✅ Automatically cleaned after build

## Next Steps

1. ✅ Add TAURI_PRIVATE_KEY secret
2. ✅ Push a tag to test
3. ✅ Monitor first build
4. ✅ Verify release files
5. ✅ Test auto update

## Resources

- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [Tauri CI/CD Guide](https://tauri.app/v1/guides/building/cross-platform)
- [Workflow File](./.github/workflows/release.yml)

## Done! 🎉

Bây giờ chỉ cần push tag là có release tự động cho cả macOS và Windows!

```bash
git tag v0.2.0
git push origin v0.2.0
# Chờ ~20 phút → Release ready! 🚀
```
