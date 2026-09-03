# moltis ⇄ b00t — capability overlap & standing-task integration

`vendor/moltis-b00t` (submodule; `.gitmodules` → `elasticdotventures/moltis-b00t`, a
fork of `moltis-org/moltis`; some checkouts locally override `.git/config` to the
synced `app4dog/moltis-b00t` fork)
had drifted ~40 commits / 7 weeks past the b00t pin `5d171bb1` (2026-07-15).
This bumps it to `4829e6a7` (2026-09-02) and proposes how moltis earns a standing
seat in the hive.

## What moltis added in the last ~6 months (since the pin)

| Area | Notable commits |
|---|---|
| **ACP** | `feat(acp): expose Moltis as an ACP agent over stdio` (#1169, new `crates/acp/`), auto-detect ACP agents (#1149), model/effort selection for external agents (#1125), MiniMax Code ACP agent (#1204) |
| **Sandbox** | remote & multi-backend sandbox — Vercel/Daytona/Firecracker (#942), Podman escape hatches (#1106), per-agent runtime limits (#1066), per-turn tool controls (#1069), "agents as capability boundaries: MCP/sandbox/skills" (#1049) |
| **Cron** | `crates/cron/` — durable (sqlite/file/memory stores), heartbeat, schedule parser, `system_events`; channel-context-aware delivery (#1226, #1243) |
| **Memory** | `B00tSoulWriter` adapter → b00t soul :7700 (the pin), **`zvec` vector-DB memory backend** (#1158) |
| **Channels** | Slack native live task cards + Block Kit + reaction triggers + reconnect supervision (#1166/#1195/#1238), WhatsApp LID-native addressing + media streaming (#1144/#1228/#1233), Nostr NIP-29 groups (#1168), durable calendar/channel/email connectors (#1190), **telephony via Twilio** (#920), Fastmail MCP OAuth |
| **Voice** | whisper-local STT provider (#981), OpenAI realtime guidance (#984) |
| **Providers** | GPT-5.6, Kimi K3, MiniMax M3, NEAR AI Cloud, Luna routing |
| **Ops** | feedback-collection infra (#1174), managed Files library + Settings browser (#1206), Markdown copy + session export (#1176), Ed25519 challenge-response node identity / TOFU (#979), NetBird + Cloudflare Tunnel remote access (#1002/#1008) |

Workspace is ~270K LOC across 59 crates; agent runner ~7.5K LOC.

## Overlap with b00t

| moltis | nearest b00t | verdict |
|---|---|---|
| Multi-channel human comms (Slack/WhatsApp/Matrix/Nostr/telephony/voice/email/CalDAV) | *none* — b00t is CLI / MCP / NATS | **No overlap. moltis is the hive's human front-door.** |
| ACP agent over stdio (`crates/acp/`) | `b00t chat` ACP, `pi --mode rpc`, opencode ACP | **Same protocol** — moltis plugs into the existing ACP mesh like pi/opencode. Register it. |
| Remote/multi-backend sandbox (Firecracker/Daytona/Vercel) | `b00t sh` audited exec + podman guards; `b00t hive` | moltis is well ahead (microVM isolation). **Candidate to back b00t's exec-isolation tier.** |
| `crates/cron` durable channel-aware scheduler | `b00t maintenance` daemon, `b00t task`, systemd `.timer`, `CronCreate` (cloud) | **Complementary.** moltis-cron → human-facing recurring reports; `b00t task` → machine work. |
| Memory: `B00tSoulWriter` (→ soul :7700) + `zvec` | `b00t soul` K/V, grok RAG, NeumannStore, codebase-memory-mcp, irontology | Keep moltis on **b00t-soul** (already wired, `key_prefix = "moltis/"`). **Do not also run `zvec`** — a second hive vector store competes with grok/irontology. |
| MCP (OAuth client secrets, Fastmail) | `b00t-mcp`, MCP datums, `pi-mcp-adapter` | Complementary. moltis carries OAuth flows b00t-mcp doesn't. |
| Instrumentation: Langfuse / OTLP / Prometheus | ledgrrr FOCUS, `b00t soul`, historian | Partial. Point moltis OTLP at the same Grafana; keep cost/finops in ledgrrr. |
| Node identity: Ed25519 challenge-response / TOFU (#979) | hive peers + NATS operator/JWT auth (#1235) | Overlap. Unify later; low priority. |
| Remote access: NetBird / CF Tunnel / Tailscale | CF Workers/DNS via wrangler; no tunnel | moltis ahead — useful for exposing the hive off-LAN. |

## moltis as a standing hive citizen — recurring tasks

`b00t@moltis-agent.service` stays the always-on host (after `b00t-soul.service`).
Recurring work uses **moltis's own durable cron** (`moltis cron add …`), not systemd
timers — because every job's value is *delivering to a human channel*, which
moltis-cron does natively and `b00t task` does not.

| job | cadence | what it does | delivery |
|---|---|---|---|
| `hive-health-digest` | hourly | `b00t hive status` + tail `.b00t/hive-watchdog.jsonl` + 3 s `nats sub b00t.hive.mesh.health.>` → GPU heat/temp/util, service states, crash-loop trips, NRestarts deltas | Slack #ops |
| `pr-triage` | daily 09:00 | `gh pr list` + `b00t task list` → unreviewed / mergeable / stale-branch PRs; forecast-accuracy digest from `.b00t/forecasts.jsonl` + ledgrrr FOCUS | Slack #ops + `b00t task add` for anything needing action |
| `vendor-drift-watch` | weekly Mon | every `vendor/*` submodule pin vs its `origin/main`; report which advanced + commit range (this doc's own trigger) | Slack #ops + `b00t task add` |
| `soul-snapshot` | daily 23:00 | read moltis's `moltis/` namespace from soul :7700, project to JSONL/Markdown (moltis session-export), commit to a memory archive | git |
| `crash-escalation` | **event** (not cron) | subscribe `b00t.hive.mesh.health.crashloop`; on a trip, place a Twilio call / WhatsApp to the operator | phone — the one thing b00t cannot do itself |

Config surface: `_b00t_/moltis-standing-tasks.hive.toml` (this PR) lists the jobs as
`moltis cron add` recipes + the `[b00t.hive.service]` host. Activation:
`b00t hive activate moltis-standing-tasks`.

## Follow-ups (not in this PR)
- Vendor link reconciled: `.gitmodules` + `moltis.agent.toml` now both point at
  `elasticdotventures/moltis-b00t` (fork of `moltis-org/moltis`). Local `.git/config`
  may still override to the synced `app4dog/moltis-b00t` — run `git submodule sync` to reset.
- `moltis.agent.toml` points model at litellm `:1234` / `local-litellm`; the live
  gateway is `cli-proxy-api` `:1234` and the local slot is qwen38 `:8001`. Re-point.
- Register moltis in `b00t chat` / the ACP mesh now that `crates/acp` is stdio-ready.
