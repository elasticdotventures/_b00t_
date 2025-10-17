#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

console.log('🎉 b00t-mcp post-installation setup...');

const binDir = path.join(__dirname, '..', 'bin');
const platform = process.platform;
const extension = platform === 'win32' ? '.exe' : '';
const binaryName = `b00t-mcp${extension}`;
const binaryPath = path.join(binDir, binaryName);

// Verify binary exists and is executable
if (!fs.existsSync(binaryPath)) {
  console.error('❌ Binary not found after installation');
  process.exit(1);
}

// Ensure binary is executable
if (platform !== 'win32') {
  fs.chmodSync(binaryPath, 0o755);
}

// Test binary works
try {
  const { execSync } = require('child_process');
  const output = execSync(`"${binaryPath}" --version`, { 
    encoding: 'utf8',
    timeout: 5000 
  });
  console.log('✅ Binary verification successful');
  console.log(`📋 Version: ${output.trim()}`);
} catch (error) {
  console.warn('⚠️ Binary verification failed, but installation completed');
}

console.log('');
console.log('🥾 b00t-mcp is ready! Usage:');
console.log('  npx b00t-mcp --help');
console.log('  bunx b00t-mcp --help');
console.log('');
console.log('📚 Documentation: https://github.com/elasticdotventures/dotfiles/tree/main/b00t-mcp');