#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

console.log('🎉 b00t-cli post-installation check...');

const extension = process.platform === 'win32' ? '.exe' : '';
const binaryPath = path.join(__dirname, '..', 'bin', `b00t-cli${extension}`);

if (!fs.existsSync(binaryPath)) {
  console.error('❌ Binary not found after installation');
  process.exit(1);
}

if (process.platform !== 'win32') {
  fs.chmodSync(binaryPath, 0o755);
}

try {
  const output = execSync(`"${binaryPath}" --version`, { encoding: 'utf8', timeout: 5000 });
  console.log('✅ Binary verification successful');
  console.log(`📋 Version: ${output.trim()}`);
} catch (error) {
  console.warn('⚠️ Binary verification failed, but installation completed');
}

console.log('');
console.log('🥾 b00t-cli is ready! Usage:');
console.log('  b00t-cli --help');
console.log('  b00t whoami');
console.log('');
console.log('📚 Documentation: https://github.com/elasticdotventures/dotfiles/tree/main/b00t-cli');
