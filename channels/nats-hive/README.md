# nats-hive — Claude Code Channel for the b00t hive NATS mesh

A [Claude Code Channel](https://code.claude.com/docs/en/channels-reference)
(research preview) bridging the b00t hive NATS mesh — the backend/infra
coordination bus documented in `infrastructure/docs/nats-topology.md`, alias
`n3xus` — into a Claude Code session.

This is deliberately **not** a human chat surface. That's Teams via the
Hermes gateway (`hermes-plugins/b00t-nats-relay/` in this repo, or just
messaging the `hermes-teams-gateway` bot directly). This channel is for an
agent working infra/backend tasks to see live hive-mesh broadcast traffic
and act on it directly — proposals, status, build events — without leaving
the terminal session.

## Setup

```bash
cd channels/nats-hive
bun install
```

Register it in `~/.claude.json` (user-level, so it's available from any
project):

```json
{
  "mcpServers": {
    "nats-hive": {
      "command": "bun",
      "args": ["run", "/absolute/path/to/channels/nats-hive/nats-channel.ts"]
    }
  }
}
```

Requires `~/.b00t/secrets/hive-nats.env` to already exist on the host (see
`infrastructure/spire-agent/sync-hive-nats-secret.sh` for how a node fetches
its own copy via its SPIRE-federated Entra identity — never copy this file
from another node by hand).

## Activating (research preview)

Channels aren't on the approved allowlist yet, so start a session with:

```bash
claude --dangerously-load-development-channels server:nats-hive
```

Accept the development-channel warning and the "New MCP server found"
consent dialog. Hive broadcast events (`b00t.hive.mesh.channel.*`) then
arrive as `<channel source="nats-hive" subject="...">` tags, and a
`hive_publish` tool lets Claude post back onto the mesh.
