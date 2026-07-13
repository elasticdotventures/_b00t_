#!/usr/bin/env node
const https = require('https');
const key = process.env.OBSIDIAN_MCP_KEY;
if (!key) { process.stderr.write('OBSIDIAN_MCP_KEY required\n'); process.exit(1); }
const host = process.env.OBSIDIAN_HOST || '192.168.1.150';
const port = +(process.env.OBSIDIAN_PORT || '3443');
let sid = null;

async function send(line) {
  const headers = { 'Content-Type': 'application/json', 'Accept': 'application/json, text/event-stream', 'Authorization': 'Bearer ' + key };
  if (sid) headers['Mcp-Session-Id'] = sid;
  return new Promise((resolve) => {
    const req = https.request({ hostname: host, port, path: '/mcp', method: 'POST', headers, rejectUnauthorized: false }, res => {
      let data = '';
      res.on('data', c => data += c);
      res.on('end', () => {
        if (!sid && res.headers['mcp-session-id']) sid = res.headers['mcp-session-id'];
        process.stdout.write(data + '\n');
        resolve();
      });
    });
    req.on('error', e => { process.stderr.write(e.message + '\n'); resolve(); });
    req.write(line);
    req.end();
  });
}

(async () => {
  let buf = '';
  for await (const chunk of process.stdin) {
    buf += chunk;
    for (;;) {
      const nl = buf.indexOf('\n');
      if (nl === -1) break;
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (line) await send(line);
    }
  }
})();
