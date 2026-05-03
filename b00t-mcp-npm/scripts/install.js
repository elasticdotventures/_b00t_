#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

// Platform detection
const platform = process.platform;
const arch = process.arch;

// 🤓 b00t convention: descriptive logging with emojis
console.log('🥾 b00t-mcp installation starting...');
console.log(`📍 Platform: ${platform}-${arch}`);

// Map Node.js platform/arch to Rust target triples
const targetMap = {
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc'
};

const target = targetMap[`${platform}-${arch}`];
if (!target) {
  console.error(`❌ Unsupported platform: ${platform}-${arch}`);
  process.exit(1);
}

// 🤓 Try multiple installation strategies in order of preference
async function installBinary() {
  // Safety check: prevent running from dangerous locations
  const cwd = process.cwd();
  if (cwd.includes('dotfiles') && cwd.includes('.git')) {
    throw new Error('⚠️ Safety: Cannot run cargo install from workspace root to prevent fork bomb');
  }

  const strategies = [
    () => installFromGitHubReleases(target),
    () => installFromCargo(),
    () => compileFromSource()
  ];

  for (const strategy of strategies) {
    try {
      await strategy();
      console.log('✅ b00t-mcp installed successfully');
      return;
    } catch (error) {
      console.warn(`⚠️ Installation strategy failed: ${error.message}`);
    }
  }

  throw new Error('❌ All installation strategies failed');
}

// Strategy 1: Download pre-compiled binary from GitHub releases
async function installFromGitHubReleases(target) {
  console.log('🔍 Attempting to download pre-compiled binary...');
  
  const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));
  const version = packageJson.version;
  
  const extension = platform === 'win32' ? '.exe' : '';
  const binaryName = `b00t-mcp${extension}`;
  const archiveName = `b00t-mcp-${target}.tar.gz`;
  
  // 🤓 GitHub releases pattern following b00t conventions
  const releaseUrl = `https://github.com/elasticdotventures/dotfiles/releases/download/v${version}/${archiveName}`;

  console.log(`📥 Downloading from: ${releaseUrl}`);

  // For development/CI, check if we have pre-packaged binaries
  const releasesDir = path.join(__dirname, '..', 'releases');
  if (fs.existsSync(releasesDir)) {
    const localArchive = path.join(releasesDir, archiveName);
    if (fs.existsSync(localArchive)) {
      console.log('📦 Using local pre-built binary');
      return extractLocalArchive(localArchive, binaryName);
    }
  }
  
  return new Promise((resolve, reject) => {
    const request = https.get(releaseUrl, (response) => {
      if (response.statusCode === 404) {
        reject(new Error('Release not found, trying next strategy'));
        return;
      }
      
      if (response.statusCode !== 200) {
        reject(new Error(`Download failed with status ${response.statusCode}`));
        return;
      }
      
      // Extract and place binary
      const tar = require('tar');
      const extractStream = tar.extract({
        cwd: path.join(__dirname, '..', 'bin'),
        strip: 1
      });
      
      response.pipe(extractStream);
      
      extractStream.on('end', () => {
        const binaryPath = path.join(__dirname, '..', 'bin', binaryName);
        fs.chmodSync(binaryPath, 0o755);
        resolve();
      });
      
      extractStream.on('error', reject);
    });
    
    request.on('error', reject);
  });
}

// Helper function to extract local archive
function extractLocalArchive(archivePath, binaryName) {
  return new Promise((resolve, reject) => {
    const tar = require('tar');
    const extractStream = tar.extract({
      cwd: path.join(__dirname, '..', 'bin'),
      strip: 1
    });

    const readStream = fs.createReadStream(archivePath);
    readStream.pipe(extractStream);

    extractStream.on('end', () => {
      const binaryPath = path.join(__dirname, '..', 'bin', binaryName);
      if (fs.existsSync(binaryPath)) {
        fs.chmodSync(binaryPath, 0o755);
        resolve();
      } else {
        reject(new Error('Binary not found after extraction'));
      }
    });

    extractStream.on('error', reject);
    readStream.on('error', reject);
  });
}

// Strategy 2: Install via cargo with safety limits
async function installFromCargo() {
  console.log('🦀 Attempting cargo installation...');

  try {
    execSync('cargo --version', { stdio: 'ignore' });
  } catch {
    throw new Error('Cargo not available');
  }

  // 🤓 Safety: Set resource limits to prevent fork bomb
  const cargoEnv = {
    ...process.env,
    CARGO_BUILD_JOBS: '1',           // Limit parallel jobs
    CARGO_NET_RETRY: '2',            // Limit network retries
    CARGO_HTTP_TIMEOUT: '30',        // 30 second timeout
    RUST_BACKTRACE: '0'              // Disable backtrace to save memory
  };

  console.log('⚠️ Using limited cargo build (CARGO_BUILD_JOBS=1 to prevent resource exhaustion)');

  // Create bin directory if it doesn't exist
  const binDir = path.join(__dirname, '..', 'bin');
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  // Install with timeout and resource limits
  try {
    execSync('cargo install --git https://github.com/elasticdotventures/dotfiles --bin b00t-mcp --root .', {
      cwd: path.join(__dirname, '..'),
      stdio: 'inherit',
      env: cargoEnv,
      timeout: 300000, // 5 minute timeout
      maxBuffer: 50 * 1024 * 1024 // 50MB buffer limit
    });
  } catch (error) {
    if (error.code === 'ETIMEDOUT') {
      throw new Error('Cargo build timed out (5min limit) - try pre-built binary instead');
    }
    throw error;
  }

  // Move binary from cargo install location to bin/
  const extension = platform === 'win32' ? '.exe' : '';
  const srcPath = path.join(__dirname, '..', 'bin', `b00t-mcp${extension}`);
  const cargoPath = path.join(__dirname, '..', 'bin', `b00t-mcp${extension}`);

  if (!fs.existsSync(cargoPath)) {
    throw new Error('Cargo install succeeded but binary not found in expected location');
  }
}

// Strategy 3: Compile from source (requires git clone)
async function compileFromSource() {
  console.log('🔨 Attempting source compilation...');
  throw new Error('Source compilation not implemented yet');
}

// Run installation
installBinary().catch((error) => {
  console.error(`❌ Installation failed: ${error.message}`);
  console.error('🔧 Try installing manually with: cargo install --git https://github.com/elasticdotventures/dotfiles b00t-mcp');
  process.exit(1);
});