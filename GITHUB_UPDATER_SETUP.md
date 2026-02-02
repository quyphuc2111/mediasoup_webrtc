# 🚀 Setup Auto Updater với GitHub Releases

## Tại sao dùng GitHub?

- ✅ **Miễn phí** - Không tốn chi phí hosting
- ✅ **CDN toàn cầu** - Tốc độ download nhanh
- ✅ **Dễ quản lý** - UI đơn giản, quen thuộc
- ✅ **Tự động** - Tích hợp với GitHub Actions
- ✅ **Bảo mật** - HTTPS mặc định

## Cách hoạt động

1. Build app và tạo update packages
2. Upload lên GitHub Releases
3. Tauri updater tự động check GitHub API
4. Download và install update

## Setup

### Bước 1: Cấu hình tauri.conf.json

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://github.com/YOUR_USERNAME/YOUR_REPO/releases/latest/download/latest.json"
      ],
      "dialog": true,
      "pubkey": "YOUR_PUBLIC_KEY_HERE"
    }
  }
}
```

**Thay thế:**
- `YOUR_USERNAME` → GitHub username của bạn
- `YOUR_REPO` → Tên repository
- `YOUR_PUBLIC_KEY_HERE` → Public key từ `npm run updater:generate-key`

**Ví dụ:**
```json
"endpoints": [
  "https://github.com/zenadev/smartlab-promax/releases/latest/download/latest.json"
]
```

### Bước 2: Tạo GitHub Personal Access Token (Optional)

Nếu repo là private, cần token:

1. Vào GitHub Settings → Developer settings → Personal access tokens
2. Generate new token (classic)
3. Chọn scope: `repo` (Full control of private repositories)
4. Copy token

### Bước 3: Tạo script upload lên GitHub

Tạo file `scripts/release-github.sh`:

```bash
#!/bin/bash

VERSION=$1
NOTES=$2
REPO="YOUR_USERNAME/YOUR_REPO"

if [ -z "$VERSION" ]; then
  echo "Usage: ./scripts/release-github.sh <version> [notes]"
  exit 1
fi

echo "🚀 Creating GitHub Release v$VERSION"

# Build
echo "📦 Building..."
npm run build:teacher

# Sign
echo "🔐 Signing packages..."
npm run updater:sign

# Generate manifest
echo "📝 Generating manifest..."
npm run updater:manifest "$VERSION" "$NOTES"

# Rename manifest to latest.json
cp update-manifest.json latest.json

# Create GitHub release using gh CLI
echo "📤 Creating GitHub release..."
gh release create "v$VERSION" \
  --title "v$VERSION" \
  --notes "$NOTES" \
  src-tauri/target/release/bundle/macos/*.tar.gz \
  src-tauri/target/release/bundle/macos/*.sig \
  latest.json

echo "✅ Release created successfully!"
echo "🔗 https://github.com/$REPO/releases/tag/v$VERSION"
```

### Bước 4: Cài đặt GitHub CLI

```bash
# macOS
brew install gh

# Windows
winget install --id GitHub.cli

# Linux
sudo apt install gh
```

Login:
```bash
gh auth login
```

### Bước 5: Release workflow

```bash
# Make script executable
chmod +x scripts/release-github.sh

# Create release
./scripts/release-github.sh 0.2.0 "Bug fixes and improvements"
```

## Cấu trúc GitHub Release

Mỗi release cần có:

```
v0.2.0/
├── latest.json                                    # Update manifest
├── SmartlabPromax_0.2.0_x64.app.tar.gz           # macOS Intel
├── SmartlabPromax_0.2.0_x64.app.tar.gz.sig       # Signature
├── SmartlabPromax_0.2.0_aarch64.app.tar.gz       # macOS Apple Silicon
├── SmartlabPromax_0.2.0_aarch64.app.tar.gz.sig   # Signature
├── SmartlabPromax_0.2.0_x64-setup.nsis.zip       # Windows
└── SmartlabPromax_0.2.0_x64-setup.nsis.zip.sig   # Signature
```

## Update Manifest cho GitHub

File `latest.json` cần format:

```json
{
  "version": "0.2.0",
  "notes": "Bug fixes and improvements",
  "pub_date": "2024-01-15T12:00:00Z",
  "platforms": {
    "darwin-x86_64": {
      "signature": "SIGNATURE_FROM_SIG_FILE",
      "url": "https://github.com/YOUR_USERNAME/YOUR_REPO/releases/download/v0.2.0/SmartlabPromax_0.2.0_x64.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "SIGNATURE_FROM_SIG_FILE",
      "url": "https://github.com/YOUR_USERNAME/YOUR_REPO/releases/download/v0.2.0/SmartlabPromax_0.2.0_aarch64.app.tar.gz"
    },
    "windows-x86_64": {
      "signature": "SIGNATURE_FROM_SIG_FILE",
      "url": "https://github.com/YOUR_USERNAME/YOUR_REPO/releases/download/v0.2.0/SmartlabPromax_0.2.0_x64-setup.nsis.zip"
    }
  }
}
```

## Script tự động tạo manifest cho GitHub

Tạo `scripts/generate-github-manifest.js`:

```javascript
#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
const repo = process.argv[3] || 'YOUR_USERNAME/YOUR_REPO';
const notes = process.argv[4] || 'Bug fixes and improvements';

if (!version) {
  console.error('Usage: node generate-github-manifest.js <version> [repo] [notes]');
  process.exit(1);
}

const bundleDir = path.join(__dirname, '../src-tauri/target/release/bundle');

const readSignature = (filePath) => {
  try {
    return fs.readFileSync(filePath, 'utf8').trim();
  } catch (err) {
    console.warn(`Warning: ${filePath} not found`);
    return 'SIGNATURE_NOT_FOUND';
  }
};

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {}
};

// macOS x86_64
const macosX64Sig = path.join(bundleDir, 'macos', `SmartlabPromax_${version}_x64.app.tar.gz.sig`);
if (fs.existsSync(macosX64Sig)) {
  manifest.platforms['darwin-x86_64'] = {
    signature: readSignature(macosX64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_x64.app.tar.gz`
  };
}

// macOS aarch64
const macosArm64Sig = path.join(bundleDir, 'macos', `SmartlabPromax_${version}_aarch64.app.tar.gz.sig`);
if (fs.existsSync(macosArm64Sig)) {
  manifest.platforms['darwin-aarch64'] = {
    signature: readSignature(macosArm64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_aarch64.app.tar.gz`
  };
}

// Windows x86_64
const windowsX64Sig = path.join(bundleDir, 'nsis', `SmartlabPromax_${version}_x64-setup.nsis.zip.sig`);
if (fs.existsSync(windowsX64Sig)) {
  manifest.platforms['windows-x86_64'] = {
    signature: readSignature(windowsX64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_x64-setup.nsis.zip`
  };
}

// Write to latest.json
fs.writeFileSync('latest.json', JSON.stringify(manifest, null, 2));

console.log('✅ GitHub manifest generated: latest.json');
console.log(JSON.stringify(manifest, null, 2));
```

## GitHub Actions (Tự động hoàn toàn)

Tạo `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        platform: [macos-latest, windows-latest]
    
    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node
        uses: actions/setup-node@v3
        with:
          node-version: 18
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install dependencies
        run: npm install
      
      - name: Build
        run: npm run build:teacher
      
      - name: Sign updates (macOS)
        if: matrix.platform == 'macos-latest'
        env:
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
        run: |
          echo "$TAURI_PRIVATE_KEY" > private.key
          chmod 600 private.key
          for file in src-tauri/target/release/bundle/macos/*.tar.gz; do
            npx @tauri-apps/cli signer sign private.key "$file"
          done
          rm private.key
      
      - name: Sign updates (Windows)
        if: matrix.platform == 'windows-latest'
        env:
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
        run: |
          echo "$env:TAURI_PRIVATE_KEY" | Out-File -FilePath private.key
          Get-ChildItem src-tauri/target/release/bundle/nsis/*.zip | ForEach-Object {
            npx @tauri-apps/cli signer sign private.key $_.FullName
          }
          Remove-Item private.key
      
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.platform }}-artifacts
          path: |
            src-tauri/target/release/bundle/**/*.tar.gz
            src-tauri/target/release/bundle/**/*.zip
            src-tauri/target/release/bundle/**/*.sig
  
  create-release:
    needs: release
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Download artifacts
        uses: actions/download-artifact@v3
      
      - name: Generate manifest
        run: |
          node scripts/generate-github-manifest.js ${GITHUB_REF#refs/tags/v} ${{ github.repository }} "Release notes"
      
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            **/*.tar.gz
            **/*.zip
            **/*.sig
            latest.json
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Setup GitHub Secrets

1. Vào repo Settings → Secrets and variables → Actions
2. Thêm secret mới:
   - Name: `TAURI_PRIVATE_KEY`
   - Value: Nội dung file `~/.tauri/smartlab.key`

```bash
# Copy private key
cat ~/.tauri/smartlab.key | pbcopy
# Paste vào GitHub Secrets
```

## Workflow với GitHub Actions

```bash
# 1. Commit changes
git add .
git commit -m "Release v0.2.0"

# 2. Create tag
git tag v0.2.0

# 3. Push tag
git push origin v0.2.0

# 4. GitHub Actions tự động:
#    - Build app
#    - Sign packages
#    - Generate manifest
#    - Create release
#    - Upload files
```

## Testing

### Test với GitHub Release

1. Tạo release đầu tiên (v0.1.0)
2. Cài đặt app từ release
3. Tạo release mới (v0.2.0)
4. Mở app v0.1.0
5. AutoUpdater sẽ tự động phát hiện v0.2.0

### Test endpoint

```bash
# Check manifest
curl https://github.com/YOUR_USERNAME/YOUR_REPO/releases/latest/download/latest.json

# Should return JSON with version info
```

## Troubleshooting

### "404 Not Found"
→ Đảm bảo:
- Release đã được published (không phải draft)
- File `latest.json` đã được upload
- URL trong config đúng

### "Invalid signature"
→ Đảm bảo:
- Public key trong tauri.conf.json đúng
- Private key dùng để ký khớp với public key
- Signature files đã được upload

### GitHub Actions failed
→ Check:
- TAURI_PRIVATE_KEY secret đã được thêm
- Dependencies đã được cài đặt đúng
- Build commands đúng

## Best Practices

### 1. Versioning
```bash
# Semantic versioning
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
- Improved UI/UX

### 🐛 Bug Fixes
- Fixed WebRTC connection issues
- Resolved screen capture problems

### ⚡ Improvements
- 30% performance boost
- Reduced memory usage
```

### 3. Pre-releases
```bash
# Create pre-release for testing
git tag v0.2.0-beta.1
git push origin v0.2.0-beta.1

# Mark as pre-release in GitHub
```

### 4. Rollback
```bash
# If update has issues, delete release
gh release delete v0.2.0

# Users will stay on previous version
```

## Advantages của GitHub

✅ **Free** - Không tốn phí
✅ **Fast** - CDN toàn cầu
✅ **Reliable** - 99.9% uptime
✅ **Secure** - HTTPS mặc định
✅ **Easy** - UI đơn giản
✅ **Automated** - GitHub Actions
✅ **Version control** - Git integration

## Kết luận

GitHub Releases là lựa chọn tốt nhất cho auto updater vì:
- Miễn phí và dễ setup
- Tích hợp tốt với workflow development
- CDN nhanh, ổn định
- Tự động hoá với GitHub Actions

Chỉ cần:
1. Cấu hình endpoint trong tauri.conf.json
2. Setup GitHub Actions (optional)
3. Push tag để release

Done! 🚀
