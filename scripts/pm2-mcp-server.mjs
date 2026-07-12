#!/usr/bin/env node
/**
 * pm2-mcp MCP Server — exposes PM2 process management as MCP tools.
 *
 * Tools:
 *   pm2_start   — Start a process
 *   pm2_stop    — Stop a process
 *   pm2_restart — Restart a process
 *   pm2_status  — List all managed processes
 *   pm2_logs    — Get recent logs for a process
 *   pm2_test    — Run internal connectivity test
 *
 * Usage:
 *   node scripts/pm2-mcp-server.mjs        # stdio MCP mode
 *   node scripts/pm2-mcp-server.mjs --test  # self-test mode
 */

import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

const PM2_CMD = process.env.PM2_CMD || 'pm2';

// ─── MCP Protocol helpers ───────────────────────────────────────────────────

function send(msg) {
  const line = JSON.stringify(msg);
  process.stdout.write(line + '\n');
}

function error(msg) {
  send({ jsonrpc: '2.0', error: { code: -1, message: msg } });
}

// ─── PM2 helpers ────────────────────────────────────────────────────────────

function pm2(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(PM2_CMD, args, {
      stdio: ['ignore', 'pipe', 'pipe'],
      shell: false,
    });
    let stdout = '', stderr = '';
    child.stdout.on('data', d => stdout += d);
    child.stderr.on('data', d => stderr += d);
    child.on('close', code => {
      if (code === 0) resolve(stdout.trim());
      else reject(new Error(stderr.trim() || stdout.trim() || `exit code ${code}`));
    });
    child.on('error', reject);
  });
}

// ─── Tool handlers ──────────────────────────────────────────────────────────

const TOOLS = {
  pm2_start: {
    description: 'Start a process under PM2 management.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Process name' },
        script: { type: 'string', description: 'Command or script path' },
        args: { type: 'string', description: 'Command arguments' },
        cwd: { type: 'string', description: 'Working directory' },
      },
      required: ['name', 'script'],
    },
    handler: async (args) => {
      const pmArgs = ['start', args.script, '--name', args.name];
      if (args.args) pmArgs.push('--', ...args.args.split(/\s+/));
      if (args.cwd) pmArgs.push('--cwd', args.cwd);
      const out = await pm2(pmArgs);
      return { status: 'started', name: args.name, output: out };
    },
  },

  pm2_stop: {
    description: 'Stop a managed process.',
    inputSchema: {
      type: 'object', properties: { name: { type: 'string' } }, required: ['name'],
    },
    handler: async (args) => {
      const out = await pm2(['stop', args.name]);
      return { status: 'stopped', name: args.name, output: out };
    },
  },

  pm2_restart: {
    description: 'Restart a managed process.',
    inputSchema: {
      type: 'object', properties: { name: { type: 'string' } }, required: ['name'],
    },
    handler: async (args) => {
      const out = await pm2(['restart', args.name]);
      return { status: 'restarted', name: args.name, output: out };
    },
  },

  pm2_status: {
    description: 'List all PM2-managed processes.',
    inputSchema: { type: 'object', properties: {} },
    handler: async () => {
      const out = await pm2(['jlist']);
      const list = JSON.parse(out || '[]');
      return list.map(p => ({
        name: p.name,
        status: p.pm2_env?.status || 'unknown',
        pid: p.pid,
        cpu: p.monit?.cpu,
        memory: p.monit?.memory,
        uptime: p.pm2_env?.pm_uptime,
        restarts: p.pm2_env?.restart_time,
      }));
    },
  },

  pm2_logs: {
    description: 'Get recent logs for a process.',
    inputSchema: {
      type: 'object', properties: {
        name: { type: 'string', description: 'Process name (or "all")' },
        lines: { type: 'number', description: 'Lines to show (default 20)' },
      }, required: ['name'],
    },
    handler: async (args) => {
      const lines = args.lines || 20;
      try {
        const out = await pm2(['logs', args.name, '--nostream', '--lines', String(lines)]);
        return { logs: out.split('\n').slice(-lines).join('\n') };
      } catch (e) {
        // Fallback: read log file directly
        const out = await pm2(['show', args.name]);
        const logPath = (out.match(/out log path:\s+(\S+)/) || [])[1];
        if (!logPath) throw new Error('Cannot find log path');
        const fs = await import('node:fs');
        const content = fs.readFileSync(logPath, 'utf-8');
        const tail = content.split('\n').slice(-lines).join('\n');
        return { logs: tail };
      }
    },
  },

  pm2_test: {
    description: 'Run pm2-mcp internal connectivity and health test.',
    inputSchema: { type: 'object', properties: {} },
    handler: async () => {
      const results = [];
      let passed = 0, failed = 0;

      // Test 1: pm2 ping
      try {
        const ping = await pm2(['ping']);
        results.push({ test: 'pm2 ping', status: '✅', detail: ping });
        passed++;
      } catch (e) {
        results.push({ test: 'pm2 ping', status: '❌', detail: e.message });
        failed++;
      }

      // Test 2: pm2 list
      try {
        const list = await pm2(['list', '--no-color']);
        results.push({ test: 'pm2 list', status: '✅', detail: list.length > 100 ? list.slice(0, 100) + '...' : list });
        passed++;
      } catch (e) {
        results.push({ test: 'pm2 list', status: '❌', detail: e.message });
        failed++;
      }

      // Test 3: list as JSON
      try {
        const jlist = await pm2(['jlist']);
        const parsed = JSON.parse(jlist);
        results.push({ test: 'pm2 jlist', status: '✅', detail: `${parsed.length} process(es) managed` });
        passed++;
      } catch (e) {
        results.push({ test: 'pm2 jlist', status: '❌', detail: e.message });
        failed++;
      }

      return {
        summary: `${passed} passed, ${failed} failed`,
        results,
        pm2_version: await pm2(['--version']).catch(() => 'unknown'),
        node_version: process.version,
      };
    },
  },
};

// ─── MCP Server Loop ────────────────────────────────────────────────────────

async function handleRequest(msg) {
  if (msg.method === 'ping') {
    return send({ jsonrpc: '2.0', id: msg.id, result: 'pong' });
  }

  if (msg.method === 'tools/list') {
    return send({
      jsonrpc: '2.0', id: msg.id,
      result: Object.entries(TOOLS).map(([name, t]) => ({
        name, description: t.description, inputSchema: t.inputSchema,
      })),
    });
  }

  if (msg.method === 'tools/call') {
    const tool = TOOLS[msg.params.name];
    if (!tool) {
      return send({ jsonrpc: '2.0', id: msg.id, error: { code: -32601, message: `Unknown tool: ${msg.params.name}` } });
    }
    try {
      const result = await tool.handler(msg.params.arguments || {});
      return send({ jsonrpc: '2.0', id: msg.id, result });
    } catch (e) {
      return send({ jsonrpc: '2.0', id: msg.id, error: { code: -1, message: e.message } });
    }
  }

  return send({ jsonrpc: '2.0', id: msg.id, error: { code: -32601, message: `Method not found: ${msg.method}` } });
}

// ─── Main ───────────────────────────────────────────────────────────────────

async function main() {
  // Self-test mode
  if (process.argv.includes('--test')) {
    const tool = TOOLS.pm2_test;
    const result = await tool.handler({});
    console.log(JSON.stringify(result, null, 2));
    process.exit(result.summary.startsWith('3') ? 0 : 1);
  }

  // MCP stdio mode
  const rl = createInterface({ input: process.stdin });
  rl.on('line', async (line) => {
    try {
      const msg = JSON.parse(line);
      await handleRequest(msg);
    } catch (e) {
      // Ignore malformed JSON
    }
  });

  // Send initialized notification
  send({ jsonrpc: '2.0', method: 'initialized' });
}

main().catch(e => {
  error(e.message);
  process.exit(1);
});
