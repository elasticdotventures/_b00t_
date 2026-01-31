#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

console.log('🔨 Building b00t-mcp for distribution...');

// Build targets for different platforms
const targets = [
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu', 
  'x86_64-apple-darwin',
  'aarch64-apple-darwin',
  'x86_64-pc-windows-msvc',
  'aarch64-pc-windows-msvc'
];

const projectRoot = path.join(__dirname, '..', '..');
const outputDir = path.join(__dirname, '..', 'dist');

// Ensure output directory exists
if (!fs.existsSync(outputDir)) {
  fs.mkdirSync(outputDir, { recursive: true });
}

function buildTarget(target) {
  console.log(`🎯 Building for target: ${target}`);
  
  try {
    // Add target if not already installed
    execSync(`rustup target add ${target}`, { 
      cwd: projectRoot,
      stdio: 'pipe' 
    });
    
    // Build for target
    execSync(`cargo build --release --target ${target} --package b00t-mcp`, {
      cwd: projectRoot,
      stdio: 'inherit'
    });
    
    // Copy binary to output directory
    const extension = target.includes('windows') ? '.exe' : '';
    const binaryName = `b00t-mcp${extension}`;
    const sourcePath = path.join(projectRoot, 'target', target, 'release', binaryName);
    const destPath = path.join(outputDir, `b00t-mcp-${target}${extension}`);
    
    if (fs.existsSync(sourcePath)) {
      fs.copyFileSync(sourcePath, destPath);
      console.log(`✅ Built ${target}`);
    } else {
      throw new Error(`Binary not found at ${sourcePath}`);
    }
    
  } catch (error) {
    console.error(`❌ Failed to build ${target}: ${error.message}`);
    return false;
  }
  
  return true;
}

// Build for all targets
console.log('🚀 Starting cross-platform build...');

const results = targets.map(target => ({
  target,
  success: buildTarget(target)
}));

// Summary
console.log('\n📊 Build Summary:');
results.forEach(({ target, success }) => {
  const status = success ? '✅' : '❌';
  console.log(`  ${status} ${target}`);
});

const successCount = results.filter(r => r.success).length;
console.log(`\n🎉 Built ${successCount}/${targets.length} targets successfully`);

if (successCount === 0) {
  console.error('❌ No builds succeeded');
  process.exit(1);
}