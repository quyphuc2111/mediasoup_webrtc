#!/bin/bash

# Sign all update packages
# Usage: ./scripts/sign-updates.sh [key-path]

KEY_PATH="${1:-$HOME/.tauri/smartlab.key}"

if [ ! -f "$KEY_PATH" ]; then
  echo "❌ Private key not found at: $KEY_PATH"
  echo "Generate a key first with: npm run tauri signer generate -- -w $KEY_PATH"
  exit 1
fi

echo "🔑 Using private key: $KEY_PATH"
echo ""

BUNDLE_DIR="src-tauri/target/release/bundle"

# Sign macOS packages
if [ -d "$BUNDLE_DIR/macos" ]; then
  echo "📦 Signing macOS packages..."
  for file in "$BUNDLE_DIR/macos"/*.tar.gz; do
    if [ -f "$file" ]; then
      echo "  Signing: $(basename "$file")"
      npm run tauri signer sign "$KEY_PATH" "$file"
    fi
  done
fi

# Sign Windows packages
if [ -d "$BUNDLE_DIR/nsis" ]; then
  echo "📦 Signing Windows packages..."
  for file in "$BUNDLE_DIR/nsis"/*.zip; do
    if [ -f "$file" ]; then
      echo "  Signing: $(basename "$file")"
      npm run tauri signer sign "$KEY_PATH" "$file"
    fi
  done
fi

# Sign Linux packages
if [ -d "$BUNDLE_DIR/appimage" ]; then
  echo "📦 Signing Linux packages..."
  for file in "$BUNDLE_DIR/appimage"/*.tar.gz; do
    if [ -f "$file" ]; then
      echo "  Signing: $(basename "$file")"
      npm run tauri signer sign "$KEY_PATH" "$file"
    fi
  done
fi

echo ""
echo "✅ All packages signed!"
echo ""
echo "Next steps:"
echo "1. Generate manifest: node scripts/generate-update-manifest.js <version>"
echo "2. Upload packages and signatures to your server"
echo "3. Deploy manifest to update endpoint"
