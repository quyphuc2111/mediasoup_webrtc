#!/usr/bin/env node

/**
 * Generate update manifest for GitHub Releases
 * Usage: node scripts/generate-github-manifest.js <version> <repo> [notes]
 */

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
const repo = process.argv[3] || 'YOUR_USERNAME/YOUR_REPO';
const notes = process.argv[4] || 'Bug fixes and improvements';

if (!version || repo === 'YOUR_USERNAME/YOUR_REPO') {
  console.error('Usage: node generate-github-manifest.js <version> <repo> [notes]');
  console.error('Example: node generate-github-manifest.js 0.2.0 zenadev/smartlab-promax "New features"');
  process.exit(1);
}

// Read signatures from .sig files
const bundleDir = path.join(__dirname, '../src-tauri/target/release/bundle');

const readSignature = (filePath) => {
  try {
    return fs.readFileSync(filePath, 'utf8').trim();
  } catch (err) {
    console.warn(`⚠️  Warning: Could not read signature from ${filePath}`);
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

let foundPlatforms = 0;

// macOS x86_64
const macosX64Sig = path.join(bundleDir, 'macos', `SmartlabPromax_${version}_x64.app.tar.gz.sig`);
if (fs.existsSync(macosX64Sig)) {
  manifest.platforms['darwin-x86_64'] = {
    signature: readSignature(macosX64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_x64.app.tar.gz`
  };
  foundPlatforms++;
  console.log('✅ Found macOS x86_64 package');
}

// macOS aarch64
const macosArm64Sig = path.join(bundleDir, 'macos', `SmartlabPromax_${version}_aarch64.app.tar.gz.sig`);
if (fs.existsSync(macosArm64Sig)) {
  manifest.platforms['darwin-aarch64'] = {
    signature: readSignature(macosArm64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_aarch64.app.tar.gz`
  };
  foundPlatforms++;
  console.log('✅ Found macOS aarch64 package');
}

// Windows x86_64
const windowsX64Sig = path.join(bundleDir, 'nsis', `SmartlabPromax_${version}_x64-setup.nsis.zip.sig`);
if (fs.existsSync(windowsX64Sig)) {
  manifest.platforms['windows-x86_64'] = {
    signature: readSignature(windowsX64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_x64-setup.nsis.zip`
  };
  foundPlatforms++;
  console.log('✅ Found Windows x86_64 package');
}

// Linux x86_64
const linuxX64Sig = path.join(bundleDir, 'appimage', `SmartlabPromax_${version}_amd64.AppImage.tar.gz.sig`);
if (fs.existsSync(linuxX64Sig)) {
  manifest.platforms['linux-x86_64'] = {
    signature: readSignature(linuxX64Sig),
    url: `https://github.com/${repo}/releases/download/v${version}/SmartlabPromax_${version}_amd64.AppImage.tar.gz`
  };
  foundPlatforms++;
  console.log('✅ Found Linux x86_64 package');
}

if (foundPlatforms === 0) {
  console.error('❌ No signed packages found!');
  console.error('Run: npm run updater:sign');
  process.exit(1);
}

// Write manifest to latest.json (GitHub convention)
const outputPath = path.join(__dirname, '../latest.json');
fs.writeFileSync(outputPath, JSON.stringify(manifest, null, 2));

console.log('\n✅ GitHub manifest generated: latest.json');
console.log(`📦 Platforms: ${foundPlatforms}`);
console.log(`🔗 Repo: https://github.com/${repo}`);
console.log(`📝 Version: v${version}`);
console.log('\n📋 Manifest:');
console.log(JSON.stringify(manifest, null, 2));
console.log('\n⚠️  Next steps:');
console.log('1. Create GitHub release:');
console.log(`   gh release create v${version} --title "v${version}" --notes "${notes}" latest.json src-tauri/target/release/bundle/**/*.tar.gz src-tauri/target/release/bundle/**/*.sig`);
console.log('2. Or use: ./scripts/release-github.sh');
