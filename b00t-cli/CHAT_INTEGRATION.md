# 🥾 b00t chat integration

## ✅ Local coordination channel wired

- `b00t-cli chat send --message "hello"` posts to the Unix domain socket at `~/.b00t/chat.channel.socket`.
- `--transport nats` keeps parity with the legacy workflow (still a stubbed publisher).
- Metadata accepts arbitrary JSON via `--metadata '{"ticket":"ABC-123"}'`.

## 🧠 Design notes

- Implementation depends on the new `b00t-chat` crate (`b00t-cli/Cargo.toml`).
- Command plumbing lives in `src/commands/chat.rs`.
- `ChatTransportKind` normalises transports while `ChatClient` handles serialization and dispatch.
- Default channel mirrors namespace convention `account.{username}` so multiple agents stay isolated by default.

## 🔧 Example session

```bash
# Local message
b00t-cli chat send --message "handoff complete" --channel mission.delta

# Stubbed NATS telemetry
b00t-cli chat send --message "deploying" --transport nats --metadata '{"env":"prod"}'

# Inspect configured socket
b00t-cli chat info
```

## 🚦 Follow-ups

1. Replace NATS stub with async-nats publisher once credentials are provisioned.
2. Extend `chat listen` once a broadcast transport is defined.
3. Convert docs and tutorials that still reference ACP to the new chat terminology.
