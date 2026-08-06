#!/usr/bin/env node

// b00t-cli npm wrapper: cargo-install-only strategy.
//
// There is currently no CI/release pipeline producing prebuilt
// b00t-{target}.tar.gz binary assets on GitHub Releases (verified before
// writing this: `gh release view v0.8.4` has zero assets) — unlike
// b00t-mcp-npm's install.js, this package does NOT attempt a
// GitHub-releases download strategy, since it can never succeed today and
// would just be dead code presenting a false option. Requires the end
// user to have a Rust toolchain (cargo) installed.

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

console.log('🥾 b00t-cli installation starting (cargo-install)...');

function installFromCargo() {
  try {
    execSync('cargo --version', { stdio: 'ignore' });
  } catch {
    throw new Error(
      'cargo not found. b00t-cli currently installs via `cargo install` only ' +
        '(no prebuilt binaries yet) — install Rust from https://rustup.rs and retry.'
    );
  }

  // 🤓 Safety: bound resource usage — this is a big workspace to build.
  const cargoEnv = {
    ...process.env,
    CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '2',
    CARGO_NET_RETRY: '3',
    CARGO_HTTP_TIMEOUT: '60',
  };

  const binDir = path.join(__dirname, '..', 'bin');
  fs.mkdirSync(binDir, { recursive: true });

  console.log('🦀 cargo install --git https://github.com/elasticdotventures/dotfiles --bin b00t-cli');
  console.log('⏳ This builds a large Rust workspace and can take 10-30+ minutes on first install.');

  execSync(
    'cargo install --git https://github.com/elasticdotventures/dotfiles --bin b00t-cli --root .',
    {
      cwd: path.join(__dirname, '..'),
      stdio: 'inherit',
      env: cargoEnv,
      timeout: 45 * 60 * 1000, // 45 minute ceiling
      maxBuffer: 50 * 1024 * 1024,
    }
  );

  const extension = process.platform === 'win32' ? '.exe' : '';
  const installedPath = path.join(binDir, `b00t-cli${extension}`);
  if (!fs.existsSync(installedPath)) {
    throw new Error('cargo install completed but binary not found at expected path');
  }
}

try {
  installFromCargo();
  console.log('✅ b00t-cli installed successfully');
} catch (error) {
  console.error(`❌ Installation failed: ${error.message}`);
  console.error(
    '🔧 Try manually: cargo install --git https://github.com/elasticdotventures/dotfiles --bin b00t-cli'
  );
  process.exit(1);
}
