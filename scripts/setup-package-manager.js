#!/usr/bin/env node
/**
 * Package Manager Setup Script
 * Detects and configures package manager preferences
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

const SUPPORTED_MANAGERS = ['pnpm', 'bun', 'yarn', 'npm'];

const LOCK_FILES = {
  'pnpm-lock.yaml': 'pnpm',
  'bun.lockb': 'bun',
  'yarn.lock': 'yarn',
  'package-lock.json': 'npm'
};

function getProjectRoot() {
  return process.cwd();
}

function getGlobalConfigPath() {
  return path.join(os.homedir(), '.claude', 'package-manager.json');
}

function getProjectConfigPath() {
  return path.join(getProjectRoot(), '.claude', 'package-manager.json');
}

function readJsonFile(filePath) {
  try {
    if (fs.existsSync(filePath)) {
      return JSON.parse(fs.readFileSync(filePath, 'utf8'));
    }
  } catch (err) {
    console.error(`Error reading ${filePath}:`, err.message);
  }
  return null;
}

function writeJsonFile(filePath, data) {
  try {
    const dir = path.dirname(filePath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n');
    return true;
  } catch (err) {
    console.error(`Error writing ${filePath}:`, err.message);
    return false;
  }
}

function checkCommand(cmd) {
  try {
    require('child_process').execSync(`${cmd} --version`, {
      stdio: 'ignore',
      timeout: 2000
    });
    return true;
  } catch {
    return false;
  }
}

function detectFromEnv() {
  const pm = process.env.CLAUDE_PACKAGE_MANAGER;
  if (pm && SUPPORTED_MANAGERS.includes(pm)) {
    return { source: 'environment', packageManager: pm };
  }
  return null;
}

function detectFromProjectConfig() {
  const config = readJsonFile(getProjectConfigPath());
  if (config?.packageManager && SUPPORTED_MANAGERS.includes(config.packageManager)) {
    return { source: 'project-config', packageManager: config.packageManager };
  }
  return null;
}

function detectFromPackageJson() {
  const pkgPath = path.join(getProjectRoot(), 'package.json');
  const pkg = readJsonFile(pkgPath);

  if (pkg?.packageManager) {
    // Format: "pnpm@8.6.0" or just "pnpm"
    const pm = pkg.packageManager.split('@')[0];
    if (SUPPORTED_MANAGERS.includes(pm)) {
      return { source: 'package.json', packageManager: pm };
    }
  }
  return null;
}

function detectFromLockFile() {
  const projectRoot = getProjectRoot();

  for (const [lockFile, pm] of Object.entries(LOCK_FILES)) {
    if (fs.existsSync(path.join(projectRoot, lockFile))) {
      return { source: 'lock-file', packageManager: pm, lockFile };
    }
  }
  return null;
}

function detectFromGlobalConfig() {
  const config = readJsonFile(getGlobalConfigPath());
  if (config?.packageManager && SUPPORTED_MANAGERS.includes(config.packageManager)) {
    return { source: 'global-config', packageManager: config.packageManager };
  }
  return null;
}

function detectFromAvailable() {
  // Priority order: pnpm > bun > yarn > npm
  for (const pm of SUPPORTED_MANAGERS) {
    if (checkCommand(pm)) {
      return { source: 'first-available', packageManager: pm };
    }
  }
  return null;
}

function detectPackageManager() {
  // Detection priority order
  const detectors = [
    detectFromEnv,
    detectFromProjectConfig,
    detectFromPackageJson,
    detectFromLockFile,
    detectFromGlobalConfig,
    detectFromAvailable
  ];

  for (const detector of detectors) {
    const result = detector();
    if (result) {
      return result;
    }
  }

  return { source: 'fallback', packageManager: 'npm' };
}

function setGlobalPreference(pm) {
  if (!SUPPORTED_MANAGERS.includes(pm)) {
    console.error(`Unsupported package manager: ${pm}`);
    console.log(`Supported: ${SUPPORTED_MANAGERS.join(', ')}`);
    return false;
  }

  if (!checkCommand(pm)) {
    console.warn(`⚠️  Warning: ${pm} is not installed or not in PATH`);
  }

  const config = { packageManager: pm };
  const success = writeJsonFile(getGlobalConfigPath(), config);

  if (success) {
    console.log(`✅ Global package manager set to: ${pm}`);
    console.log(`   Config: ${getGlobalConfigPath()}`);
  }
  return success;
}

function setProjectPreference(pm) {
  if (!SUPPORTED_MANAGERS.includes(pm)) {
    console.error(`Unsupported package manager: ${pm}`);
    console.log(`Supported: ${SUPPORTED_MANAGERS.join(', ')}`);
    return false;
  }

  if (!checkCommand(pm)) {
    console.warn(`⚠️  Warning: ${pm} is not installed or not in PATH`);
  }

  const config = { packageManager: pm };
  const success = writeJsonFile(getProjectConfigPath(), config);

  if (success) {
    console.log(`✅ Project package manager set to: ${pm}`);
    console.log(`   Config: ${getProjectConfigPath()}`);
  }
  return success;
}

function listPackageManagers() {
  console.log('Available Package Managers:\n');

  for (const pm of SUPPORTED_MANAGERS) {
    const installed = checkCommand(pm);
    const status = installed ? '✅ installed' : '❌ not found';
    console.log(`  ${pm.padEnd(6)} ${status}`);
  }
}

function displayDetection() {
  const result = detectPackageManager();

  console.log('Package Manager Detection Results:\n');
  console.log(`  Selected: ${result.packageManager}`);
  console.log(`  Source:   ${result.source}`);

  if (result.lockFile) {
    console.log(`  Lock:     ${result.lockFile}`);
  }

  console.log('\nDetection Priority:');
  console.log('  1. Environment variable (CLAUDE_PACKAGE_MANAGER)');
  console.log('  2. Project config (.claude/package-manager.json)');
  console.log('  3. package.json (packageManager field)');
  console.log('  4. Lock file presence');
  console.log('  5. Global config (~/.claude/package-manager.json)');
  console.log('  6. First available (pnpm > bun > yarn > npm)');

  // Show current state
  const env = process.env.CLAUDE_PACKAGE_MANAGER;
  const projectConfig = readJsonFile(getProjectConfigPath());
  const globalConfig = readJsonFile(getGlobalConfigPath());
  const pkg = readJsonFile(path.join(getProjectRoot(), 'package.json'));

  console.log('\nCurrent Configuration:');
  console.log(`  ENV:            ${env || '(not set)'}`);
  console.log(`  Project config: ${projectConfig?.packageManager || '(not set)'}`);
  console.log(`  package.json:   ${pkg?.packageManager || '(not set)'}`);
  console.log(`  Global config:  ${globalConfig?.packageManager || '(not set)'}`);
}

// CLI
const args = process.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  console.log(`
Package Manager Setup

Usage:
  node setup-package-manager.js [options]

Options:
  --detect              Show current detection results
  --list                List available package managers
  --global <pm>         Set global preference
  --project <pm>        Set project preference
  --help, -h            Show this help

Supported Package Managers:
  ${SUPPORTED_MANAGERS.join(', ')}

Examples:
  node setup-package-manager.js --detect
  node setup-package-manager.js --global pnpm
  node setup-package-manager.js --project bun
  node setup-package-manager.js --list
`);
  process.exit(0);
}

if (args.includes('--detect')) {
  displayDetection();
  process.exit(0);
}

if (args.includes('--list')) {
  listPackageManagers();
  process.exit(0);
}

const globalIdx = args.indexOf('--global');
if (globalIdx >= 0 && args[globalIdx + 1]) {
  const success = setGlobalPreference(args[globalIdx + 1]);
  process.exit(success ? 0 : 1);
}

const projectIdx = args.indexOf('--project');
if (projectIdx >= 0 && args[projectIdx + 1]) {
  const success = setProjectPreference(args[projectIdx + 1]);
  process.exit(success ? 0 : 1);
}

// Default: show detection
displayDetection();
