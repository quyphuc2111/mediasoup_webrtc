#!/bin/bash

# Release to GitHub
# Usage: ./scripts/release-github.sh <version> <repo> [notes]

VERSION=$1
REPO=$2
NOTES=${3:-"Bug fixes and improvements"}

if [ -z "$VERSION" ] || [ -z "$REPO" ]; then
  echo "Usage: ./scripts/release-github.sh <version> <repo> [notes]"
  echo "Example: ./scripts/release-github.sh 0.2.0 zenadev/smartlab-promax \"New features\""
  exit 1
fi

echo "🚀 Creating GitHub Release v$VERSION"
echo "📦 Repository: $REPO"
echo ""

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
  echo "❌ GitHub CLI (gh) is not installed"
  echo "Install: brew install gh (macOS) or visit https://cli.github.com/"
  exit 1
fi

# Check if logged in
if ! gh auth status &> /dev/null; then
  echo "❌ Not logged in to GitHub"
  echo "Run: gh auth login"
  exit 1
fi

# Build
echo "📦 Building..."
echo "🔧 Detecting architecture..."
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  echo "📱 Building for Apple Silicon (aarch64)..."
  npm run prepare:binaries
  npm run build
  npm run tauri build -- --target aarch64-apple-darwin
elif [ "$ARCH" = "x86_64" ]; then
  echo "💻 Building for Intel (x86_64)..."
  npm run prepare:binaries
  npm run build
  npm run tauri build -- --target x86_64-apple-darwin
else
  echo "⚠️  Unknown architecture: $ARCH, building default..."
  npm run build:teacher
fi

if [ $? -ne 0 ]; then
  echo "❌ Build failed"
  exit 1
fi

# Sign
echo "🔐 Signing packages..."
npm run updater:sign
if [ $? -ne 0 ]; then
  echo "❌ Signing failed"
  exit 1
fi

# Generate manifest
echo "📝 Generating manifest..."
node scripts/generate-github-manifest.cjs "$VERSION" "$REPO" "$NOTES"
if [ $? -ne 0 ]; then
  echo "❌ Manifest generation failed"
  exit 1
fi

# Collect files to upload
FILES=()
FILES+=("latest.json")

# macOS packages
if [ -d "src-tauri/target/release/bundle/macos" ]; then
  for file in src-tauri/target/release/bundle/macos/*.tar.gz; do
    [ -f "$file" ] && FILES+=("$file")
  done
  for file in src-tauri/target/release/bundle/macos/*.sig; do
    [ -f "$file" ] && FILES+=("$file")
  done
fi

# Windows packages
if [ -d "src-tauri/target/release/bundle/nsis" ]; then
  for file in src-tauri/target/release/bundle/nsis/*.zip; do
    [ -f "$file" ] && FILES+=("$file")
  done
  for file in src-tauri/target/release/bundle/nsis/*.sig; do
    [ -f "$file" ] && FILES+=("$file")
  done
fi

# Linux packages
if [ -d "src-tauri/target/release/bundle/appimage" ]; then
  for file in src-tauri/target/release/bundle/appimage/*.tar.gz; do
    [ -f "$file" ] && FILES+=("$file")
  done
  for file in src-tauri/target/release/bundle/appimage/*.sig; do
    [ -f "$file" ] && FILES+=("$file")
  done
fi

echo ""
echo "📤 Files to upload:"
for file in "${FILES[@]}"; do
  echo "  - $(basename "$file")"
done
echo ""

# Create release
echo "📤 Creating GitHub release..."
gh release create "v$VERSION" \
  --repo "$REPO" \
  --title "v$VERSION" \
  --notes "$NOTES" \
  "${FILES[@]}"

if [ $? -eq 0 ]; then
  echo ""
  echo "✅ Release created successfully!"
  echo "🔗 https://github.com/$REPO/releases/tag/v$VERSION"
  echo ""
  echo "📝 Update endpoint in tauri.conf.json:"
  echo "   https://github.com/$REPO/releases/latest/download/latest.json"
else
  echo "❌ Failed to create release"
  exit 1
fi
