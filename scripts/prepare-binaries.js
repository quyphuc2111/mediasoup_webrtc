// Script to prepare binaries for Tauri bundle
// - Build mediasoup-server
// - Copy mediasoup-server binary based on OS
// - Bundle Node.js (optional, can use system Node.js)

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.join(__dirname, '..');

// Detect OS
const platform = process.platform;
const isWindows = platform === 'win32';
const isMacOS = platform === 'darwin';
const isLinux = platform === 'linux';

console.log(`🖥️  Detected OS: ${platform}`);

// Paths
const serverDir = path.join(rootDir, 'mediasoup-server');
const binariesDir = path.join(rootDir, 'src-tauri', 'binaries');
const serverBinDir = path.join(binariesDir, 'server');
const serverDistDir = path.join(serverBinDir, 'dist');
const nodeBinDir = path.join(binariesDir, 'node');

// Ensure directories exist
function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
    console.log(`✅ Created directory: ${dir}`);
  }
}

// Get mediasoup-server binary name based on OS
function getServerBinaryName() {
  if (isWindows) {
    return 'mediasoup-server-win.exe';
  } else if (isMacOS) {
    return 'mediasoup-server-macos';
  } else if (isLinux) {
    return 'mediasoup-server-linux';
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

// Build mediasoup-server
function buildServer() {
  console.log('\n📦 Building mediasoup-server...');
  
  try {
    // Build TypeScript
    console.log('   → Compiling TypeScript...');
    execSync('npm run build', { cwd: serverDir, stdio: 'inherit' });
    
    console.log('✅ mediasoup-server built successfully');
    return true;
  } catch (error) {
    console.error('❌ Failed to build mediasoup-server:', error.message);
    return false;
  }
}

// Copy mediasoup-server binary or dist
function copyServerBinary() {
  console.log('\n📋 Preparing mediasoup-server for bundle...');
  
  const binaryName = getServerBinaryName();
  const sourceBinaryPath = path.join(binariesDir, binaryName);
  const distSourcePath = path.join(serverDir, 'dist', 'index.js');
  const targetDistPath = path.join(serverDistDir, 'index.js');
  
  // Ensure target directory exists
  ensureDir(serverDistDir);
  
  // Priority 1: Copy dist/index.js (preferred for bundling with Node.js)
  if (fs.existsSync(distSourcePath)) {
    fs.copyFileSync(distSourcePath, targetDistPath);
    console.log(`   ✅ Copied dist/index.js to ${targetDistPath}`);
    
    // Also copy the binary to server root for reference
    if (fs.existsSync(sourceBinaryPath)) {
      const targetBinaryPath = path.join(serverBinDir, binaryName);
      fs.copyFileSync(sourceBinaryPath, targetBinaryPath);
      console.log(`   ✅ Copied binary ${binaryName} to ${targetBinaryPath}`);
      
      // Make binary executable on Unix
      if (!isWindows && fs.existsSync(targetBinaryPath)) {
        try {
          fs.chmodSync(targetBinaryPath, '755');
          console.log(`   ✅ Made binary executable`);
        } catch (error) {
          console.warn(`   ⚠️  Could not make binary executable: ${error.message}`);
        }
      }
    } else {
      console.warn(`   ⚠️  Binary not found: ${sourceBinaryPath}`);
      console.warn(`      You may need to build it using: cd mediasoup-server && npm run pkg`);
    }
    
    return true;
  } else {
    console.error(`   ❌ dist/index.js not found. Please build mediasoup-server first.`);
    return false;
  }
}

// Copy mediasoup-server node_modules (needed for mediasoup native modules)
function copyNodeModules() {
  console.log('\n📦 Copying mediasoup-server node_modules...');
  
  const sourceNodeModules = path.join(serverDir, 'node_modules');
  const targetNodeModules = path.join(serverDistDir, 'node_modules');
  
  if (!fs.existsSync(sourceNodeModules)) {
    console.warn('⚠️  node_modules not found, installing...');
    try {
      execSync('npm install', { cwd: serverDir, stdio: 'inherit' });
    } catch (error) {
      console.error('❌ Failed to install node_modules:', error.message);
      return false;
    }
  }
  
  // Copy only necessary node_modules (mediasoup requires native modules)
  const requiredModules = ['mediasoup', 'ws', 'uuid'];
  
  ensureDir(targetNodeModules);
  
  for (const module of requiredModules) {
    const sourceModule = path.join(sourceNodeModules, module);
    const targetModule = path.join(targetNodeModules, module);
    
    if (fs.existsSync(sourceModule)) {
      if (fs.existsSync(targetModule)) {
        fs.rmSync(targetModule, { recursive: true, force: true });
      }
      fs.cpSync(sourceModule, targetModule, { recursive: true });
      console.log(`   ✅ Copied ${module}`);
    } else {
      console.warn(`   ⚠️  Module not found: ${module}`);
    }
  }
  
  console.log('✅ Node modules copied');
  return true;
}

// Main function
function main() {
  console.log('🚀 Preparing binaries for Tauri bundle...\n');
  
  // Ensure directories exist
  ensureDir(binariesDir);
  ensureDir(serverBinDir);
  ensureDir(serverDistDir);
  
  // Build mediasoup-server
  const buildSuccess = buildServer();
  
  if (!buildSuccess) {
    console.error('\n❌ Failed to build mediasoup-server');
    process.exit(1);
  }
  
  // Copy server binary/dist
  const copySuccess = copyServerBinary();
  
  if (!copySuccess) {
    console.error('\n❌ Failed to copy server binary');
    process.exit(1);
  }
  
  // Copy node_modules
  copyNodeModules();
  
  console.log('\n✅ All binaries prepared successfully!');
  console.log('\n📝 Note: Node.js will use system version if not bundled.');
  console.log('   To bundle Node.js, download it and place at:');
  if (isWindows) {
    console.log('   src-tauri/binaries/node/node.exe');
  } else {
    console.log('   src-tauri/binaries/node/bin/node (or node)');
  }
}

main();
