# 🚀 GitHub Actions Build Guide

## ✅ Status: Build Triggered!

Tag `v1.0.0-tcp` đã được push lên GitHub và sẽ trigger build tự động.

## 📍 Theo dõi Build Progress

### Cách 1: GitHub Actions Tab
1. Mở browser và vào: **https://github.com/quyphuc2111/mediasoup_webrtc/actions**
2. Bạn sẽ thấy workflow **"Build Teacher App"** đang chạy
3. Click vào workflow để xem live logs

### Cách 2: Terminal
```bash
# Hoặc mở trực tiếp
open https://github.com/quyphuc2111/mediasoup_webrtc/actions
```

## 📦 Build Matrix

GitHub Actions sẽ build **3 platforms đồng thời**:

| Platform | OS | Output Files |
|----------|----|--------------| 
| 🪟 **Windows** | windows-latest | `.exe`, `.msi` |
| 🐧 **Linux** | ubuntu-22.04 | `.deb`, `.AppImage` |
| 🍎 **macOS** | macos-latest | `.dmg`, `.app` |

## ⏱️ Thời gian dự kiến

- **Windows build**: ~10-15 phút
- **Linux build**: ~10-15 phút  
- **macOS build**: ~15-20 phút

**Total time**: Khoảng 15-20 phút (chạy song song)

## 📥 Download Build Artifacts

### Option A: Draft Release (Recommended)
Sau khi build xong:

1. Vào **Releases**: https://github.com/quyphuc2111/mediasoup_webrtc/releases
2. Tìm draft release **"Teacher App v1.0.0-tcp"**
3. Click Edit nếu cần sửa release notes
4. Download files bạn cần:
   - `ScreenSharing-WebRTC-MediaSoup_1.0.0-tcp_x64_en-US.msi` (Windows installer)
   - `ScreenSharing-WebRTC-MediaSoup_1.0.0-tcp_x64-setup.exe` (Windows setup)
   - `screensharing-webrtc-mediasoup_1.0.0-tcp_amd64.deb` (Ubuntu/Debian)
   - `ScreenSharing-WebRTC-MediaSoup_1.0.0-tcp_x64.dmg` (macOS)

### Option B: Actions Artifacts
Nếu build fail hoặc muốn download trước:

1. Vào Actions tab
2. Click vào workflow run
3. Scroll xuống **Artifacts** section
4. Download artifacts (30 days retention)

## 🔍 Kiểm tra Build Status

### Build Success ✅
Khi tất cả jobs màu xanh:
```
✅ build-teacher (macos-latest)
✅ build-teacher (ubuntu-22.04)
✅ build-teacher (windows-latest)
```

### Build Failed ❌
Nếu có job màu đỏ:
1. Click vào job failed
2. Xem logs để tìm lỗi
3. Thông thường lỗi ở:
   - Dependency installation
   - Rust compilation
   - Tauri bundling

## 🛠️ Troubleshooting

### Build Failed - Dependency Issues
**Windows**: 
- NASM installation failed → Check Chocolatey
- WebView2 missing → Usually auto-installed

**Linux**:
- libwebkit2gtk missing → Check apt-get step
- X11 libs missing → Check system dependencies

**macOS**:
- NASM missing → Check Homebrew
- Code signing issues → Xcode command line tools

### Build Failed - Rust Compilation
Check logs for:
```
error[E0XXX]: ...
```

Common issues:
- Missing dependencies in Cargo.toml
- Platform-specific code errors
- Feature flags not enabled

### Build Success but App Won't Run

**Windows**:
```powershell
# Run from terminal to see errors
.\ScreenSharing-WebRTC-MediaSoup.exe
```

**Linux**:
```bash
# Check dependencies
ldd ./screensharing-webrtc-mediasoup

# Run from terminal
./screensharing-webrtc-mediasoup
```

**macOS**:
```bash
# Remove quarantine
xattr -cr ScreenSharing-WebRTC-MediaSoup.app

# Run from terminal
open ScreenSharing-WebRTC-MediaSoup.app
```

## 🔄 Re-trigger Build

### Method 1: Create new tag
```bash
git tag v1.0.1-tcp
git push origin v1.0.1-tcp
```

### Method 2: Manual dispatch
1. Vào: https://github.com/quyphuc2111/mediasoup_webrtc/actions
2. Click **"Build Teacher App"** workflow
3. Click **"Run workflow"** button
4. Select branch: `main`
5. Click **"Run workflow"**

### Method 3: Re-run failed jobs
1. Vào Actions tab
2. Click vào failed workflow
3. Click **"Re-run failed jobs"** hoặc **"Re-run all jobs"**

## 📊 Current Build

```
Tag: v1.0.0-tcp
Commit: 9a9a19e
Branch: main
Triggered: 2026-01-27 15:43
```

## 🎯 Next Steps

1. ⏳ **Đợi build xong** (~15-20 phút)
2. 📥 **Download .exe** từ Releases
3. 🧪 **Test app** trên Windows
4. ✅ **Publish release** nếu test OK
5. 📢 **Share** với students

## 💡 Tips

- Build logs được giữ 30 ngày
- Draft release có thể edit trước khi publish
- Có thể download từ multiple platforms cùng lúc
- Artifacts tự động attach vào release

## 🔗 Quick Links

- **Actions**: https://github.com/quyphuc2111/mediasoup_webrtc/actions
- **Releases**: https://github.com/quyphuc2111/mediasoup_webrtc/releases
- **Workflow File**: `.github/workflows/release.yml`
- **Tag**: https://github.com/quyphuc2111/mediasoup_webrtc/releases/tag/v1.0.0-tcp

---

**Good luck! 🚀** Build sẽ hoàn thành trong vài phút nữa.
