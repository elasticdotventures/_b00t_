# ledgrrr multi-tenant service — tenant/org identity + access model

Task: b00t-cli task #175 (parent initiative). This spec covers sub-project 1
of 5; see "Relationship to the larger initiative" below for the full split.

## Context

`b00t.promptexecution.com` predates `ledgrrr`. `ledgrrr` itself
(github.com/PromptExecution/ledgrrr, vendored at `vendor/ledgrrr`) was
designed as a desktop `.mcpb` for Claude — a local-first bookkeeping/
cost-tracking control plane (typed ontology graph + Rhai rules + MCP tools +
Mermaid/isometric visualization). Confirmed by reading ledgrrr's own PRDs
(PRD-3 through PRD-11, PRD-HANDOVER): none mention a cloud or multi-tenant
backend. The most recent P0 spec (PRD-11) is explicitly scoped to "Desktop
Agent, MCPB Bootstrapper, Office Diagram Playbook, and Local Simulation
Runtime." This is a real, unaddressed gap, not a duplicated effort.

The operator has decided ledgrrr needs a common cloud backend: search, a
centralized database multiple desktop instances can contribute to and that
can be backed up, and (eventually) the full ledgrrr feature set exposed as a
hosted, multi-tenant service. This spec is the foundation that everything
else in that direction depends on: **who is a tenant, how are tenants
structured internally, and how is access controlled — before any data,
search, or ledgrrr-feature-specific design is possible.**

### Relevant existing infrastructure (verified live during this session)

- Cloudflare account `f00c391669432ae2a423c04a001dab2d`
  ("Ops+cloudflare@elastic.ventures's Account").
- D1 database `b00t-agents` (id `86bb3c9d-309a-4d27-8856-38934dd316b1`) —
  production, already has an `agents` table (hive-agent identity, NATS JWT
  credentials, seeded roles) and an `mcp_key` column added this session.
- KV namespaces `b00t-users`, `b00t-sessions`, `b00t-tenant-configs` — all
  three provisioned, all three currently **empty** (verified via
  `wrangler kv key list` this session). No existing schema to migrate.
- `telnyx-sms-forwarder` and this session's `telnyx-fax-handler` /
  `b00t-mcp-vault` Workers — proof real Workers already run against this
  account.

### Related GitHub issues

- **#1104** (open) — "agent-scoped tokens: memory access + scoped
  cloud/service access + cost, authorized via ledgrrr." Proposes exactly the
  token-issuance mechanism this spec extends: agent-scoped tokens rooted in
  an operator's trust anchor, scoping soul-shard memory access
  (#1102's taxonomy), enforcing access at the token itself, and gating
  issuance against `cake`/`budget` balance. Proposed implementation locus is
  ledgrrr itself, via a `ledgerr_authorize_agent_token` domain tool. #1104 is
  scoped to **one operator, many agents** — it has no concept of multiple
  organizations/tenants. This spec's token model is a superset: tenant is a
  new outermost scoping dimension, and everything #1104 specifies for a
  single agent's token still applies *within* a tenant.
- **#1102** (referenced by #1104) — scoped soul shards (project / system /
  agent / skill / tool / datum taxonomy). Out of scope here; referenced by
  the token model.
- **#1103** (referenced by #1104) — canonical cross-cloud shell rooted in the
  operator's SSH key as a single trust anchor. This spec generalizes that
  pattern: each top-level tenant gets its *own* root of trust, not one
  global operator root.
- **#788** (closed but unintegrated) — Proxy-Pointer-RAG never wired into
  b00t/ledgrrr. Out of scope for this spec (sub-project 4); confirmed still
  a real gap.

This spec, once implemented, is intended to close the tenant/access-control
portion of #1104 (not duplicate it) by giving it a tenant dimension to sit
inside.

## Relationship to the larger initiative

The full "ledgrrr multi-tenant cloud service" ask decomposes into five
sub-projects, in dependency order. Only sub-project 1 is specced here; the
rest are named for context and get their own spec when their turn comes.

1. **Tenant/org identity + access model** (this spec) — foundation.
2. **Cake ledger per-org** — extends the in-progress consolidation of
   `b00t-c0re-hierarchy/src/cake_economy.rs` into `b00t-cli/src/cake_ledger.rs`
   (active, uncommitted merge in a separate worktree as of this session —
   not touched here) with an org/tenant dimension. Depends on sub-project 1.
3. **Centralized/synced data backend** — search + multi-desktop-contribution
   + backup layer that ledgrrr desktop instances write into. Depends on
   sub-project 1 for tenant scoping.
4. **Proxy-pointer-RAG + agentic services** — closes #788; built on top of
   sub-project 3.
5. **SysML-v2 "lowered-types"** — formally lowering b00t's `Satisfies<C>`/
   UFO-stereotype facts into SysML-v2 model elements. Needs its own research
   spike first: "lowered-types" is not a defined term anywhere in the
   codebase today, and the adopted `sysml-v2-parser` crate's own spike
   verdict was "adopt-with-caveats — not yet mature enough to be a silent,
   load-bearing dependency." The concrete existing seed is
   `vendor/ledgrrr/crates/ufo-types/src/sysml.rs`, which already validates
   `holon-viz`-emitted SysML-v2 text via `Satisfies<SysmlV2Syntax>`, and its
   test suite already encodes two real `holon-viz` emitter bugs (`block def`
   vs `part def`; a `//` comment swallowing a closing `}`) as regressions.

## Goals

- A tenant can be a **personal account** or an **organizational account**,
  each with independently configurable settings and available services.
- Organizational tenants support **hierarchical business units** — arbitrary
  depth, not just one flat level of sub-grouping.
- **Cross-tenant data isolation is structural**, not enforced only by
  application logic — there must be no code path capable of querying across
  two tenants' data by accident (e.g. a missing `WHERE tenant_id = ?`
  clause).
- Access control within a tenant (which agent/member may touch which
  business unit / resource) is **hierarchical and transitive** — membership
  in a business unit implies scoped access to that unit and, per policy, its
  descendants.
- The hierarchy mechanism is **general-purpose from the start** — business
  units are one *kind* of node in a tree that later also covers
  directory-style and tag-style grouping, so a second, parallel hierarchy
  mechanism is never needed.
- Token issuance and validation build on **existing b00t machinery**
  (`DatumStore::query()`'s Horn-clause-shaped conjunction pattern) rather
  than a new, bespoke access-check mechanism.

## Non-goals

- Billing, pricing, or resale mechanics — explicitly out of scope per
  operator direction (2026-08-23).
- Cake ledger implementation itself (sub-project 2) — this spec defines
  *where* per-node cake balances are queried from (a rollup view over the
  node tree), not the ledger's own transaction/entry mechanics.
- Search, the centralized data backend, RAG, and SysML lowering
  (sub-projects 3–5).
- UI/dashboard for managing tenants or business units.
- SSO/OAuth federation for human users logging into an org account — this
  spec defines the *data model and access-control primitives*; how a human
  authenticates to obtain their first token is a separate, later concern.

## Architecture

### Two-tier storage: D1 registry + per-tenant Durable Object

**Global registry (Cloudflare D1, extending the existing `b00t-agents`
database):** a `tenants` table is the *only* data visible outside a tenant's
own boundary — just enough to route a request to the right Durable Object.

```sql
CREATE TABLE tenants (
    id TEXT PRIMARY KEY,              -- uuid
    kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')),
    display_name TEXT NOT NULL,
    root_do_id TEXT NOT NULL,         -- Durable Object id (hex), one per tenant
    created_at TEXT NOT NULL          -- ISO 8601
);
```

No parent/child relationship exists at this level — business-unit hierarchy
lives *inside* a tenant's own Durable Object, never between tenants. A
personal account and an organizational account are both just a `tenants` row
with a different `kind`; nothing else in the registry schema distinguishes
them (the difference is expressed in what settings/services their DO exposes
— see "Personal vs. organizational settings" below).

**Per-tenant Durable Object, with SQLite storage:** one DO instance per row
in `tenants`, addressed by `root_do_id`. This is the hard security boundary:
Cloudflare's DO addressing model means a request can only ever open one DO
instance at a time by its id — there is no API surface, accidental or
otherwise, that can join or scan across two tenants' DOs. Everything scoped
to a single tenant — business units, membership, per-node settings, and (per
sub-project 2) cake balances — lives in that DO's own embedded SQLite
storage, fully relational within the tenant's boundary.

```sql
-- Inside each tenant's own DO SQLite storage.
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,              -- uuid
    parent_id TEXT REFERENCES nodes(id),  -- NULL for a tenant's root node(s)
    kind TEXT NOT NULL CHECK (kind IN ('business_unit', 'directory', 'tag')),
    name TEXT NOT NULL,
    settings_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE members (
    agent_id TEXT NOT NULL,           -- references b00t-agents.agents.id (D1, cross-tier by value only)
    node_id TEXT NOT NULL REFERENCES nodes(id),
    role TEXT NOT NULL,               -- e.g. 'owner', 'member' — enforcement detail, not fixed here
    PRIMARY KEY (agent_id, node_id)
);

CREATE INDEX idx_nodes_parent ON nodes(parent_id);
CREATE INDEX idx_members_agent ON members(agent_id);
```

`business_unit`, `directory`, and `tag` are all rows in the same `nodes`
table, distinguished only by `kind` — a business unit is not structurally
different from a directory or a tag, it just carries organizational-access
semantics by convention. This means directory-style or tag-style grouping
(named as a future need, not designed in depth here) can be added later
without a schema migration or a second hierarchy mechanism — it reuses
`nodes` as-is.

### Personal vs. organizational settings

A `kind = 'personal'` tenant's DO is provisioned with a single root `nodes`
row (no business-unit sub-structure expected, though the schema does not
forbid it) and a settings/services set appropriate to an individual. A
`kind = 'organizational'` tenant's DO supports the full `nodes` tree depth
and a broader settings/services set. The specific list of what settings or
services differ between the two is a product decision for whichever
sub-project actually exposes services (3 onward) — this spec's contribution
is only that the `tenants.kind` column and the per-DO settings model are
capable of expressing that difference; it does not enumerate the settings
themselves.

### Authorization as Horn/FOHH conjunction, not ad-hoc checks

b00t's `DatumStore::query()` (`b00t-cli/src/datum_store.rs`) already
implements a Horn-clause-shaped conjunction API (`.proves_role().depends_on(...).run()`).
A prior gap identified in `_b00t_/learn/rustc-lowering-to-logic.md`: role/
blessing authorization (`b00t blessing --manifest --role <R>`) needs
`forall`/`if` transitive quantification (FOHH, not just Horn) but is
implemented today as an ad hoc graph walk.

This spec's authorization question is another instance of exactly that gap:
*is agent A, member of node B, authorized for shard/service S*, where B may
be an arbitrarily deep descendant of the node A actually holds membership
in. Expressed as a goal:

```
forall<A, B, N, S> {
    if (MemberOf(A, B), AncestorOf(B, N), GrantsAccess(N, S)) {
        Authorized(A, S)
    }
}
```

`AncestorOf` is the transitive-closure part that plain Horn resolution
can't express. Rather than hand-rolling a bespoke graph BFS for this one
case, the DO implements it as a **recursive CTE** against its own SQLite
(the concrete near-term answer), with the FOHH framing preserved as the
honest description of what that CTE is actually computing — per the
rustc-lowering datum's own recommendation, a future generalization (if
this pattern recurs elsewhere in b00t) should model itself after Chalk's
SLG resolution rather than each call site inventing its own graph walk.

The CTE below proves the `MemberOf ∧ AncestorOf` conjunction — the
transitive-membership part this spec is responsible for. `GrantsAccess(N, S)`
is deliberately a separate check, not folded into the same query: this spec
does not define the shard/service grant vocabulary (which shard types or
services a node's `settings_json` can declare is a product decision for
sub-projects 3 onward, per Non-goals). Token issuance therefore applies
`GrantsAccess` as a second, subsequent check — read the target node's
`settings_json`, confirm each requested shard in `requested_shards[]` is
present in whatever that node declares — *after* this CTE confirms the
agent has a valid membership path at all. An agent with no membership path
never reaches the `GrantsAccess` check; an agent with a membership path but
an ungranted shard is rejected at that second step, not silently allowed
through on ancestry alone.

```sql
-- Recursive CTE: does agent :agent_id have any membership path (direct or
-- via an ancestor) to node :target_node? Proves MemberOf ∧ AncestorOf only —
-- the caller still applies GrantsAccess separately (see above).
WITH RECURSIVE ancestors(id) AS (
    SELECT :target_node
    UNION ALL
    SELECT nodes.parent_id FROM nodes JOIN ancestors ON nodes.id = ancestors.id
    WHERE nodes.parent_id IS NOT NULL
)
SELECT 1 FROM members
WHERE members.agent_id = :agent_id
  AND members.node_id IN (SELECT id FROM ancestors);
```

### Token issuance flow (extends #1104)

1. Caller requests a token for `(tenant_id, node_id, requested_shards[])`.
2. Registry lookup: `tenants` (D1) resolves `tenant_id → root_do_id`.
3. Request opens the corresponding Durable Object.
4. The DO runs the recursive-CTE membership check above for the requesting
   agent against `node_id` (proves `MemberOf ∧ AncestorOf`). No membership
   path → reject.
5. The DO checks `GrantsAccess`: each entry in `requested_shards[]` must be
   present in `node_id`'s (or the matched ancestor's) `settings_json`-declared
   grants. Any ungranted shard → reject, even though step 4 passed.
6. If both checks pass, the DO consults `cake`/`budget` (per #1104's existing
   proposal — unchanged by this spec) to confirm the requested shards are
   affordable, and mints a token scoped to
   `(tenant_id, node_id, shard_types[])`.
7. The token is opaque outside the DO. Every subsequent use re-presents it
   to the *same* DO (`root_do_id` is embedded in the token) for validation —
   there is no separate, cacheable "is this token valid" check that could
   drift from the DO's own state.
8. Revocation is a single-row delete/update in that one tenant's SQLite.
   Revoking one agent's access is structurally incapable of touching another
   tenant's DO, because it never addresses one.

### Cake balance rollup (interface only — ledger mechanics are sub-project 2)

Per-node cake balance is a read-time recursive CTE summing leaf-level
(agent-level, already real via `cake balance --agent <name>`) transactions
up through `parent_id` to the queried node — computed on read, not
materialized, at this scale. Sub-project 2 owns the actual transaction/entry
schema; this spec only commits to the shape of the query it must support
(sum over a node and all its descendants) so that sub-project 2's design
isn't blocked on guessing what the tree looks like.

### Cross-tenant isolation is structural, not app-logic

There is no query, index, or API surface in this design that can address
more than one tenant's DO in a single operation. A legitimate cross-tenant
need (e.g. an operator's own admin view across all tenants) is only possible
by explicitly enumerating tenants from the D1 registry and querying each
DO individually, then combining results in application code — never a
single shared-table scan. This is the property the operator specifically
asked for ("cross-tenant queries must be handled separately for security
reasons") and it is enforced by Cloudflare's DO addressing model itself, not
by convention or a `WHERE tenant_id = ?` clause that could be forgotten.

**Precise scope of this guarantee** (amended after implementation review):
the DO-addressing property above holds unconditionally *given* the
`tenants` registry row is correct — `root_do_id` is read from a shared,
multi-writer D1 (`b00t-agents`, also bound by the `b00t-mcp-vault` Worker),
so the actual guarantee is "DO addressing is structurally isolated,
conditioned on registry-row integrity," not an isolation property with no
external dependency at all. A `UNIQUE` constraint on `tenants.root_do_id`
closes the accidental-collision case (two rows aliasing one DO) at the
schema level; it does not defend against a writer with direct D1 access
deliberately repointing a `root_do_id`. Guarding *that* is an access-control
problem for whatever holds write access to `b00t-agents` itself, out of
this sub-project's scope.

## Error handling

- **Tenant not found** (bad `tenant_id` in a token request): D1 registry
  lookup returns no row → reject before any DO is opened. No DO is ever
  addressed for a nonexistent tenant.
- **Unauthorized** (agent has no membership path to the requested node):
  the recursive CTE returns zero rows → token issuance refused. This is
  indistinguishable, by design, from "node does not exist" to the caller
  (avoids leaking node existence to unauthorized agents) — the DO itself
  logs the real reason internally.
- **Orphaned node** (`parent_id` referencing a deleted node): prevented at
  the schema level — `nodes.parent_id` is a foreign key to `nodes.id`
  within the same DO's SQLite; deleting a node with children must either
  cascade or be refused (this spec requires refusal — a node with children
  cannot be deleted directly, mirroring `DatumStore::validate_references()`'s
  existing "don't allow dangling edges" posture in the b00t codebase).
- **D1 registry write failure during tenant creation**: tenant creation is a
  two-step process (D1 row insert, then DO provisioning). If DO
  provisioning fails after the D1 row is committed, the tenant row exists
  with a `root_do_id` pointing at an uninitialized DO — the DO's own first
  access lazily initializes its schema (standard Cloudflare DO pattern), so
  this is self-healing rather than requiring a two-phase-commit workaround.

## Testing

- **D1 registry**: standard schema/query tests against a local D1 instance
  (`wrangler d1` local mode), covering tenant creation, lookup by id, and
  the "tenant not found" rejection path.
- **Per-tenant DO authorization**: unit tests against the DO's SQLite logic
  directly (constructible in isolation, no live Cloudflare account needed)
  covering: direct membership, membership via an ancestor node (the
  transitive case), non-membership, and the orphaned-node-deletion
  refusal.
- **Cross-tenant isolation**: an integration test asserting that no function
  in the DO or registry layer accepts more than one `tenant_id`/`root_do_id`
  in a single call — this is a structural property, so the test is really a
  code-shape assertion (grep/lint-style, or a type-level check if the
  implementation language supports it) rather than a runtime behavior test.
- **Cake rollup query shape**: a test against a hand-built `nodes` tree with
  known leaf balances, confirming the recursive-CTE sum matches expected
  totals at each level of the hierarchy — using placeholder transaction data
  since sub-project 2 owns the real schema.

## Open questions for sub-project 2 onward (explicitly not answered here)

- Exact `cake`/`budget` schema changes needed to support per-node rollup
  (sub-project 2).
- What triggers automatic provisioning of a new tenant's DO — self-serve
  signup flow, operator-only creation, or both (out of scope until a
  sub-project actually exposes tenant creation to end users).
- Whether `b00t-tenant-configs` KV plays any role (e.g. as a read-through
  cache in front of the D1 registry lookup for latency). Not required for
  correctness in this spec; left as a future optimization if the D1 lookup
  proves to be a hot path.
