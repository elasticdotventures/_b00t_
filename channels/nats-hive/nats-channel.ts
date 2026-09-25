#!/usr/bin/env bun
// Claude Code Channel bridging the b00t hive NATS mesh (backend/infra bus,
// see infrastructure/docs/nats-topology.md) into a Claude Code session.
//
// Companion to the Hermes b00t-nats-relay plugin (parked, autonomous-send
// build pending explicit sign-off) — same NATS bus and subject convention,
// different consumer: this surfaces raw hive events as session context for
// direct agent action, the Hermes plugin (when built) summarizes/filters
// for a human via Teams. Neither depends on the other; both can run.
//
// See https://code.claude.com/docs/en/channels-reference for the channel
// contract this implements.
import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import { ListToolsRequestSchema, CallToolRequestSchema } from '@modelcontextprotocol/sdk/types.js'
import { connect, StringCodec, type NatsConnection } from 'nats'
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

const sc = StringCodec()

function loadHiveNatsEnv(): Record<string, string> {
  const path = join(homedir(), '.b00t', 'secrets', 'hive-nats.env')
  const out: Record<string, string> = {}
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    const m = line.match(/^([A-Z_]+)=(.*)$/)
    if (m) out[m[1]] = m[2]
  }
  return out
}

const CHANNEL_PREFIX = 'b00t.hive.mesh.channel.'

const mcp = new Server(
  { name: 'nats-hive', version: '0.1.0' },
  {
    capabilities: {
      experimental: { 'claude/channel': {} },
      tools: {},
    },
    instructions:
      `Events from the b00t hive NATS mesh (backend/infra bus, NOT the human ` +
      `Teams chat — that's a separate surface via Hermes) arrive as ` +
      `<channel source="nats-hive" subject="${CHANNEL_PREFIX}...">. This is a ` +
      `broadcast bus: subjects under "${CHANNEL_PREFIX}" are named channels ` +
      `any hive agent can publish/subscribe to. To act on the mesh (send a ` +
      `status update, make a proposal, respond to another agent), call the ` +
      `hive_publish tool with the same subject and a text body — don't treat ` +
      `this as a 1:1 chat reply, it's a broadcast onto the named channel.`,
  },
)

let nc: NatsConnection | undefined

mcp.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: 'hive_publish',
      description: 'Publish a message onto a b00t hive NATS mesh channel (broadcast, not 1:1)',
      inputSchema: {
        type: 'object',
        properties: {
          subject: {
            type: 'string',
            description: `Full NATS subject, e.g. "${CHANNEL_PREFIX}claude-code"`,
          },
          text: { type: 'string', description: 'Message body to publish' },
        },
        required: ['subject', 'text'],
      },
    },
  ],
}))

mcp.setRequestHandler(CallToolRequestSchema, async (req) => {
  if (req.params.name !== 'hive_publish') {
    throw new Error(`unknown tool: ${req.params.name}`)
  }
  if (!nc) {
    return { content: [{ type: 'text', text: 'error: not connected to NATS' }], isError: true }
  }
  const { subject, text } = req.params.arguments as { subject: string; text: string }
  nc.publish(subject, sc.encode(text))
  return { content: [{ type: 'text', text: `published to ${subject}` }] }
})

await mcp.connect(new StdioServerTransport())

const creds = loadHiveNatsEnv()
nc = await connect({
  servers: '127.0.0.1:4222',
  user: creds.HIVE_NATS_USER,
  pass: creds.HIVE_NATS_PASSWORD,
  reconnect: true,
  maxReconnectAttempts: -1,
})

const sub = nc.subscribe(`${CHANNEL_PREFIX}>`)
for await (const msg of sub) {
  await mcp.notification({
    method: 'notifications/claude/channel',
    params: {
      content: sc.decode(msg.data),
      meta: { subject: msg.subject },
    },
  })
}
