<!-- Approved implementation plan (2026-09-10, session_01Cj7a4Bkh8pRQuzcErD1j2E). Program: b00t.promptexecution.com multi-tenant agent-r0le / MCP-proxy platform. This spec covers SP1+SP2+SP3; SP4-SP6 get their own spec->plan cycles. -->

# Plan — b00t.promptexecution.com : multi-tenant agent-r0le / MCP-proxy platform (SP1+SP2+SP3)

## Context

`b00t` gives away the *harness* (public `elasticdotventures/_b00t_`, incl.
`b00t-mcp`). The commercial step is to sell the hard part — **managing agent
harness config at scale**: the MCP-tool universe dwarfs any one host's capacity
and any one agent's usable budget (~40 tools). `b00t.promptexecution.com` is a
**multi-tenant, agent-aware MCP proxy + r0le package manager**:

- A **r0le** = one signed, versioned package `{context, skills, tool allowlist,
  soul/memory grants, model tier, budget ceiling, permissions}` assigned to an
  agent.
- **`b00t-mcp` is the proxy**: tools differ per *harness* and per *r0le*; tool
  discovery; **on-demand enablement** — an agent can't call a tool until it has
  read that tool's docs.
- **Backend MCP servers = on-demand OCI containers**, placed on the cheapest
  cloud per workload (no lock-in), each at `https://<svc>.b00t.promptexecution.com`,
  recorded as a datum. They authenticate and receive the caller's creds.
- **Tenants**: `promptexecution`, `app4dog`, plus **self-serve external**. Agents
  can request **temporary escalations** under guardrails. Eventually agents run
  **cgroup-isolated**.

`b00t-website` (private, submodule of `PromptExecution/infrastructure`) is the
front; it should migrate into the `~/promptexecution` superproject. Backend infra
is in `PromptExecution/infrastructure` `terraform/b00t/` (which carries vestigial
Tailscale fleet content to split into its own module).

**Deferred:** b00t task **#195** (build-plane CI payoff) is *tabled*. Rust
preferred; open source preferred; **the substrate is ~70% present but
disconnected — reuse, don't rebuild.**

## Locked decisions (operator)

| # | Decision |
|---|---|
| **D1** | **ledgrrr and b00t stay fully separate, integrate over HTTP.** ledgrrr = cost/cake ledger; b00t = agent/tool/identity/MCP proxy. b00t calls ledgrrr only for spend-authorization + usage-recording. A **b00t-side client trait + mock impl** (both TS and Rust) means SP1–3 never block on ledgrrr. |
| **D2** | **Hybrid identity.** *Product surface* (customer agents): **RS256 JWT** issued by a Cloudflare Worker + Durable-Object-per-tenant, JWKS at `https://b00t.promptexecution.com/.well-known/jwks.json`, issuance gated on the ledgrrr budget call. *Platform internals* (SP4, out of scope here): SPIFFE/SPIRE + "Entra as hub". Entra is never the customer IdP. |
| **D3** | **On-demand MCP servers = OCI containers, cost-placed, no vendor lock-in** (SP4). SP3's routing layer derives the target host purely from `<svc>.<base-domain>` — no hosting provider hard-coded. Standardize the FastMCP/`rmcp` **Streamable-HTTP** + `/health` contract. |
| **D4** | **First delivery = SP1 + SP2 + SP3**, decomposed into small sub-tasks dispatchable to non-Sonnet `b00t agent` runs (each gets a spec, files, interface contract, isolated verification). SP4–SP6 = roadmap. |

## Reusable substrate (call these, don't reimplement)

- **SP1 is ~60% built.** `PromptExecution/infrastructure` (or its worktree)
  `workers/ledgrrr-tenant-registry/` already has: D1 `tenants` table +
  `migrations/0001`, a `TenantNode` Durable Object (nodes/members tree, recursive
  CTE membership, cake rollup), opaque **HMAC** token issue/verify, routes
  `POST /tenants` · `GET /tenants/:id` · `POST /tokens` · `POST /verify`, and a
  vitest suite. `wrangler.jsonc` binds D1 `b00t-agents` (id `86bb…316b1`, shared,
  `prevent_destroy`) + DO `TenantNode`. **SP1 = evolve this** (HMAC→RS256 JWT,
  JWKS, `r0le` claim, ledgrrr gate, self-serve + seeds) **+ a new Rust client
  crate.** Design ref: `_b00t_/docs/superpowers/specs/2026-08-23-ledgrrr-tenant-identity-design.md`.
- **Identity/authz:** `capability-forge/src/judge.rs` (`EscalationJudge` trait +
  `FakeJudge::always_{grant,deny}` + `OpenAiJudge`) — reuse for SP3 escalation.
  `capability-forge/src/tiers.rs` `SkillTier`. `b00t-cli/src/agent_token.rs` (k8s
  TokenRequest pattern, `authorize_shard_token` via TokenReview — wired only to
  `datum show`). `b00t-c0re-gov::{scope_store,discovery::walk_lazy_chain,
  scope_credential_guard}`. **`jwt_mint.rs` (NATS-JWT) is NOT reusable** — RS256 ≠
  nats-jwt.
- **r0le/blessing:** `b00t-cli/src/whoami.rs::load_role_datum` (→ `RoleDetails{
  skills, compliance, depends_on, entangled_*}` — **make `pub`**);
  `b00t-cli/src/commands/blessing.rs::emit_manifest` (**extract `pub fn
  collect_role_unlocks(b00t_path,&role) -> RoleManifest`** — walks `depends_on`
  via `walk_lazy_chain`, collects each datum's `unlocks: Vec<String>` globs).
  `b00t-cli/src/soul_scope.rs` `ShardKind{Project,System,Agent,Skill,Tool,Datum}`.
- **Datums:** `b00t-cli/src/datum_types.rs` (`DatumType` enum + `datum_type_table!`
  macro + exhaustive `semantic_class`/`implies` matches — a new variant
  compile-forces 3 arms). `b00t-cli/src/boot_datum.rs` (`BootDatum` — `unlocks`,
  `entangled_*`, `type_tags`). `b00t-cli/src/datum_utils.rs::get_all_datums_with_paths`
  (key = filename-minus-ext, rank `.tomllmd`>`.tomllm`>`.toml`, single tree — no
  tenant hook). Fixtures MUST be `.agentprofile.toml` (`.tomllmd` downgrades to
  the generic parser).
- **b00t-mcp:** `mcp_server_rusty.rs::{list_tools (returns all), call_tool
  (captures client_info name only)}`; `mcp_tools.rs::create_mcp_registry_with_notify`
  (slim ~22) / `create_full_mcp_registry` (~56); `b00t_mcp_stack_load/unload` fire
  `tools/list_changed` via `add_post_hook` — **escalation reuses that same
  `notify_fn` Arc**. `acl.rs::AclFilter` (regex allow/deny) is **dead code** —
  revive as a glob matcher. `server_llm.rs::check_access(token,class,action)` +
  `AuthProvider{Dev,Basic,Hydra}` — the shape to mirror for the JWKS provider.
  `Cargo.toml` already has `jsonwebtoken = "10.2"` + `reqwest 0.12`.
  `b00t-c0re-lib/src/mcp_proxy.rs::GenericMcpProxy` is **subprocess-shaped** — SP3
  routing needs a new HTTP-remote sibling, not an extension of it.
- **infra CF substrate** (`terraform/b00t/`): D1 `b00t-agents`, Worker `b00t_api`
  (code by wrangler, TF `import`s + `ignore_changes`/`keep_bindings`), KV
  `b00t-tenant-configs`(empty)/`b00t-sessions`/`b00t-users`, R2 `b00t-historian`,
  apex + `*.b00t.promptexecution.com` DNS (wildcard placeholder "routed by
  Worker"). Secrets = Azure Key Vault `config-global-*` via `data external`.

## Aligned open issues (fold in — do not re-file)

r0le packaging: b00t **#817** (structural role boundaries), **#692** (multi-role
session mgr — "foundation for multi-tenant agent operation from a single
process"), **#862** (deterministic `session-preamble`), **#903/#907/#909** (→
`ufo_types` stereotypes), **#113** (tool pre-injection from `unlocks`), task
**#131** (one skill format), `SIA_COMPARISON.md` (`DatumType::AgentProfile`).
Agent-aware proxy: ledgrrr **#222** (progressive tool scoping —
`docs/progressive-tool-scoping-design.md`), **#224** (`X-Agent-Id`), **#225**
(`check_tool_call` into `tools/call`), **#226** (`request_increase` +
`notifications/tools/list_changed`), **#230**; b00t **#809** (`b00t mcp serve`);
infra **#139** (Cedar/AGT gate). Identity/tenancy: b00t **#1104** (agent-scoped
tokens = memory+cloud+cost), task **#191** (SPIRE root + HA — prereq for SP4),
infra **#140** (assigned session IDs + reaping + "billing rolls up to
superproject"), ledgrrr **#175**. Memory ACLs: **#1102/#1104** ("any agent can
touch any shard"), **#897/#906/#896**. SysML (SP5): task **#181**, **#1177**,
infra **#133**.

## Architecture — three planes

```
PRODUCT PLANE      b00t.promptexecution.com  (CF Worker + DO-per-tenant)   ── D1,D2 · SP1
                   · tenant registry (D1)   · RS256 JWT + JWKS
                   · quota/budget gate → HTTP → ledgrrr (mock)
                        │  signed r0le grant  (JWT: tenant, r0le, scopes, budget_ref, exp≤900s)
PROXY PLANE        b00t-mcp (agent-aware)                                    ── SP3
                   · verify JWT → {tenant, agent, r0le}
                   · list_tools filtered by r0le tool_allowlist (SP2)
                   · call gate: doc-read (unlocks) + budget precheck
                   · runtime escalation → tools/list_changed
                   · route <svc>.b00t.… → backend MCP server (provider-agnostic)
                        │  OCI launch spec  (McpServer datum)
PLATFORM PLANE     on-demand MCP servers (OCI, cost-placed)                 ── D3 · SP4 (roadmap)
```

The **r0le package** (SP2, `DatumType::AgentProfile`) is the artifact the product
plane issues against and the proxy plane enforces.

## Program decomposition (6 sub-projects)

| SP | Name | One line |
|---|---|---|
| **SP1** | Tenant + agent identity plane | evolve `ledgrrr-tenant-registry` worker → RS256 JWT + JWKS + `r0le` claim + ledgrrr gate + self-serve/seeds; + Rust `b00t-c0re-identity` client crate + `b00t identity` CLI. |
| **SP2** | The r0le package | `DatumType::AgentProfile` (`.agentprofile.toml`) — signed bundle of `{tool_allowlist, skills, soul_shard_grants, model_tier, budget_ceiling, permissions}`; `b00t r0le {build,show,verify}`; tenant-namespaced datum resolution. |
| **SP3** | Agent-aware `b00t-mcp` proxy | per-caller JWT → identity in dispatch; `list_tools` filtered by r0le; call-time gate (unlocks + budget); runtime escalation + `tools/list_changed`; provider-agnostic `<svc>.b00t.…` routing. |
| SP4 | On-demand serverless MCP hosting | implement `DatumType::McpServer` w/ OCI launch spec; vendor-neutral placement/spawn control plane (`b00t mcp serve`, #809); per-service DNS + SPIFFE; lazy start / idle stop / cost policy. |
| SP5 | SysML-v2 coherence substrate | enable `store-oxigraph`; finish `OxigraphSparqlSource`; load datum + tenant/r0le/tool/grant graph; SHACL; render via kr0ki. Runs alongside SP1–4. |
| SP6 | Isolation + billing rollup + website | cgroup/ns-isolated per-session containers (extend `agent_token.rs`); usage metering → superproject billing (#140); migrate `b00t-website`, split `tailscale.tf`, wire website CI. |

---

## Frontier tasks — DO FIRST (architecture decisions the ch0nky sub-tasks build against)

| id | decision to make | feeds |
|---|---|---|
| **F1** | Worker rename/rehome: is the identity service `b00t-identity` (own worker) or folded into `b00t_api`? Route split across `b00t.promptexecution.com` between this worker, `b00t-mcp-cloudflare`, and TF's `b00t_api`. Reconcile with "wrangler deploys code / TF adopts bindings" + `prevent_destroy` on D1. | SP1-10 |
| **F2** | Canonical **b00t↔ledgrrr HTTP contract** + JSON Schema: `POST /authorize-spend {tenant,agent,cost,ref}` → `{ok,budget_remaining,reason?}`; `POST /usage {tenant,agent,units,meta}` → `{ok}`. Error codes, idempotency key, cost units. Rust side = shared crate `b00t-c0re-ledgrrr` or inlined? | SP1-03, SP3-06 |
| **F3** | Per-caller identity threading through `rmcp`: the `tower` layer + request-extension design (HTTP), the one-identity-per-process model (stdio), and 900s-token vs multi-hour-session refresh/401 behaviour. | SP3-02a/b |
| **F4** | **Datum signing scheme**: canonical JSON projection of semantic fields (`signature` excluded), key source (reuse SP1 RS256 key vs separate datum-signing key), public key location (same JWKS `kid` namespace?), rotation. Signing raw TOML bytes is wrong — the `.tomllmd`>`.tomllm`>`.toml` rank rule re-serializes files. | SP2-04, SP3-03 |
| **F5** | (may merge into F3) Token lifetime vs long sessions; escalations must be session-scoped, never persisted, never widen the JWT. | SP3-07 |
| **F6** | **Tenant-namespaced datum resolution model**: overlay precedence vs the existing rank rule; key-collision isolation (two tenants' `worker.role.toml`); how `_b00t_` base relates to `tenants/<t>/_b00t_`. Getting it wrong = cross-tenant leak. | SP2-06 |

---

## SP1 — Tenant + agent identity plane (10 sub-tasks)

All under `workers/ledgrrr-tenant-registry/` (TS/Workers) unless noted. Migrations **additive-only**, tested `wrangler d1 … --local` first (D1 `b00t-agents` is shared + `prevent_destroy`).

| id | title / goal | key files | contract (essentials) | dep | verify | sz |
|---|---|---|---|---|---|---|
| **SP1-01** | RS256 keypair as wrangler secret + JWKS endpoint (the trust root). | `src/jwks.ts`(new), `src/index.ts`, `wrangler.jsonc` | `GET /.well-known/jwks.json` → `{keys:[{kty:RSA,use:sig,alg:RS256,kid,n,e}]}`, `max-age=3600`, no auth. `loadSigningKey(env)`, `jwks(env)`. Secrets `JWT_PRIVATE_KEY_PEM` (PKCS#8), `JWT_KID`. | — | `wrangler dev` + `curl …/jwks.json \| jq .keys[0].kty` → `"RSA"`; `pnpm test jwks` | S |
| **SP1-02** | Replace HMAC token with standard 3-part **RS256 JWS**; claims to spec. | `src/token.ts`, `test/token*.test.ts` | claims `{iss:"https://b00t.promptexecution.com", sub=agent_id, tenant, r0le, scopes[], budget_ref, iat, exp, jti}` with `exp-iat≤900`; header `{alg:RS256,typ:JWT,kid}`. `signJwt(env,claims)`, `verifyJwt(env,token)`. | SP1-01 | `pnpm test token` — 3 segments, `exp-iat==900`, tamper→invalid, header `kid` matches JWKS | M |
| **SP1-03** | ledgrrr budget client (worker), **mock-first** via `LEDGRRR_MODE`. | `src/ledgrrr.ts`(new), `test/ledgrrr.test.ts` | per **F2**: `authorizeSpend(env,{tenant,agent,cost,ref})→{ok,budget_remaining,reason?}`, `recordUsage(env,{tenant,agent,units,meta})→{ok}`. `LEDGRRR_MODE=mock` returns `{ok:true,budget_remaining:1e6}`. | F2 | `pnpm test ledgrrr` — mock ok; http mode hits fetch stub with exact body | S |
| **SP1-04** | Rewire `issueToken`: tenant(D1) → open DO → membership+grant → `authorizeSpend`(mock) → `signJwt`. Add `r0le`. **Order is load-bearing.** | `src/token.ts`, `src/index.ts`, `test/token*.test.ts`, `test/tenant-do.test.ts` | `POST /tokens {tenantId,agentId,nodeId,r0le,requestedShards[]}` → `200 {token,budget_remaining}` \| `404` tenant \| `403` non-member \| `402 {error:"budget_exceeded"}`. 404→403→402 order enforced; `scopes` = shards that passed grant check. | SP1-02, SP1-03 | `pnpm test token tenant-do` — member+grant mints; non-member 403 with 0 `authorizeSpend` calls (spy); mock `ok:false` → 402 | M |
| **SP1-05** | `TenantNode` DO: per-agent `r0le` + shard-grant storage. | `src/tenant-do.ts`, `test/tenant-do.test.ts` | table `agent_grants(agent_id,node_id,r0le,shards_json,PK(agent_id,node_id))`. `setAgentGrant`, `getAgentGrant`, `revokeAgent` (row delete), `agentGrantsShards(agentId,nodeId,shards)` (checks `agent_grants` then node `settings_json`). | — | `pnpm test tenant-do` — set/read/revoke; subset-shard grant true/false | M |
| **SP1-06** | Self-serve `POST /tenants` (drop blanket admin gate; keep `/admin/*`); `slug` column; seed `promptexecution` + `app4dog`. | `src/registry.ts`, `src/index.ts`, `migrations/0002_tenant_slug.sql`(new), `scripts/seed.ts`(new), `test/*` | `0002`: `ALTER TABLE tenants ADD COLUMN slug TEXT; CREATE UNIQUE INDEX …`. `POST /tenants {kind,displayName,slug,ownerAgentId}` → `201 {id,slug,…}`; dup slug → `409`. `GET /tenants/:idOrSlug`. `seed.ts` idempotent. | SP1-05 | `wrangler d1 migrations apply --local` + `pnpm test registry bootstrap`; `curl -XPOST …/tenants` → 201, repeat → 409 | M |
| **SP1-07** | `POST /verify` re-presents token to the owning DO (revocation = row delete); `DELETE …/agents/:agentId`. | `src/token.ts`, `src/tenant-do.ts`, `src/index.ts`, `test/{token,isolation}.test.ts` | `POST /verify {token}` → `200 <claims>` \| `401 {error:"invalid signature"\|"expired"\|"revoked"}`. `DELETE /tenants/:id/agents/:agentId?nodeId=` (admin) → `{revoked:true}`. `TenantNode.checkStillGranted(agentId,nodeId,scopes)`. `verify` opens exactly the DO from the claim's `tenant`→`rootDoId`, never scans. | SP1-02, SP1-05 | `pnpm test token isolation` — mint→ok→revoke→401 `revoked`; single `idFromString` call asserted | M |
| **SP1-08** | New workspace crate **`b00t-c0re-identity`** — `TokenSource` trait + HTTP impl + test-keypair mock (also used by SP3 tests). No dependency on the deployed worker. | `b00t-c0re-identity/{Cargo.toml,src/lib.rs,src/{claims,http_source,mock_source}.rs}`, workspace `Cargo.toml` members | `AgentClaims{iss,sub,tenant,r0le,scopes,budget_ref,iat,exp,jti}`; `trait TokenSource{ async fn obtain(&self,&TokenRequest)->Result<String> }`; `HttpTokenSource{base_url}` (`POST {base}/tokens`); `MockTokenSource` (local RSA mint, `jwks_json()`); `verify_jwt(token,jwks_json)->Result<AgentClaims>`; `decode_claims_unverified`. deps `jsonwebtoken=10.2`, `reqwest`(rustls), `rsa`. | SP1-02 (share golden claims JSON) | `cargo test -p b00t-c0re-identity` — mock mints → `verify_jwt` w/ its `jwks_json()` round-trips; expired/tampered → `Err` | M |
| **SP1-09** | `b00t identity` CLI subcommand; defines the env contract SP3 stdio reads. | `b00t-cli/src/commands/identity.rs`(new), `b00t-cli/src/commands/mod.rs`, `b00t-cli/src/main.rs`, `b00t-cli/Cargo.toml` | `b00t identity token --tenant --node --r0le [--shards a,b]` → prints JWT + writes `~/.b00t/identity/<tenant>.jwt`. `b00t identity whoami` → claims table. `b00t identity jwks`. Env: `B00T_IDENTITY_URL` (default `https://b00t.promptexecution.com`), `B00T_AGENT_JWT`, `B00T_IDENTITY_JWKS`. | SP1-08 | `cargo test -p b00t-cli identity::` (mock injected); manual vs `wrangler dev` | M |
| **SP1-10** | Deploy wiring: route + secrets + TF binding-adoption note. | `wrangler.jsonc`, `DEPLOY.md`(new), `workers/cf-workers.just` | route per **F1**; secrets `JWT_PRIVATE_KEY_PEM`, `JWT_KID`, `REGISTRY_ADMIN_KEY`, `LEDGRRR_BASE_URL`, `LEDGRRR_MODE`; `[[migrations]]` tag for the `agent_grants` DO table. | SP1-01…07, F1 | `wrangler deploy --dry-run`; `just … --dry-run` | S |

## SP2 — the r0le package (7 sub-tasks)

| id | title / goal | key files | contract (essentials) | dep | verify | sz |
|---|---|---|---|---|---|---|
| **SP2-01** | `DatumType::AgentProfile` variant + suffix map. | `b00t-cli/src/datum_types.rs` | table row `AgentProfile => ["agent_profile","agentprofile","r0le"] => ".agentprofile"`; `from_type_token("r0le")==Some(AgentProfile)`; `semantic_class==Agent`; `implies==&[Role]` (add arm). | — | `cargo test -p b00t-cli datum_types::` — existing stereotype test passes + suffix round-trip | S |
| **SP2-02** | `AgentProfileSpec` payload struct. | `b00t-cli/src/datum_agent_profile.rs`(new), `lib.rs`/`boot_datum.rs` | `AgentProfileSpec{ tool_allowlist:Vec<String>(globs), skills:Vec<String>, soul_shard_grants:Vec<SoulShardGrant{kind:ShardKind,id,mode:R\|Rw}>, model_tier:Sm0l\|Ch0nky\|Frontier, budget_ceiling:u64, permissions:Vec<String>, signature:Option<DatumSignature{kid,alg:"RS256",sig_b64,signed_fields_hash}> }`. Reuse `soul_scope::ShardKind`. | SP2-01 | `cargo test -p b00t-cli datum_agent_profile::` — fixture `.agentprofile.toml` deserializes; bad `model_tier`→err; missing `signature`→`None` | S |
| **SP2-03** | `b00t r0le build --role <R>` — compose spec from `load_role_datum` + `collect_role_unlocks` + soul grants. Emit unsigned TOML. | `b00t-cli/src/commands/r0le.rs`(new), `mod.rs`, `main.rs`; **`whoami::load_role_datum`→`pub`**; **extract `pub fn collect_role_unlocks(b00t_path,&role)->RoleManifest` from `blessing::emit_manifest`** | `b00t r0le build --role R [--tenant T] [--out P] [--tier …] [--budget-ceiling N]`. `tool_allowlist` = ∪ every discovered skill's `unlocks`; `skills` = discovered skill keys; `soul_shard_grants` = `{Skill:<s>:r}*` + `{Agent:<role>:rw}`; `model_tier` from `--tier` (default `ch0nky`). Output valid `.agentprofile.toml`, `signature` absent. | SP2-02 (+ SP2-06 for `--tenant`) | `cargo test -p b00t-cli r0le::build` vs temp `_b00t_` (`worker.role.toml` + `rust.skill.toml unlocks=["cargo.*"]`) → allowlist has `cargo.*` | M |
| **SP2-04** | Datum signing + `DatumSignature` **canonicalization** (per F4). | `b00t-cli/src/datum_agent_profile.rs`, `commands/r0le.rs`, `b00t-cli/Cargo.toml` | `canonical_bytes()` = `serde_json` of BTreeMap-ordered semantic fields, `signature` **excluded**. `sign(&mut self,kid,pkcs8_pem)`; `verify(&self,jwks_json)` (resolve `kid`, RS256 over `canonical_bytes()`). Key: `B00T_DATUM_SIGNING_KEY_PEM` env or OS keyring `b00t/datum-signing-key`; public half in the SP1 JWKS. | SP2-02, F4 | `cargo test … sign_verify` — sign→verify ok; reorder input list then re-serialize → same `canonical_bytes` → verify ok; flip a glob → `Err` | M |
| **SP2-05** | `b00t r0le show <id>` + `b00t r0le verify <id>`. | `commands/r0le.rs`, `main.rs` | `show <id> [--tenant T] [--json]` → table/JSON. `verify <id>` → exit 0 `✅ signature valid (kid=…)` \| exit 1 + reason (`unsigned`/`unknown kid`/`bad signature`). Resolve via `get_all_datums_with_paths` (or SP2-06 tenant variant), filter `datum_type==AgentProfile`. | SP2-03, SP2-04 | `cargo test … r0le::show_verify`; manual `build --sign --out …` then `verify` → exit 0 | S |
| **SP2-06** | Tenant-namespacing hook for datum resolution (per F6). | `b00t-cli/src/datum_utils.rs`, `b00t-c0re-lib/src/utils.rs` | `pub fn get_all_datums_for_tenant(b00t_path, tenant:Option<&str>, depth) -> Result<HashMap<String,(BootDatum,String)>>`; overlay `tenants/<t>/_b00t_/**` shadows base for the same key; base = fallback; `--strict-tenant` hides base. Emitted keys unchanged (so `r0le build` stays tenant-agnostic). No change to existing callers. | F6 | `cargo test … datum_utils::tenant_overlay` — base + `tenants/app4dog/_b00t_/worker.role.toml` → tenant scan returns a4d one; no-tenant → base; other tenant → base | M |
| **SP2-07** | `AgentProfile` dispatch in `b00t datum show/list/verify` (parity w/ the other 36 variants). | `b00t-cli/src/commands/datum.rs`, `main.rs` | `b00t datum list --type r0le`; `b00t datum show <id> --type r0le`. No new flags. | SP2-01, SP2-02 | `cargo test … datum::` + manual `b00t datum list --type r0le` | S |

## SP3 — agent-aware `b00t-mcp` proxy (11 sub-tasks incl. the 02 split)

| id | title / goal | key files | contract (essentials) | dep | verify | sz |
|---|---|---|---|---|---|---|
| **SP3-01** | `b00t-mcp/src/identity.rs` — JWT → `CallerIdentity`, JWKS fetch+cache, pinned-JWKS test path. | `b00t-mcp/src/identity.rs`(new), `lib.rs` | `CallerIdentity{tenant,agent,r0le,scopes,budget_ref}`; `JwksVerifier::{from_env(), pinned(jwks_json), async verify(jwt)->Result<CallerIdentity>}`. Reuse `jsonwebtoken` (`DecodingKey::from_rsa_components`, `Validation::new(RS256).set_issuer(&["https://b00t.promptexecution.com"])`). Claims = depend on `b00t-c0re-identity::AgentClaims`. | SP1-08 | `cargo test -p b00t-mcp identity::` — `MockTokenSource` mints → `JwksVerifier::pinned(mock.jwks_json())` → `CallerIdentity{tenant:"promptexecution",…}`; expired/tampered → `Err` | M |
| **SP3-02a** | HTTP: `tower` layer extracts + verifies `Authorization: Bearer`, injects into request extensions. | `b00t-mcp/src/main.rs`, `mcp_server_rusty.rs` | `/mcp` w/o valid bearer + `B00T_MCP_REQUIRE_AUTH=1` → `401`; unset → anon `CallerIdentity{r0le:"anon"}`. `CorsLayer` gains `authorization`. Design per **F3**. | SP3-01, F3 | server test on ephemeral port w/ `JwksVerifier::pinned`: `curl -H "Authorization: Bearer <mockjwt>" …/mcp` initialize → 200; bad + `REQUIRE_AUTH=1` → 401 | M |
| **SP3-02b** | stdio: read `B00T_AGENT_JWT` at startup; `initialize.params.meta.b00t_jwt` overrides. Shared state accessor. | `b00t-mcp/src/{main.rs,mcp_server_rusty.rs,lib.rs}` | `B00tMcpServerRusty` gains `caller: Arc<RwLock<Option<CallerIdentity>>>` + `verifier`; `caller()->Option<CallerIdentity>`. | SP3-01, F3 | spawn `b00t-mcp --stdio` w/ `B00T_AGENT_JWT=<mockjwt>` → `caller()` is `Some` | M |
| **SP3-03** | r0le → `tool_allowlist` resolver (trait + fixture + datum impl). | `b00t-mcp/src/r0le_resolver.rs`(new), `lib.rs` | `trait R0leResolver{ fn resolve(&self,tenant,r0le)->Result<ResolvedR0le> }`; `ResolvedR0le{tool_allowlist,skills,budget_ceiling,model_tier}`. `FixtureR0leResolver(map)` (tests / pre-SP2). `DatumR0leResolver` (SP2-06 scan + SP2-04 verify — **rejects unsigned/bad-sig**). | `FixtureR0leResolver` now; real impl after SP2-03/04/06 | `cargo test … r0le_resolver::` — fixture returns allowlist; datum impl vs signed `worker.agentprofile.toml`; tampered → `Err` | M |
| **SP3-04** | `list_tools` filtered by allowlist; revive `acl.rs` as glob matcher. | `mcp_server_rusty.rs::list_tools`, `b00t-mcp/src/acl.rs`, `Cargo.toml` (`glob`) | `AllowlistFilter::new(&[String])`, `.allows(tool_name)->bool` (`*` = all). `list_tools` w/ `caller.r0le != "anon"` → filter `registry.get_tools()`; `anon` → slim set unchanged. | SP3-02, SP3-03 | `cargo test … list_tools_filtered` — fixture `["b00t_status","b00t_learn","soul_*"]` → exactly those; `*` → full; `anon` → slim | M |
| **SP3-05** | Call-time unlock gate (`unlocks` ↔ learned-per-session). | `mcp_server_rusty.rs::call_tool` (+ `learned: Arc<RwLock<HashSet<String>>>`), `b00t-mcp/src/unlock_gate.rs`(new) | `UnlockGate{tool_to_skill}` from `ResolvedR0le.skills` + `collect_role_unlocks`; `required_skill(tool)`, `is_satisfied(tool,&learned)`. `call_tool`: unsatisfied → MCP error `{code:-32003,message:"tool '<t>' locked: run b00t_learn('<skill>') first"}`. Successful `b00t_learn`/`b00t_discover` → `learned.insert(skill)`. anon → gate off. | SP3-02, SP3-03, SP2-03 (`collect_role_unlocks` pub) | `cargo test … unlock_gate::` — gated tool → -32003, then succeeds after `b00t_learn` same session | M |
| **SP3-06** | Rust ledgrrr client trait + mock + `call_tool` budget pre-check (twin of SP1-03, per F2). | `b00t-mcp/src/ledgrrr_client.rs`(new) or shared `b00t-c0re-ledgrrr/`; `mcp_server_rusty.rs::call_tool` | `trait SpendAuthorizer{ async authorize_spend(tenant,agent,cost,ref)->Result<AuthorizeResp>; async record_usage(…)->Result<()> }`; `AuthorizeResp{ok,budget_remaining,reason}`; `HttpSpendAuthorizer{base_url}`, `MockSpendAuthorizer{always_ok,budget}`. `call_tool`: cost (flat 1 for now) → `authorize_spend`; `ok:false` → `{code:-32004}` + **do not execute**; success → run then `record_usage`. | SP3-02, F2 | `cargo test … ledgrrr_client::` — `always_ok:false` → -32004, tool not executed (spy); `true` → executes + 1 `record_usage` | M |
| **SP3-07** | `b00t_r0le_request_escalation` tool → judge → expand live allowlist + fire `tools/list_changed`. | `b00t-mcp/src/escalation.rs`(new), `mcp_tools.rs` (register + reuse `add_post_hook` notify), `mcp_server_rusty.rs` (`granted_extra: Arc<RwLock<Vec<String>>>` merged into filter) | tool params `{tools:string[],justification:string}` → `{granted[],denied:[{tool,reason}]}`. Policy = `capability_forge::judge::EscalationJudge` (reuse); default `FakeJudge::always_deny` prod, `always_grant` tests; `OpenAiJudge` opt-in. On grant: push `granted_extra`, call the **same `notify_fn` Arc** `stack_load` uses → `tools/list_changed`. Session-scoped, never persisted, never widens the JWT (per F5). | SP3-04, F5 | `cargo test … escalation::` — `always_grant` → granted + `notify_fn` once + next `list_tools` includes it; `always_deny` → denied, unchanged | M |
| **SP3-08** | Provider-agnostic remote routing (`RemoteMcpProxy`) — `<svc>__<tool>` → `https://<svc>.<base-domain>/mcp`, forward the caller JWT. | `b00t-c0re-lib/src/remote_mcp_proxy.rs`(new, sibling to `mcp_proxy.rs`), `mcp_server_rusty.rs::call_tool` | `RemoteMcpProxy{base_domain,client}` (default `b00t.promptexecution.com`); `route(tool)->Option<(svc,bare_tool,Url)>`; `async call(tool,params,bearer)->Result<Value>` (MCP `tools/call` over streamable-http). `B00T_MCP_ROUTING_DOMAIN` / `…_BASE_URL` override for tests. Non-namespaced → `route` None → local registry (unchanged). **No hosting provider hard-coded.** | SP3-02 | `cargo test … remote_mcp_proxy::` — `route("gh__list_repos")` → host ends `gh.b00t.promptexecution.com/mcp`; `call` vs local `axum` stub returns result + stub asserts `Authorization` forwarded | M |
| **SP3-09** | Integration wiring in `B00tMcpServerRusty::new_*` + `main.rs`; safe dev defaults (anon + mocks). | `mcp_server_rusty.rs`, `main.rs`, `README.md` | env: `B00T_MCP_REQUIRE_AUTH`, `B00T_IDENTITY_URL`/`_JWKS`, `B00T_LEDGRRR_URL`/`_MODE`, `B00T_MCP_ROUTING_DOMAIN`, `B00T_MCP_JUDGE` (`deny`\|`grant`\|`openai`). **All unset → fully local anon mode, behaviour identical to today.** | SP3-01…08 | `cargo test -p b00t-mcp` full suite green; `b00t-mcp --stdio` no env → slim registry unchanged (regression) | M |
| **SP3-10** | End-to-end: real SP1 worker JWT → `b00t-mcp` → filtering + gating. | `b00t-mcp/tests/e2e_identity.rs`(new, `#[ignore]` unless `B00T_E2E=1`) | start `wrangler dev` + seed `promptexecution` + grant agent `worker` → `POST /tokens` → feed JWT to `b00t-mcp --http` w/ `B00T_IDENTITY_URL` at the worker → `tools/list` shows only worker tools → gated `call_tool` → -32003 → `b00t_learn` → retry ok. | SP1-10, SP3-09, SP2-05 | `B00T_E2E=1 cargo test -p b00t-mcp --test e2e_identity` | M |

---

## Build + integrate order (DAG)

```
Wave 0  F1  F2  F4  F6                      (decisions; F3/F5 before Wave E)
Wave A  SP1-01  SP1-05  SP2-01  SP2-06
Wave B  SP1-02  SP1-03  SP2-02
Wave C  SP1-04  SP1-08  SP2-03(+load_role_datum/collect_role_unlocks pub)  SP1-06  SP2-04
Wave D  SP1-07  SP1-09  SP3-01  SP2-05  SP2-07
Wave E  SP1-10  SP3-02a  SP3-02b  SP3-03(Fixture now / Datum after C)
Wave F  SP3-04  SP3-06  SP3-08
Wave G  SP3-05  SP3-07
Wave H  SP3-09  →  SP3-10
```
Critical path: `SP1-01 → SP1-02 → SP1-08 → SP3-01 → SP3-02 → SP3-04 → SP3-07 → SP3-09 → SP3-10`.

## Integration checkpoints (test the join, not just the units)

1. **CP-A claims parity** (SP1-02 + SP1-08): a golden `AgentClaims` JSON fixture round-trips TS signer ↔ Rust `verify_jwt`.
2. **CP-B issuance flow** (SP1-04 + SP1-05 + SP1-03): member+grant+mock-budget → JWT w/ correct `scopes`/`r0le`; 404→403→402 order asserted.
3. **CP-C token → proxy** (SP1-08 mock + SP3-01 + SP3-02): mock JWT accepted; SP3-10 repeats with the real worker JWKS.
4. **CP-D r0le datum → proxy** (SP2-03/04/06 + SP3-03 `DatumR0leResolver`): signed `worker.agentprofile.toml` yields the same `tool_allowlist` the proxy filters on; tamper → both `b00t r0le verify` and the proxy reject.
5. **CP-E unlocks ↔ gate** (SP2-03 `collect_role_unlocks` + SP3-05): the gate's `tool→skill` map matches `b00t blessing --manifest`.
6. **CP-F ledgrrr contract** (SP1-03 TS + SP3-06 Rust): one shared JSON Schema, two mocks, a contract test each on the same example payloads.
7. **CP-G escalation ↔ notify** (SP3-07 + existing `stack_load` plumbing): `tools/list_changed` fires once per grant; `list_tools` reflects the new set.

## Top risks

- **SP1 is not greenfield** — evolve `ledgrrr-tenant-registry` (HMAC→RS256, +JWKS/ledgrrr/r0le/self-serve), don't rebuild. Worker name/route/rehome = **F1** (don't let a ch0nky agent guess).
- **Shared prod D1 `b00t-agents` + `prevent_destroy`** — every migration additive-only, `--local` first; check whether `migrations/0001` is already applied to prod.
- **900s token vs multi-hour session** — needs a refresh/401 story (**F3/F5**), not in any sub-task yet.
- **`rmcp` has no per-request auth** — `client_info` is `initialize`-time only; HTTP needs a `tower` layer + extensions, stdio is one-identity-per-process. **SP3-02 is the riskiest ch0nky task** — split as 02a/02b; may still need a Sonnet pass.
- **Datum signing must sign a canonical JSON projection**, never raw TOML bytes (the `.tomllmd`>`.tomllm`>`.toml` rank re-serializes files) — **F4**.
- **Datum key collisions across tenants** — `scan_datums_recursive` keys by filename only; **F6** must define overlay precedence *and* an isolation mode or one tenant's role leaks into another.
- **`capability-forge` is NATS-JWT + `ScopeStore`** — only `EscalationJudge`/`FakeJudge` (+ `SkillTier` conceptually) are reusable; `jwt_mint.rs` is a decoy.
- **`load_role_datum` + the blessing walk are private** — SP2-03/SP3-05 need `pub` + an extracted `collect_role_unlocks`; small refactor, `blessing.rs` test fallout risk.
- **`GenericMcpProxy` is subprocess-shaped** — SP3-08 is a *new* HTTP-remote type, not a bolt-on.
- **CORS** — `main.rs` `CorsLayer` must add `authorization` or browser MCP clients break.
- **Wildcard `*.b00t.promptexecution.com` has no real backends in SP1–3** — SP3-08 tests against a local stub via base-URL override; assume nothing exists.

## Verification (end-to-end, once SP1+SP2+SP3 land)

1. `cd workers/ledgrrr-tenant-registry && pnpm test && pnpm exec wrangler deploy --dry-run` — SP1 suite green, deployable.
2. `cargo test -p b00t-c0re-identity && cargo test -p b00t-cli identity:: r0le:: datum_types:: datum_agent_profile:: datum_utils::tenant_overlay && cargo test -p b00t-mcp` — SP2/SP3 unit suites green.
3. `b00t-mcp --stdio` with no env → slim registry, byte-identical behaviour to today (regression gate).
4. `B00T_E2E=1 cargo test -p b00t-mcp --test e2e_identity` — real worker JWT drives filtered `tools/list` + the -32003 unlock gate + post-`b00t_learn` success.
5. Manual: `wrangler dev` → seed `promptexecution` → `b00t identity token --tenant promptexecution --node <root> --r0le worker` → `B00T_IDENTITY_URL=… B00T_MCP_REQUIRE_AUTH=1 b00t-mcp --http` → an MCP client sees only `worker` tools; `b00t_r0le_request_escalation` with `B00T_MCP_JUDGE=grant` adds a tool + a `tools/list_changed`.

## Deferred / open

- **F1–F6 are the first work** (they resolve worker rehoming, the ledgrrr HTTP
  contract, rmcp identity threading, the datum-signing scheme, token lifetime,
  and tenant-namespaced resolution).
- ledgrrr's side of the HTTP contract (ledgrrr #175) — b00t ships against the
  mock until ledgrrr implements `/authorize-spend` + `/usage`.
- SPIRE HA (task #191) is a hard prerequisite for the SPIFFE half of D2 (SP4).
- SP4 (serverless OCI MCP hosting), SP5 (SysML/Oxigraph substrate), SP6
  (isolation + billing rollup + `b00t-website` migration + `tailscale.tf` split)
  — roadmap, each its own spec→plan cycle.
- b00t task **#195** stays tabled.
- Sub-task dispatch: `just compile-agent <role> <n> /tmp/agent.md && claude
  --agent`, or `mcp__b00t-mcp__b00t_agent_*`, or fix `b00t agent invoke` (task
  #158) — pick during F-wave.

---

# F-wave decisions (resolved 2026-09-10, session_01Cj7a4Bkh8pRQuzcErD1j2E)

Investigation corrected two plan assumptions: **`workers/ledgrrr-tenant-registry/`
is in the public `_b00t_` repo** (`_b00t_/workers/`, tracked — not
`infrastructure`), and **three workers already bind D1 `b00t-agents`**
(`ledgrrr-tenant-registry`, `b00t-mcp-vault`, the website's `b00t_api`). Baseline
worker: HMAC 2-part `payload.sig` token (`TOKEN_SIGNING_KEY`, 1h TTL), routes
`POST /tenants` · `GET /tenants/:id` · `POST /tokens` · `POST /verify` **all**
gated by `Bearer == REGISTRY_ADMIN_KEY`; `issueToken` = `lookupTenant` →
`TENANT_DO.idFromString(rootDoId)` → `hasMembershipPath` → `nodeGrantsShards` →
sign (no budget call); `verifyToken` = sig + `expiresAt` only (no DO round-trip).
`TenantNode` DO methods: `createNode`, `addMember(agentId,nodeId,role)`,
`hasMembershipPath`, `nodeGrantsShards`, `deleteNode`, `cakeRollup`; tables
`nodes`, `members`, `_placeholder_leaf_balances`. `workers/cf-workers.just` has
`deploy <worker>` and `migrate-remote <worker> <db>` (`wrangler d1 migrations
apply --remote`).

## F1 — identity-plane hosting architecture — DECIDED

- **Own worker, renamed.** `_b00t_/workers/ledgrrr-tenant-registry/` →
  **`_b00t_/workers/b00t-identity/`** (`wrangler.jsonc` `name: "b00t-identity"`).
  Per D1 the service is b00t's; ledgrrr is an outbound HTTP call, not a name.
  **Do NOT fold into `b00t_api`** — that couples the public harness's identity
  plane to the private `b00t-website` submodule.
- **Route split on `b00t.promptexecution.com`** (path-carved, standard OIDC):
  `b00t-identity` owns `b00t.promptexecution.com/.well-known/*` (JWKS +
  `openid-configuration`) **and** `b00t.promptexecution.com/identity/*`
  (`/identity/tenants/*`, `/identity/tokens`, `/identity/verify`,
  `/identity/register`). `iss` claim = `https://b00t.promptexecution.com`;
  `jwks_uri` = `…/.well-known/jwks.json`. Unchanged: `/api/*` → `b00t_api`;
  `/dashboard/*` → Pages `b00t-dashboard`; `*.b00t.promptexecution.com/*` →
  `b00t-mcp-cloudflare`; `/` → marketing homepage.
- **TF reconciliation** — mirror `terraform/b00t/website-worker.tf`: wrangler +
  `workers/cf-workers.just` deploy the code; new `terraform/b00t/identity-worker.tf`
  adopts the `b00t-identity` script (post first deploy), `ignore_changes` on
  `content`/`compatibility_date`, manages only the two `routes` + the
  `secret_text` bindings. D1 `b00t-agents` stays `prevent_destroy` + import-only
  in TF; `b00t-identity` merely *binds* it via `wrangler.jsonc`.
- **Secrets** → Azure Key Vault `config-global-b00t-identity-*` JSON, read via
  `data external` `az-keyvault-secret.sh`: `JWT_PRIVATE_KEY_PEM` (PKCS#8),
  `JWT_KID`, `REGISTRY_ADMIN_KEY`, `LEDGRRR_BASE_URL`, `LEDGRRR_MODE`.
- **Migration state** — `0001_create_tenants.sql` is almost certainly **not
  applied to prod D1** (the worker has never had a `routes` block ⇒ never
  deployed; the website D1 schema has no `tenants` table). Actions: add
  `IF NOT EXISTS` to `0001` (additive-only, harmless if applied); SP1-* dev
  against `wrangler d1 migrations apply b00t-agents --local`; **SP1-10 runs
  `wrangler d1 migrations list b00t-agents --remote` and confirms before the
  first `--remote` apply**. `0002_tenant_slug.sql` (`ALTER TABLE … ADD COLUMN`)
  is run-once — guard or accept.

## F2 — canonical b00t↔ledgrrr HTTP contract — DECIDED

Greenfield: ledgrrr today exposes `budget` only as an **MCP tool**
(`ledgerr-mcp` `handle_budget_tool` / `BudgetArgs`); no HTTP endpoint,
`http_gateway.rs` is backlog (ledgrrr #228). b00t defines the contract; ledgrrr
implements it later (ledgrrr #175). Until then both sides run the mock.

- **`POST {LEDGRRR_BASE_URL}/v1/authorize-spend`**
  req `{tenant, agent, cost:int (cake units), ref:string, idempotency_key:string}`
  → `200 {ok:true, budget_remaining:int}` | `200 {ok:false, budget_remaining:int, reason:string}`
  | `4xx {ok:false, reason}`. **Reserve-then-settle is out of scope** — `authorize-spend`
  is an advisory pre-check + soft decrement keyed by `idempotency_key` (same key ⇒
  same answer, no double-count). `cost` is in **cake** (ledgrrr's canonical unit;
  USD is an exchange rate — see `ledgerr-cloud` `usd_per_cake`).
- **`POST {LEDGRRR_BASE_URL}/v1/usage`**
  req `{tenant, agent, units:int, ref:string, idempotency_key:string, meta:object}`
  → `200 {ok:true}`. Fire-and-forget from the caller's view (best-effort, retried).
- **Error codes**: transport failure / non-200 ⇒ caller treats as `{ok:false,
  reason:"ledgrrr_unavailable"}` and **fails closed** in prod (`LEDGRRR_MODE=http`),
  **open** in dev (`LEDGRRR_MODE=mock`).
- **Rust side = a shared crate `b00t-c0re-ledgrrr`** (not inlined): `trait
  SpendAuthorizer { authorize_spend, record_usage }`, `HttpSpendAuthorizer`,
  `MockSpendAuthorizer`. Consumed by SP3-06; the TS twin (SP1-03) is
  `src/ledgrrr.ts` with the same shapes. One JSON Schema file
  `docs/schemas/ledgrrr-v1.json` is the contract of record (CP-F).

## F3 — per-caller identity threading through rmcp — DECIDED

`b00t-mcp` uses `axum 0.8` + `tower 0.5` + `tower-http 0.6`; `rmcp`'s
`RequestContext<RoleServer>` carries an `extensions: Extensions`. `call_tool`
already takes `context`; `list_tools` takes `_context` (unused).

- **HTTP `/mcp`**: a `tower` middleware layer extracts `Authorization: Bearer`,
  calls `JwksVerifier::verify`, and **inserts `CallerIdentity` into the request
  `Extensions`**. Change `list_tools(_context)` → `list_tools(context)` and read
  `context.extensions.get::<CallerIdentity>()` in both `list_tools` and
  `call_tool`. No bearer + `B00T_MCP_REQUIRE_AUTH=1` ⇒ `401`; unset ⇒ inject
  `CallerIdentity{r0le:"anon"}`.
- **stdio**: no per-request auth. Read `B00T_AGENT_JWT` at startup, verify once
  (sig + `iss` + `kid`), store on the server struct
  `caller: Arc<RwLock<Option<CallerIdentity>>>`. `initialize.params.meta.b00t_jwt`
  (read in `on_initialized`, same place `client_info` is captured) overrides.
- **Token lifetime (folds F5)**: stdio verification treats `exp` as **advisory**
  — logs a warning past expiry, does not 401 (the process boundary is the trust
  boundary; a stdio session is one agent for its lifetime). HTTP enforces `exp`
  strictly ⇒ `401`; the Rust `b00t-c0re-identity` client re-mints via
  `HttpTokenSource` on a 401 and the caller reconnects. Runtime escalations
  (SP3-07) live only in `granted_extra: Arc<RwLock<Vec<String>>>` on the session
  — **never persisted, never written back into a JWT**; they die with the
  connection.

## F4 — datum signing scheme — DECIDED

No existing canonical-JSON / JCS / datum-signing code in b00t — greenfield.

- **Sign a canonical JSON projection, never TOML bytes.**
  `AgentProfileSpec::canonical_bytes()` builds a `serde_json::Value` from the
  semantic fields **with `signature` excluded**, then canonicalizes: object keys
  sorted (recursively), and the list fields `tool_allowlist`, `skills`,
  `permissions` **sorted** (their order is not semantic);
  `soul_shard_grants` sorted by `(kind, id)`. `serde_json::to_vec` of that.
  This survives the `.tomllmd`>`.tomllm`>`.toml` rank-rule re-serialization.
- **One RS256 key, both purposes.** The `b00t-identity` worker's signing key
  signs JWTs **and** datum canonical bytes; **one `kid`**; verifiers fetch the
  same `…/.well-known/jwks.json`. (Rust side signs with the `rsa` crate's
  `pkcs1v15::SigningKey<Sha256>` — `jsonwebtoken` can't sign raw bytes.)
  `DatumSignature{ kid, alg:"RS256", sig_b64, signed_fields_sha256 }`.
- **Key source for CLI signing** (`b00t r0le build --sign`):
  `B00T_DATUM_SIGNING_KEY_PEM` env, else OS keyring `b00t/datum-signing-key`.
  In practice the operator holds the PKCS#8 private key that was also uploaded to
  the worker as `JWT_PRIVATE_KEY_PEM`.
- **Rotation**: new `kid`, both public keys served in the JWKS during the overlap
  window; re-sign profiles lazily. `verify` accepts any `kid` present in JWKS.

## F5 — token lifetime vs long sessions — DECIDED (merged into F3)

See F3 "Token lifetime": HTTP strict `exp` + client re-mint on 401; stdio
`exp`-advisory; escalations session-scoped and non-persistent. No separate
sub-task — SP3-02a/b and SP3-07 carry it.

## F6 — tenant-namespaced datum resolution — DECIDED

`datum_utils.rs`: `get_all_datums(b00t_path)`, `get_all_datums_with_paths(...)`,
`get_all_datums_with_diagnostics(...)`, private `scan_datums_recursive(...)`;
keys = filename-minus-ext, rank `.tomllmd`>`.tomllm`>`.toml`, later-scan-wins on
tie, good-parse beats degraded.

- **New `pub fn get_all_datums_for_tenant(b00t_path, tenant: Option<&str>,
  depth) -> Result<HashMap<String,(BootDatum,String)>>`.** Scans base
  `b00t_path` (e.g. `~/.b00t/_b00t_`) with the existing rank rules → base map.
  If `tenant = Some(t)`: also scans `<b00t_path>/../tenants/<t>/_b00t_`
  (`~/.b00t/tenants/<t>/_b00t_`) with the same rank rules → overlay map; then
  **for every overlay key, replace the base entry** (overlay always wins — it is
  an explicit tenant override, *not* a rank comparison).
- **Keys stay bare** (`worker.role`) so `b00t r0le build` is tenant-agnostic —
  the resolver, not the key, carries the tenant.
- **Cross-tenant isolation is structural**: each call passes exactly one
  `tenant`; the two scan roots are disjoint directories; there is no shared
  mutable map and two tenants never co-occur in one resolution. A tenant's tree
  cannot reference another tenant's — `depends_on` resolves within the merged
  (base+one-overlay) map only.
- **`--strict-tenant`** (and a `TenantScope::Strict` variant) ⇒ overlay map
  only; base `_b00t_` invisible. Default is overlay-over-base.
- Existing `get_all_datums*` callers unchanged; `get_all_datums_for_tenant(_,
  None, _)` ≡ `get_all_datums_with_paths`.

## Net effect on the sub-task DAG

- **SP1** worker dir/name is `_b00t_/workers/b00t-identity/`; routes are the two
  path-carved patterns above; `iss` = `https://b00t.promptexecution.com`.
- **F2** adds a small crate `b00t-c0re-ledgrrr` to Wave A (shared by SP1-03's TS
  twin and SP3-06) + `docs/schemas/ledgrrr-v1.json`.
- **SP3-02** reads `CallerIdentity` from `RequestContext::extensions` (HTTP) /
  server-struct `RwLock` (stdio); `list_tools` signature changes `_context` →
  `context`.
- **SP2-04** signs `canonical_bytes()` (sorted JSON projection) with the `rsa`
  crate; one `kid` shared with SP1 JWT signing.
- **SP2-06** = `get_all_datums_for_tenant` + `~/.b00t/tenants/<t>/_b00t_`
  overlay-wins semantics + `--strict-tenant`.
- **F5** is not a standalone sub-task (folded into SP3-02a/b + SP3-07).
