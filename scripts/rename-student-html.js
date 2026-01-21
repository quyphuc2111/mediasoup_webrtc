// Script to rename index-student.html to index.html after build
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const distPath = path.join(__dirname, '..', 'dist-student');
const oldPath = path.join(distPath, 'index-student.html');
const newPath = path.join(distPath, 'index.html');

try {
  if (fs.existsSync(oldPath)) {
    fs.renameSync(oldPath, newPath);
    console.log('✅ Renamed index-student.html to index.html');
  } else {
    console.warn('⚠️ index-student.html not found, skipping rename');
  }
} catch (error) {
  console.error('❌ Error renaming file:', error);
  process.exit(1);
}
