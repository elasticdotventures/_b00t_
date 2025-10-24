# b00t chat protocol

PromptExecution agents now exchange lightweight JSON chat payloads over two
transports:

- 🧵 **Local IPC** – Unix domain socket at `~/.b00t/chat.channel.socket`
- 📡 **NATS stub** – retains subject namespacing for future federation

## Message envelope

```json
{
  "channel": "mission.delta",
  "sender": "frontend.agent",
  "body": "handoff complete",
  "metadata": {"ticket": "OPS-123"},
  "timestamp": "2025-03-04T05:30:00Z"
}
```

- `channel` keeps conversations scoped per mission/crew.
- `sender` is free-form but SHOULD stay unique inside a channel.
- `body` is plain text; additional structure belongs in `metadata`.
- `timestamp` MUST be RFC 3339; producer SHOULD use UTC.

## Local socket lifecycle

1. **b00t-mcp** boots and calls `b00t_chat::spawn_local_server`.
2. The server binds the socket, accepts JSON lines, and queues them inside
   `ChatInbox`.
3. Before any command response is emitted, the MCP server drains the inbox and
   appends `<🥾>{ "chat": { "msgs": N }}</🥾>` to the outgoing payload.
4. Drained messages are logged (channel, sender, body) so operators can stitch
   context.

Consumers MUST write newline-delimited JSON to the socket. The helper client in
`b00t-cli chat send` already handles serialization, flushing, and fallbacks.

## NATS subjects (stub)

Until credentials are provisioned, the NATS transport simply logs intent. The
subject prefix matches historical ACP conventions: `chat.{channel}`. Swapping in
`async-nats` will require exporting JWT-authenticated configuration from the MCP
environment.

## CLI usage

```bash
# Local sockets (default)
b00t-cli chat send --channel mission.delta --message "artifact staged"

# Explicit transport selection
b00t-cli chat send --transport nats --message "deploying" \
  --metadata '{"env":"prod"}'

# Discover socket path
b00t-cli chat info
```

## Integration checklist

- [x] Rename workspace crate to `b00t-chat`
- [x] Replace `acp` CLI command tree with `chat`
- [x] Start chat listener inside `b00t-mcp` and surface unread counts
- [x] Update Docker scaffolding and docs to reflect the new library

Future work SHOULD cover:

1. Wiring a broadcast-friendly listener so dedicated viewers can tail messages.
2. Completing the NATS transport with authentication + reconnect handling.
3. Extending metadata conventions (e.g. thread IDs, attachments) once the socket
   protocol stabilizes.
