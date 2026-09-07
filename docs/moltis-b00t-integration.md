# m0ltis ⇄ b00t — ecosystem charter, capability overlap & standing-task integration

`vendor/moltis-b00t` (submodule; `.gitmodules` → `elasticdotventures/moltis-b00t`, a
fork of `moltis-org/moltis`; some checkouts locally override `.git/config` to the
synced `app4dog/moltis-b00t` fork)
had drifted ~40 commits / 7 weeks past the b00t pin `5d171bb1` (2026-07-15).
This bumps it to `4829e6a7` (2026-09-02) and proposes how moltis earns a standing
seat in the hive.

## m0ltis — ecosystem charter (operator directive 2026-09-07)

**Nomenclature (adopt everywhere; the two are entangled — always link both):**

| term | meaning |
|---|---|
| **moltis** | the **upstream vendor** project — `moltis-org/moltis`. "A secure persistent personal agent server in Rust." Not ours. Vendored (a fork) at `vendor/moltis-b00t`. Datum: `_b00t_/datums/VENDOR-MOLTIS-B00T.tomllmd`. |
| **m0ltis** | the **b00t ecosystem variant** — moltis + b00t's plugins: soul K/V memory backend, ACP-mesh agent, hive standing-tasks + bouncer gates, ch0nky model slot, provider→ufo-types lowering. Top-level b00tyverse ecosystem project. Datum: `_b00t_/m0ltis.repo.toml`. |

m0ltis is b00t's **secure integrated alternative to OpenClaw** (the TypeScript/Node
"broad gateway, channel, node, and app ecosystem"). Single Rust binary, no npm
plugin-marketplace supply-chain surface, sandboxed-by-default, keys never leave the
host. It is also the hive's **multi-channel human front-door** — the capability row
below with "no overlap".

**Policy:**

1. **Assist the excellence of moltis (upstream).** No hostile fork. The fork carries
   b00t adaptations and *stages patches upstream* (fork-fix-forward); generic fixes
   are PR'd to `moltis-org/moltis`, only b00t glue stays in the fork. Keep the fork
   delta minimal so a rebase is always cheap. Single-maintainer / bus-factor risk on
   upstream is **accepted for now** — the auditable Rust core is the mitigation.
2. **Track nightly OR LTS.** "System normal" follows the fork's `main`
   (nightly-equivalent) via the two-stage sync CI below. The latest dated release
   (e.g. `20260902.03`) is the LTS-equivalent pin for conservative deploys.
3. **External providers go through the b00t agent + ai subsystems.** Voice STT/TTS,
   browser backends, LLM providers, MCP OAuth — none consumed ad hoc. Canonical
   path: *provider adapter → b00t agent/ai subsystem → lowered types in `ufo-types`
   (`iso_ir` / `stereotype` / `sysml` / `mbse`)*. A provider that cannot be lowered
   to a `ufo-types` representation is opt-in only, outside "system normal".
4. **Roadmap: m0ltis graduates to its own sub-superproject workspace** — a
   first-class b00tyverse ecosystem with its own Cargo workspace / superproject repo;
   the b00t plugin crates split out of the vendored fork; `vendor/moltis-b00t`
   reduced to the pinned upstream substrate underneath it.

### Two-stage upstream-sync CI

| stage | where | file | does |
|---|---|---|---|
| 1 | fork `elasticdotventures/moltis-b00t` | install from `docs/moltis/fork-upstream-sync.yml` → `.github/workflows/upstream-sync.yml` | weekly (Mon 05:00 UTC) merge `moltis-org/main` (or a release tag) into fork `main` via PR; replays the b00t delta on fresh upstream |
| 2 | `_b00t_` | `.github/workflows/moltis-upstream-sync.yml` | weekly (Mon 07:00 UTC) when fork `main` is ahead of the pinned `vendor/moltis-b00t` SHA, open a submodule-bump PR here → triggers `datum-validate-graph.yml` |

`b00tyverse-map.skill.tomllm` carries the portfolio entry (promoted `vendor-fork` → `platform`).

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
- **Install the stage-1 workflow in the fork** — copy `docs/moltis/fork-upstream-sync.yml`
  to `elasticdotventures/moltis-b00t/.github/workflows/upstream-sync.yml`.
- **Stand up the m0ltis sub-superproject workspace** — split the b00t plugin crates
  out of the vendored fork; reduce `vendor/moltis-b00t` to the upstream substrate.
- **Wire the provider→ufo-types lowering path** in the b00t agent/ai subsystems.
- Register m0ltis in `b00t chat` / the ACP mesh now that `crates/acp` is stdio-ready.
- Vendor link: `.gitmodules` + `moltis.agent.toml` + `VENDOR-MOLTIS-B00T.tomllmd` now
  all point at `elasticdotventures/moltis-b00t`. Local `.git/config` may still override
  to `app4dog/moltis-b00t` — run `git submodule sync` to reset.
- Do **not** run moltis's `zvec` vector store — a second hive vector DB competes with
  grok/irontology; m0ltis stays on b00t-soul (`key_prefix = "moltis/"`).
