# Agent-Scoped Token Issuance (datum shard pilot)

**Date:** 2026-08-22
**Status:** Implemented (pilot — `datum` shard-type only)
**Context:** Follow-on to
`docs/superpowers/specs/2026-08-22-k0s-agent-identity-control-plane-design.md`
(the k0s control plane + AWS OIDC federation prerequisites work). Implements
the "ledgrrr provides authorization and releases credentials" flow from
elasticdotventures/_b00t_#1104, scoped to the taxonomy in #1102, for exactly
one shard-type (`datum`) end-to-end. The other four shard-types (system,
repo-superproject, agent, skill) are mechanical extensions of this same
pattern and are explicitly out of scope (YAGNI) for this pilot.

## Goal

Give individual AI agents their own scoped, revocable identity/tokens
instead of one shared blanket credential, with memory-shard access (RBAC)
and cost tracking (cake/budget) unified under one issuance flow — proven
end-to-end for the `datum` shard-type before generalizing.

## Architecture

```
b00t ai agent token request --agent <id> --shard datum:<datum-id> --cost <N>
        │
        ▼
1. cake balance check (CakeLedger::balance, reused — fail before privilege)
        │  insufficient → deny immediately, no k8s calls, no journal entry
        ▼
2. ensure ServiceAccount "agent-<id>" in namespace "b00t-agents" (idempotent)
        ▼
3. ensure ClusterRole "role-shard-datum" exists (idempotent, embedded YAML)
        ▼
4. ensure RoleBinding: ServiceAccount → role-shard-datum (idempotent)
        ▼
5. k8s TokenRequest API → short-lived (15 min) SA token
        ▼
6. record issuance as a double-entry transaction in ledger-core,
   appended to ~/.b00t/ledger/agent-tokens.beancount (dedicated file,
   never mixed with real financial records) — this debits the agent's
   cake balance for `cost`
        ▼
7. print/return the minted token
```

Enforcement (this pilot only) is via k8s's TokenReview API, wired directly
into the existing `b00t-cli datum show` command via a new
`--as-agent-token <token>` flag — not a new standalone proxy service.

## Components

### 1. `b00t-cli/src/agent_token.rs` — issuance module

New module implementing the 7-step flow above, exposed as
`b00t ai agent token request --agent <id> --shard datum:<datum-id> --cost <N>`
(wired via `AiCommands::Agent { Token { Request { .. } } }` in
`b00t-cli/src/commands/ai.rs`). Uses the existing `kube`/`k8s-openapi`
dependency already present in `b00t-cli/Cargo.toml` and the existing
`crate::k8s::K8sClient` wrapper pattern (`b00t-cli/src/k8s/`) where
convenient; adds direct `kube::Api<ServiceAccount>` /
`Api<ClusterRole>` / `Api<RoleBinding>` calls for the RBAC-object
lifecycle and a raw `TokenRequest` subresource call for minting.

Budget check happens strictly before any k8s API call or client
construction — verified by a test that runs with no reachable cluster and
asserts the error returned is the budget error, not a connection error.

### 2. `role-shard-datum` ClusterRole — embedded marker Role

Since datum data is not migrated into k8s-native resources (deliberate
scope decision — the datum store stays wherever it already lives), this
Role's `rules:` are intentionally minimal (no real API access granted). Its
only job is to exist as a RoleBinding target that Component 4's TokenReview
check looks for. Defined as an embedded YAML string constant in
`agent_token.rs` and applied (idempotent create-if-missing) by the
issuance flow itself the first time it's needed — no manual `kubectl
apply` step, no separate manifest-distribution story.

### 3. `ledger-core::JournalTransaction::from_agent_token_issuance`

New constructor in `vendor/ledgrrr/crates/ledger-core/src/journal.rs`,
alongside the existing `from_input` (tax-import) constructor — does not
modify `from_input` or its behavior. Does not depend on
`TransactionInput`/`deterministic_tx_id` (those are tax-import-specific);
takes `agent_id`, `shard_ref`, `cost`, a caller-supplied `tx_id`, and
`date` directly. Produces a balanced double-entry beancount transaction:

- `Assets:Cake:<agent_id>` debited by `cost`
- `Expenses:AgentTokens:<shard-type>` credited by `cost` (shard-type is the
  part of `shard_ref` before the first `:`, e.g. `datum` from
  `datum:some-id`)

Entries are appended (via the existing `append_entries` function, reused
unmodified) to a **dedicated** file, `~/.b00t/ledger/agent-tokens.beancount`
— separate from any real tax-ledger data. This lives in a companion PR
against `PromptExecution/ledgrrr` (the submodule's own upstream repo):
https://github.com/PromptExecution/ledgrrr/pull/192.

### 4. TokenReview enforcement in `datum show`

New `--as-agent-token <token>` flag on `b00t-cli datum show`
(`b00t-cli/src/commands/datum.rs`). When present:

1. Calls k8s's TokenReview API with the provided token.
2. If not authenticated/expired → "not authorized for this shard" error
   (distinct from a generic error).
3. Confirms the token's ServiceAccount (from the TokenReview response
   `status.user.username`, format `system:serviceaccount:<ns>:<name>`) has
   a RoleBinding to `role-shard-datum` in `b00t-agents`.
4. If not bound → same "not authorized" error.
5. Otherwise, proceeds with normal `datum show` behavior.

Only `datum show` (the read path) is gated in this pilot — proving the
pattern once, not gating every datum subcommand.

## Design decisions not fully specified in the brief

- **`b00t-agents` namespace creation:** ensured (create-if-missing) as part
  of the ServiceAccount-ensure step, using the same idempotent-create
  pattern as the rest of the flow, rather than assuming it pre-exists.
- **cake debit mechanism:** `CakeLedger` (`b00t-cli/src/cake_ledger.rs`)
  previously had no spend/debit path (only lottery-win credits). Added a
  new `CakeLedger::debit(agent, amount)` method following the existing
  `ON CONFLICT ... DO UPDATE` upsert style used by `resolve_ticket`, with
  an explicit insufficient-balance guard (belt-and-suspenders — the
  primary check is the pre-flight `balance()` read in `agent_token.rs`,
  fail-before-privilege).
- **tx_id generation:** the ledger-core constructor takes `tx_id` as a
  caller-supplied `String` (per the brief) rather than deriving it via
  `deterministic_tx_id` (that helper is tax-import-specific and hashes
  `TransactionInput` fields that don't apply here). `agent_token.rs`
  generates a `uuid::Uuid::new_v4()`-based id, since each token issuance is
  a distinct event, not an idempotent re-import of the same source data.
- **Async wiring:** `b00t-cli`'s `main()` is already `#[tokio::main]`.
  Rather than follow the existing (pre-existing, separately-noted-risky)
  `commands/k8s.rs` pattern of spinning up a nested `tokio::runtime::Runtime`
  and calling `.block_on()` from inside an already-running runtime,
  `AiCommands::execute` and `handle_datum_command`/`handle_show` were
  changed to `async fn` (all previously-sync branches are trivially
  compatible — none of them awaited anything before, they just don't now
  either) and their call sites in `main.rs` were updated to `.await`.
- **TokenReview → RoleBinding lookup:** the TokenReview response's
  `status.user.username` is parsed as `system:serviceaccount:<namespace>:<name>`
  per Kubernetes' standard ServiceAccount username format; RoleBindings are
  then listed in that namespace and checked for a `roleRef` naming
  `role-shard-datum`.
- **Cross-repo submodule change:** since `vendor/ledgrrr` is a real
  submodule of a separate repo (`PromptExecution/ledgrrr`), Component 3's
  change was committed and pushed there directly
  (https://github.com/PromptExecution/ledgrrr/pull/192), and this PR's
  submodule pointer bump references that branch's tip commit.

## Testing

- Unit tests for `JournalTransaction::from_agent_token_issuance`: field
  correctness, and that `to_beancount_entry()` produces a debit/credit pair
  that are exact inverses via the existing `invert_amount` helper.
- Unit test for the fail-before-privilege ordering: an agent with
  insufficient cake balance is denied with the budget error specifically
  (not a k8s connection error), runnable with **no reachable cluster** —
  this is what actually proves the ordering, since a real k8s call
  attempted first would surface as a connection failure in this
  environment, not the budget message.
- `CakeLedger::debit` unit tests (insufficient balance rejected, balance
  decremented correctly on success).
- k8s-dependent integration tests (ServiceAccount/RoleBinding/ClusterRole
  ensure, TokenRequest mint, TokenReview enforcement in `datum show`) use a
  soft-skip pattern: they attempt a real cluster connection first and, if
  unavailable, print a skip notice and return early rather than failing —
  see the PR description for what was/wasn't verified against a live
  cluster in this session.
