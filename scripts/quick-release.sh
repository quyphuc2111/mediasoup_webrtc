#!/bin/bash

# Quick release - use existing npm scripts
# Usage: ./scripts/quick-release.sh <version> <repo> [notes]

VERSION=$1
REPO=$2
NOTES=${3:-"Bug fixes and improvements"}

if [ -z "$VERSION" ] || [ -z "$REPO" ]; then
  echo "Usage: ./scripts/quick-release.sh <version> <repo> [notes]"
  echo "Example: ./scripts/quick-release.sh 0.1.0 quyphuc2111/mediasoup_webrtc \"Initial release\""
  exit 1
fi

echo "🚀 Quick Release v$VERSION"
echo "📦 Repository: $REPO"
echo ""

# Check gh CLI
if ! gh auth status &> /dev/null; then
  echo "❌ Not logged in to GitHub. Run: gh auth login"
  exit 1
fi

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  echo "📱 Apple Silicon detected"
  ARCH_NAME="aarch64"
  TARGET="aarch64-apple-darwin"
elif [ "$ARCH" = "x86_64" ]; then
  echo "💻 Intel detected"
  ARCH_NAME="x64"
  TARGET="x86_64-apple-darwin"
else
  echo "❌ Unknown architecture: $ARCH"
  exit 1
fi

# Build
echo "📦 Building for $TARGET..."
# Set private key from file for Tauri build
PRIVATE_KEY=$(cat ~/.tauri/smartlab.key)
export TAURI_SIGNING_PRIVATE_KEY="$PRIVATE_KEY"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="Zenadev@123@"

npm run prepare:binaries && npm run build && npm run tauri build -- --target $TARGET

if [ $? -ne 0 ]; then
  echo "❌ Build failed"
  exit 1
fi

# Unset for security
unset TAURI_SIGNING_PRIVATE_KEY
unset TAURI_SIGNING_PRIVATE_KEY_PASSWORD

# Find the .app bundle
BUNDLE_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP_FILE=$(ls -d $BUNDLE_DIR/*.app 2>/dev/null | head -n 1)

if [ -z "$APP_FILE" ]; then
  echo "❌ No .app bundle found in $BUNDLE_DIR"
  exit 1
fi

echo "✅ Found: $(basename "$APP_FILE")"

# Check if Tauri already created tar.gz and signature
TAURI_TAR=$(ls $BUNDLE_DIR/*.app.tar.gz 2>/dev/null | head -n 1)
TAURI_SIG=$(ls $BUNDLE_DIR/*.app.tar.gz.sig 2>/dev/null | head -n 1)

if [ -n "$TAURI_TAR" ] && [ -n "$TAURI_SIG" ]; then
  echo "✅ Tauri already created update package and signature"
  
  # Rename to include version and arch
  TAR_NAME="SmartlabPromax_${VERSION}_${ARCH_NAME}.app.tar.gz"
  SIG_NAME="SmartlabPromax_${VERSION}_${ARCH_NAME}.app.tar.gz.sig"
  
  cp "$TAURI_TAR" "$BUNDLE_DIR/$TAR_NAME"
  cp "$TAURI_SIG" "$BUNDLE_DIR/$SIG_NAME"
  
  echo "✅ Renamed to: $TAR_NAME"
else
  echo "⚠️  Tauri didn't create update package, creating manually..."
  
  # Create tar.gz for updater
  TAR_NAME="SmartlabPromax_${VERSION}_${ARCH_NAME}.app.tar.gz"
  cd "$BUNDLE_DIR"
  tar -czf "$TAR_NAME" "$(basename "$APP_FILE")"
  cd - > /dev/null
  
  echo "✅ Created: $TAR_NAME"
  
  # Sign manually
  echo "🔐 Signing..."
  npx @tauri-apps/cli signer sign ~/.tauri/smartlab.key "$BUNDLE_DIR/$TAR_NAME"
  
  if [ $? -ne 0 ]; then
    echo "❌ Signing failed"
    exit 1
  fi
  
  SIG_NAME="$TAR_NAME.sig"
fi

# Copy to standard location for manifest generation
mkdir -p src-tauri/target/release/bundle/macos
cp "$BUNDLE_DIR/$TAR_NAME" src-tauri/target/release/bundle/macos/
cp "$BUNDLE_DIR/$SIG_NAME" src-tauri/target/release/bundle/macos/

# Generate manifest
echo "📝 Generating manifest..."
node scripts/generate-github-manifest.cjs "$VERSION" "$REPO" "$NOTES"

if [ $? -ne 0 ]; then
  echo "❌ Manifest generation failed"
  exit 1
fi

# Upload
echo "📤 Creating release..."
gh release create "v$VERSION" \
  --repo "$REPO" \
  --title "v$VERSION" \
  --notes "$NOTES" \
  latest.json \
  "$BUNDLE_DIR/$TAR_NAME" \
  "$BUNDLE_DIR/$SIG_NAME"

if [ $? -eq 0 ]; then
  echo ""
  echo "✅ Release created successfully!"
  echo "🔗 https://github.com/$REPO/releases/tag/v$VERSION"
  echo ""
  echo "📦 Files uploaded:"
  echo "  - latest.json"
  echo "  - $TAR_NAME"
  echo "  - $SIG_NAME"
else
  echo "❌ Release failed"
  exit 1
fi
