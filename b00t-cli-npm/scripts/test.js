#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

console.log('🧪 Testing b00t-cli npm package...');

const tests = [testPackageJson, testShimExists, testInstallationScript];

async function runTests() {
  let passed = 0;
  let failed = 0;

  for (const test of tests) {
    try {
      await test();
      console.log(`✅ ${test.name}`);
      passed++;
    } catch (error) {
      console.error(`❌ ${test.name}: ${error.message}`);
      failed++;
    }
  }

  console.log(`\n📊 Test Results: ${passed} passed, ${failed} failed`);
  if (failed > 0) {
    process.exit(1);
  }
}

function testPackageJson() {
  const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));
  if (!pkg.name || pkg.name !== 'b00t-cli') {
    throw new Error('Invalid package name');
  }
  if (!pkg.version || !pkg.version.match(/^\d+\.\d+\.\d+/)) {
    throw new Error('Invalid version format');
  }
  if (!pkg.bin || !pkg.bin['b00t-cli']) {
    throw new Error('Missing binary configuration');
  }
}

function testShimExists() {
  const shimPath = path.join(__dirname, '..', 'bin', 'b00t-cli');
  if (!fs.existsSync(shimPath)) {
    throw new Error('bin/b00t-cli shim not found');
  }
}

function testInstallationScript() {
  const installPath = path.join(__dirname, 'install.js');
  if (!fs.existsSync(installPath)) {
    throw new Error('Installation script not found');
  }
  const content = fs.readFileSync(installPath, 'utf8');
  if (!content.includes('installFromCargo')) {
    throw new Error('Installation script missing key functions');
  }
}

runTests().catch((error) => {
  console.error(`❌ Test suite failed: ${error.message}`);
  process.exit(1);
});
