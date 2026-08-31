# Agent-Scoped Token Issuance (#1104)

**Date:** 2026-08-31
**Status:** Implemented, generalized to all six #1102 shard kinds.
**Context:** Implements the "operator holds root trust; short-lived,
narrowly-scoped tokens are issued from it" flow from
elasticdotventures/_b00t_#1104, scoped to the #1102 shard taxonomy
(project/system/agent/skill/tool/datum). Supersedes an earlier unmerged
pilot (`origin/feat/agent-scoped-token-issuance`, commit `aa1a425e`) which
proved the same flow end-to-end for the `datum` shard-type only.

## Goal

Give individual AI agents their own scoped, revocable identity/tokens
instead of one shared blanket credential, with memory-shard access (RBAC)
and cost tracking (cake) unified under one issuance flow — across all six
#1102 shard kinds, not just one.

## Architecture

```
b00t ai agent token request --agent <id> --scope <kind>:<id> --cost <N>
        │
        ▼
1. cake balance check (CakeLedger::balance — fail before privilege)
        │  insufficient → deny immediately, no k8s calls, no journal entry
        ▼
2. ensure ServiceAccount "agent-<id>" in namespace "b00t-agents" (idempotent)
        ▼
3. ensure ClusterRole "role-shard-access" exists (idempotent, embedded YAML,
   shared across ALL six shard kinds — not one ClusterRole per kind)
        ▼
4. ensure RoleBinding: ServiceAccount → role-shard-access, labeled with the
   exact shard kind + id this binding authorizes (idempotent)
        ▼
5. k8s TokenRequest API → short-lived (15 min) SA token
        ▼
6. record issuance as a double-entry transaction, appended to
   ~/.b00t/ledger/agent-tokens.beancount (dedicated file, never mixed with
   real financial records) — this debits the agent's cake balance for `cost`
        ▼
7. print/return the minted token
```

Enforcement is via k8s's TokenReview API, wired into `b00t-cli datum show`
via `--as-agent-token <token>` (scoped to `datum:<name>`) — mechanically
extensible to any other soul-shard-backed read path.

## Why one ClusterRole, not six

The unmerged pilot minted a per-shard-kind marker ClusterRole
(`role-shard-datum`). Generalizing that 1:1 to six kinds would mean six
near-identical marker roles with no real access difference between them —
pure sprawl. Instead, one `role-shard-access` ClusterRole is shared across
all kinds; the specific kind + id a RoleBinding authorizes is carried as
labels (`b00t.elastic.ventures/shard-kind`, `b00t.elastic.ventures/shard-id`)
on the RoleBinding itself, checked by `authorize_shard_token`'s TokenReview
path. A RoleBinding is still minted per (agent, kind, id) — only the
ClusterRole is shared.

## Why not a dependency on `ledger-core`

The original design intent (and the unmerged pilot) called
`ledger_core::journal::JournalTransaction::from_agent_token_issuance`
directly. Attempting to wire that up here surfaced a real, pre-existing
problem: `ledger-core`'s optional `arc-kit-au` → `msft-agent-gov-ledgrrr` →
`agentmesh` dependency chain (which Cargo resolves regardless of feature
flags, since path-dependency manifests are parsed unconditionally) hard-pins
`agentmesh v3.5.0`'s required `serde =1.0.228`, which conflicts with the
rest of this workspace's `serde 1.0.229`.

This is a cross-cutting version conflict inside `vendor/ledgrrr` itself,
unrelated to agent-token issuance, and out of scope to fix here — other
sessions are actively working in that submodule. `b00t-cli/src/agent_token.rs`
instead reproduces the exact same double-entry beancount shape locally
(`AgentTokenJournalEntry`, tests pinned against `ledger-core`'s own test
expectations for byte-identical output). The on-disk ledger format is
unaffected; swapping in the real `ledger-core` dependency once the
`agentmesh`/serde conflict is resolved is a drop-in change, not a format
migration.

## Components

### 1. `b00t-cli/src/agent_token.rs` — issuance + enforcement module

Implements the 7-step flow above and `authorize_shard_token` (TokenReview +
RoleBinding-label check), exposed as
`b00t ai agent token request --agent <id> --scope <kind>:<id> --cost <N>`
(wired via `AiCommands::Agent { Token { Request { .. } } }` in
`b00t-cli/src/commands/ai.rs`). Depends on #1102's `crate::soul_scope`
(`SoulScope`/`ShardKind`) for the scope taxonomy and on the existing
`kube`/`k8s-openapi` dependencies already present in `b00t-cli/Cargo.toml`.

### 2. `b00t-cli/src/commands/ai.rs` — CLI surface

`AiCommands::Agent { Token { Request } }`, taking `--agent`, `--scope
<kind>:<id>`, `--cost`. Prints a pre-flight cake-balance line before
issuance. Deliberately does **not** wire `b00t-cli budget`'s
stack-scoped controller in here — that subsystem is per-job-stack, not
per-agent, and there's no natural stack for an ad-hoc token request to
belong to without a separate design decision. Left as a follow-on rather
than a forced, semantically-mismatched check.

### 3. `b00t-cli/src/commands/datum.rs` — enforcement wiring

`datum show --as-agent-token <token>` calls `authorize_shard_token(token,
&SoulScope::new(ShardKind::Datum, datum_name))` before reading. Mechanically
extensible: any other command backed by a #1102 soul shard can gate the
same way once it has a natural `SoulScope` to check against.

## Not implemented in this pass

- The `ledgerr_authorize_agent_token`/`ledgerr_release_credential` MCP
  domain-tool surface described in #1104's original proposal (issuance as a
  `vendor/ledgrrr` transaction, exposed over MCP) — requires new code in
  that submodule, explicitly deferred and out of scope while other sessions
  are active there. b00t-cli's implementation is fully local (direct k8s +
  local ledger file, no MCP round-trip) and functional for internal use
  without it.
- Cost-menu / discoverable "what can I afford" surfacing beyond the single
  pre-flight cake-balance line — the issue's fuller "menu of affordable
  services" vision needs a broader design (what counts as a "service," how
  cost ceilings are declared per skill/tool) that's out of scope for this
  pass.
- Extending enforcement beyond `datum show` to other soul-shard-backed
  commands — mechanical once needed, not done preemptively (YAGNI).
