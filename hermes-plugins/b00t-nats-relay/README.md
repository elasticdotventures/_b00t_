# b00t-nats-relay — Hermes plugin, read-only hive mesh lookup

A native [Hermes Agent](https://hermes-agent.nousresearch.com/docs/developer-guide/plugins)
plugin giving a Hermes session (e.g. the Teams-bridged one on fung1) an
on-demand tool, `nats_relay_recent`, to answer "what's happening on the
hive?" — recent messages seen on the b00t hive NATS mesh's broadcast
channels (`b00t.hive.mesh.channel.*`, backend/infra bus, see
`infrastructure/docs/nats-topology.md`).

**Deliberately read-only.** A background thread (started in `register()`,
since Hermes's plugin model has no built-in persistent worker/pub-sub API)
keeps a 200-message in-memory ring buffer filled, but the plugin never sends
anything anywhere on its own — it only answers when the model calls the
tool during a turn a human started. An earlier design that autonomously
summarized and pushed hive events to Teams unprompted was deliberately not
built: it's a materially different risk profile (unattended external
messaging) and needs its own explicit sign-off before building it, not a
one-line design tweak.

Companion to `channels/nats-hive/` in this repo (a Claude Code Channel on
the same NATS bus, for agents working infra tasks directly) — same subject
convention, independent consumers, neither depends on the other.

## Deploy

```bash
scp plugin.yaml __init__.py tools.py <host>:~/.hermes/plugins/b00t-nats-relay/
ssh <host> 'hermes plugins validate ~/.hermes/plugins/b00t-nats-relay'
ssh <host> 'hermes plugins enable b00t-nats-relay'
ssh <host> 'systemctl --user restart hermes-gateway.service'  # enable needs a restart, not hot-reloaded
```

Requires `~/.b00t/secrets/hive-nats.env` to already exist on that host (see
`infrastructure/spire-agent/sync-hive-nats-secret.sh`).

Verify the listener actually connected: `ss -tnp | grep :4222` on the host
should show an established connection from the `hermes` process.
