#!/usr/bin/env node

/**
 * Simple update server for testing Tauri updater
 * Usage: node scripts/update-server-example.js
 * 
 * This server serves update manifests and packages for testing.
 * In production, use a proper CDN or static file hosting.
 */

const express = require('express');
const cors = require('cors');
const path = require('path');
const fs = require('fs');

const app = express();
const PORT = process.env.PORT || 3030;

// Enable CORS for all origins (for testing only!)
app.use(cors());

// Serve static files from updates directory
app.use('/updates', express.static(path.join(__dirname, '../updates')));

// Update manifest endpoint
// Format: /{target}/{arch}/{current_version}
app.get('/:target/:arch/:version', (req, res) => {
  const { target, arch, version } = req.params;
  
  console.log(`📡 Update check: ${target}/${arch} v${version}`);
  
  // Load manifest
  const manifestPath = path.join(__dirname, '../update-manifest.json');
  
  if (!fs.existsSync(manifestPath)) {
    console.log('❌ No manifest found');
    return res.status(204).send(); // No update available
  }
  
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  
  // Check if update is available
  if (manifest.version === version) {
    console.log('✅ Already up to date');
    return res.status(204).send(); // No update available
  }
  
  // Get platform-specific info
  const platformKey = `${target}-${arch}`;
  const platformInfo = manifest.platforms[platformKey];
  
  if (!platformInfo) {
    console.log(`❌ No update for platform: ${platformKey}`);
    return res.status(204).send();
  }
  
  // Return update info
  const response = {
    version: manifest.version,
    notes: manifest.notes,
    pub_date: manifest.pub_date,
    platforms: {
      [platformKey]: platformInfo
    }
  };
  
  console.log(`✅ Update available: v${manifest.version}`);
  res.json(response);
});

// Health check
app.get('/health', (req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

// Start server
app.listen(PORT, () => {
  console.log('🚀 Update server running!');
  console.log(`📍 URL: http://localhost:${PORT}`);
  console.log('');
  console.log('📝 Endpoints:');
  console.log(`   - Health: http://localhost:${PORT}/health`);
  console.log(`   - Updates: http://localhost:${PORT}/{target}/{arch}/{version}`);
  console.log(`   - Files: http://localhost:${PORT}/updates/`);
  console.log('');
  console.log('💡 Example check:');
  console.log(`   curl http://localhost:${PORT}/darwin/x86_64/0.1.0`);
  console.log('');
  console.log('⚠️  For production, use:');
  console.log('   - CDN (CloudFlare, AWS CloudFront)');
  console.log('   - Static hosting (Vercel, Netlify)');
  console.log('   - Object storage (S3, GCS, Azure Blob)');
});
