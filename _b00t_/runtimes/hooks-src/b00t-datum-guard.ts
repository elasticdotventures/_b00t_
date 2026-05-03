// b00t-datum-guard.ts — PreToolUse hook
// Redirects package manager installs to b00t cli install
// Validates depends_on before b00t install
// 🤓 NEVER exit non-zero — always advisory only (additionalContext)

import { existsSync, readFileSync } from 'fs';
import { join, dirname } from 'path';

const input = JSON.parse(process.argv[2] || '{}');
const command: string = input?.input?.command ?? '';

// Package manager redirect patterns (original behavior)
const PACKAGE_MANAGER_PATTERNS = [
  { regex: /^\s*pip\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*npm\s+install\s+-g\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*apt(-get)?\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*brew\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*cargo\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
];

// b00t install detection (new behavior)
const INSTALL_REGEX = /^\s*b00t\s+(?:cli\s+)?install\s+/i;

// Check if datum file exists in _b00t_/ directory
function datumExists(datumName: string, b00tDir: string): boolean {
  const suffixes = ['', '.cli', '.mcp', '.agent', '.model', '.stack', '.job', '.docker', '.api', '.role', '.datum'];
  const baseName = datumName.replace(/\.cli$/, '');
  
  for (const suffix of suffixes) {
    const checkName = baseName + suffix;
    for (const ext of ['.toml', '']) {
      const checkPath = join(b00tDir, checkName + ext);
      if (existsSync(checkPath)) return true;
    }
  }
  return false;
}

// Parse depends_on array from TOML content
function parseDependsOn(tomlContent: string): string[] {
  const deps: string[] = [];
  const regex = /depends_on\s*=\s*\[([^\]]*)\]/g;
  const match = regex.exec(tomlContent);
  if (match && match[1]) {
    const items = match[1].match(/"([^"]+)"/g);
    if (items) {
      for (const item of items) {
        deps.push(item.replace(/"/g, '').trim());
      }
    }
  }
  return deps;
}

// Find _b00t_/ directory relative to hook location
function findB00tDir(hookDir: string): string | null {
  let dir = hookDir;
  for (let i = 0; i < 10; i++) {
    const check = join(dir, '_b00t_');
    if (existsSync(check)) return check;
    const prev = dirname(dir);
    if (prev === dir) break;
    dir = prev;
  }
  return null;
}

const B00T_DIR = findB00tDir(__dirname);
let advisory: string | null = null;

// 1) Package manager guards (original behavior)
for (const { regex, hint } of PACKAGE_MANAGER_PATTERNS) {
  if (regex.test(command)) {
    advisory = `⚠️ b00t datum-guard: prefer \`${hint}\` over direct package managers.\nCheck available datums with \`b00t cli desires\`.\nDirect install will work but won't be tracked in the b00t hive.`;
    break;
  }
}

// 2) b00t install depends_on check (new behavior)
if (!advisory && INSTALL_REGEX.test(command)) {
  const match = command.match(/^\s*b00t\s+(?:cli\s+)?install\s+(\S+)/);
  if (match && match[1]) {
    const datumName = match[1].trim();
    
    if (!B00T_DIR) {
      advisory = '⚠️ b00t datum-guard: could not locate _b00t_/ directory';
    } else {
      const suffixes = ['', '.cli', '.mcp', '.agent', '.model', '.stack', '.job', '.docker', '.api', '.role', '.datum'];
      let tomlPath: string | null = null;
      
      for (const suffix of suffixes) {
        const path = join(B00T_DIR, datumName + suffix + '.toml');
        if (existsSync(path)) {
          tomlPath = path;
          break;
        }
      }
      
      if (!tomlPath) {
        advisory = `⚠️ b00t datum-guard: datum '${datumName}' not found in _b00t_/`;
      } else {
        const tomlContent = readFileSync(tomlPath, 'utf-8');
        const dependencies = parseDependsOn(tomlContent);
        
        if (dependencies.length === 0) {
          advisory = `ok`;
        } else {
          const missingDeps: string[] = [];
          for (const dep of dependencies) {
            if (!datumExists(dep, B00T_DIR)) {
              missingDeps.push(dep);
            }
          }
          
          if (missingDeps.length > 0) {
            advisory = `⚠️ b00t datum-guard: missing dependencies for '${datumName}' — install dependency${missingDeps.length > 1 ? 's' : ''} first: ${missingDeps.join(', ')}`;
          } else {
            advisory = `ok`;
          }
        }
      }
    }
  }
}

// Output result
if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}

process.exit(0);  // ALWAYS exit 0 — never block
