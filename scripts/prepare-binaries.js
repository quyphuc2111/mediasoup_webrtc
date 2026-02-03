// Script to prepare Rust mediasoup server binary for Tauri bundle
// Copies the compiled Rust binary to the correct location

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.join(__dirname, '..');

// Detect OS and architecture
const platform = process.platform;
const arch = process.arch;
const isWindows = platform === 'win32';
const isMacOS = platform === 'darwin';
const isLinux = platform === 'linux';

console.log(`🖥️  Detected OS: ${platform}, Arch: ${arch}`);

// Paths
const rustServerDir = path.join(rootDir, 'mediasoup-rust-server');
const binariesDir = path.join(rootDir, 'src-tauri', 'binaries');
const serverBinDir = path.join(binariesDir, 'server');

// Ensure directories exist
function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
    console.log(`✅ Created directory: ${dir}`);
  }
}

// Get Tauri target triple
function getTargetTriple() {
  if (isWindows) {
    return 'x86_64-pc-windows-msvc';
  } else if (isMacOS) {
    // For universal builds, we need both architectures
    if (arch === 'arm64') {
      return 'aarch64-apple-darwin';
    }
    return 'x86_64-apple-darwin';
  } else if (isLinux) {
    return 'x86_64-unknown-linux-gnu';
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

// Get binary extension
function getBinaryExtension() {
  return isWindows ? '.exe' : '';
}

// Copy Rust server binary
function copyRustServerBinary() {
  console.log('\n📋 Preparing mediasoup-rust-server for bundle...');
  
  const ext = getBinaryExtension();
  const targetTriple = getTargetTriple();
  
  // Check for custom CARGO_TARGET_DIR (used in CI for Windows path length issues)
  const customTargetDir = process.env.CARGO_TARGET_DIR;
  
  // Source paths to check (in order of priority)
  const possibleSourcePaths = [];
  
  if (customTargetDir) {
    // Custom target dir (CI Windows)
    possibleSourcePaths.push(path.join(customTargetDir, 'release', `mediasoup-rust-server${ext}`));
  }
  
  // Default: mediasoup-rust-server/target/release/mediasoup-rust-server(.exe)
  possibleSourcePaths.push(path.join(rustServerDir, 'target', 'release', `mediasoup-rust-server${ext}`));
  
  // Find the first existing binary
  let sourceBinaryPath = null;
  for (const p of possibleSourcePaths) {
    console.log(`   🔍 Checking: ${p}`);
    if (fs.existsSync(p)) {
      sourceBinaryPath = p;
      console.log(`   ✅ Found binary at: ${p}`);
      break;
    }
  }
  
  if (!sourceBinaryPath) {
    console.error(`❌ Rust server binary not found at any of these locations:`);
    possibleSourcePaths.forEach(p => console.error(`   - ${p}`));
    console.error('   Please build it first with: cd mediasoup-rust-server && cargo build --release');
    return false;
  }
  
  // Target: src-tauri/binaries/server/mediasoup-rust-server-{target-triple}(.exe)
  // Tauri expects binaries named with target triple suffix
  const targetBinaryName = `mediasoup-rust-server-${targetTriple}${ext}`;
  const targetBinaryPath = path.join(serverBinDir, targetBinaryName);
  
  // Also copy without target triple for backwards compatibility
  const targetBinaryPathSimple = path.join(serverBinDir, `mediasoup-rust-server${ext}`);
  
  // Ensure target directory exists
  ensureDir(serverBinDir);
  
  // Copy binary with target triple name (for Tauri sidecar)
  fs.copyFileSync(sourceBinaryPath, targetBinaryPath);
  console.log(`   ✅ Copied to ${targetBinaryPath}`);
  
  // Also copy without target triple (for direct use)
  fs.copyFileSync(sourceBinaryPath, targetBinaryPathSimple);
  console.log(`   ✅ Copied to ${targetBinaryPathSimple}`);
  
  // Make binary executable on Unix
  if (!isWindows) {
    try {
      fs.chmodSync(targetBinaryPath, '755');
      fs.chmodSync(targetBinaryPathSimple, '755');
      console.log(`   ✅ Made binaries executable`);
    } catch (error) {
      console.warn(`   ⚠️  Could not make binary executable: ${error.message}`);
    }
  }
  
  // Get file size
  const stats = fs.statSync(targetBinaryPath);
  const sizeMB = (stats.size / (1024 * 1024)).toFixed(2);
  console.log(`   📦 Binary size: ${sizeMB} MB`);
  
  return true;
}

// For macOS universal builds, copy both architectures if available
function copyMacOSUniversalBinaries() {
  if (!isMacOS) return true;
  
  console.log('\n📋 Checking for macOS universal build binaries...');
  
  const architectures = [
    { arch: 'x86_64-apple-darwin', folder: 'x86_64-apple-darwin' },
    { arch: 'aarch64-apple-darwin', folder: 'aarch64-apple-darwin' },
  ];
  
  let copiedCount = 0;
  
  for (const { arch: targetArch, folder } of architectures) {
    // Check for cross-compiled binary
    const crossCompiledPath = path.join(rustServerDir, 'target', folder, 'release', 'mediasoup-rust-server');
    const targetPath = path.join(serverBinDir, `mediasoup-rust-server-${targetArch}`);
    
    if (fs.existsSync(crossCompiledPath)) {
      fs.copyFileSync(crossCompiledPath, targetPath);
      fs.chmodSync(targetPath, '755');
      console.log(`   ✅ Copied ${targetArch} binary`);
      copiedCount++;
    }
  }
  
  if (copiedCount > 0) {
    console.log(`   📦 Copied ${copiedCount} architecture-specific binaries`);
  }
  
  return true;
}

// Main function
function main() {
  console.log('🚀 Preparing Rust mediasoup server binary for Tauri bundle...\n');
  
  // Ensure directories exist
  ensureDir(binariesDir);
  ensureDir(serverBinDir);
  
  // Copy Rust server binary
  const copySuccess = copyRustServerBinary();
  
  if (!copySuccess) {
    console.error('\n❌ Failed to copy Rust server binary');
    process.exit(1);
  }
  
  // For macOS, also try to copy universal binaries
  copyMacOSUniversalBinaries();
  
  console.log('\n✅ Rust server binary prepared successfully!');
}

main();
