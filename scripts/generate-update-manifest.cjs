#!/usr/bin/env node

/**
 * Generate update manifest for Tauri updater
 * Usage: node scripts/generate-update-manifest.js <version> <notes>
 */

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
const notes = process.argv[3] || 'Bug fixes and improvements';

if (!version) {
  console.error('Usage: node generate-update-manifest.js <version> [notes]');
  console.error('Example: node generate-update-manifest.js 0.2.0 "New features"');
  process.exit(1);
}

// Read signatures from .sig files
const bundleDir = path.join(__dirname, '../src-tauri/target/release/bundle');

const readSignature = (filePath) => {
  try {
    return fs.readFileSync(filePath, 'utf8').trim();
  } catch (err) {
    console.warn(`Warning: Could not read signature from ${filePath}`);
    return 'SIGNATURE_NOT_FOUND';
  }
};

// Generate manifest
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
    url: `https://your-server.com/updates/SmartlabPromax_${version}_x64.app.tar.gz`
  };
}

// macOS aarch64
const macosArm64Sig = path.join(bundleDir, 'macos', `SmartlabPromax_${version}_aarch64.app.tar.gz.sig`);
if (fs.existsSync(macosArm64Sig)) {
  manifest.platforms['darwin-aarch64'] = {
    signature: readSignature(macosArm64Sig),
    url: `https://your-server.com/updates/SmartlabPromax_${version}_aarch64.app.tar.gz`
  };
}

// Windows x86_64
const windowsX64Sig = path.join(bundleDir, 'nsis', `SmartlabPromax_${version}_x64-setup.nsis.zip.sig`);
if (fs.existsSync(windowsX64Sig)) {
  manifest.platforms['windows-x86_64'] = {
    signature: readSignature(windowsX64Sig),
    url: `https://your-server.com/updates/SmartlabPromax_${version}_x64-setup.nsis.zip`
  };
}

// Linux x86_64
const linuxX64Sig = path.join(bundleDir, 'appimage', `SmartlabPromax_${version}_amd64.AppImage.tar.gz.sig`);
if (fs.existsSync(linuxX64Sig)) {
  manifest.platforms['linux-x86_64'] = {
    signature: readSignature(linuxX64Sig),
    url: `https://your-server.com/updates/SmartlabPromax_${version}_amd64.AppImage.tar.gz`
  };
}

// Write manifest
const outputPath = path.join(__dirname, '../update-manifest.json');
fs.writeFileSync(outputPath, JSON.stringify(manifest, null, 2));

console.log('✅ Update manifest generated:');
console.log(JSON.stringify(manifest, null, 2));
console.log(`\n📝 Saved to: ${outputPath}`);
console.log('\n⚠️  Remember to:');
console.log('1. Update URLs to your actual server');
console.log('2. Upload update packages and signatures');
console.log('3. Deploy manifest to your update server');
