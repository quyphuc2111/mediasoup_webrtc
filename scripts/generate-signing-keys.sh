#!/bin/bash

# Script to generate Tauri signing keys for auto-update

echo "🔐 Generating Tauri Signing Keys for Auto-Update"
echo "================================================"
echo ""

# Check if tauri CLI is installed
if ! command -v cargo-tauri &> /dev/null; then
    echo "❌ Tauri CLI not found. Installing..."
    npm install -g @tauri-apps/cli
fi

# Create .tauri directory if not exists
mkdir -p ~/.tauri

# Generate keypair
echo "📝 Generating keypair..."
echo ""
npm run tauri signer generate -w ~/.tauri/smartlab.key

echo ""
echo "✅ Keys generated successfully!"
echo ""
echo "📋 Next steps:"
echo "1. Copy the PUBLIC KEY above"
echo "2. Update src-tauri/tauri.conf.json → plugins.updater.pubkey"
echo "3. Setup GitHub Secrets:"
echo "   - TAURI_SIGNING_PRIVATE_KEY: Run 'cat ~/.tauri/smartlab.key'"
echo "   - TAURI_SIGNING_PRIVATE_KEY_PASSWORD: Leave empty (if no password)"
echo ""
echo "⚠️  IMPORTANT: Backup ~/.tauri/smartlab.key to a safe place!"
echo "   Losing this key means you cannot release updates!"
echo ""
