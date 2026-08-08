#!/usr/bin/env bash
# obsidian-mcp.sh — proxy stdio MCP ↔ Obsidian vault via HTTPS
set -euo pipefail
: "${OBSIDIAN_MCP_KEY:?required}"
HOST="${OBSIDIAN_HOST:-192.168.1.150}"
PORT="${OBSIDIAN_PORT:-3443}"

exec node -e "
const https = require('https');
const key = process.env.OBSIDIAN_MCP_KEY;
const host = process.env.OBSIDIAN_HOST || '192.168.1.150';
const port = +(process.env.OBSIDIAN_PORT || '3443');
let sid;

let buf = '';
process.stdin.on('data', c => { buf += c; processBuffer(); });
process.stdin.on('end', () => process.exit(0));

function processBuffer() {
  for (;;) {
    const nl = buf.indexOf('\n');
    if (nl === -1) return;
    const line = buf.slice(0, nl).trim();
    buf = buf.slice(nl + 1);
    if (!line) continue;
    send(line);
  }
}

async function send(line) {
  const headers = {'Content-Type':'application/json','Accept':'application/json, text/event-stream','Authorization':'Bearer '+key};
  if (sid) headers['Mcp-Session-Id'] = sid;
  try {
    const data = await new Promise((resolve, reject) => {
      const req = https.request({hostname:host,port,path:'/mcp',method:'POST',headers,rejectUnauthorized:false}, res => {
        let d = '';
        res.on('data', c => d += c);
        res.on('end', () => {
          if (!sid && res.headers['mcp-session-id']) sid = res.headers['mcp-session-id'];
          resolve(d);
        });
      });
      req.on('error', reject);
      req.write(line);
      req.end();
    });
    process.stdout.write(data + '\n');
  } catch(e) {
    process.stderr.write(e.message + '\n');
  }
}
"