# capability-forge live deployment — handoff

Date: 2026-08-15
Status: phase-1 implementation done, reviewed, merged-ready. Live deployment to the Vultr
node not started — blocked, then unblocked on auth, but stopped here on request before
proceeding into production infrastructure changes.

## What's done

- **Design**: `docs/superpowers/specs/2026-08-13-capability-forge-design.md` (this repo,
  `infrastructure`).
- **Plan + implementation**: `docs/superpowers/plans/2026-08-13-capability-forge-implementation.md`
  (in the `~/.b00t` worktree at `/home/brianh/.b00t/.worktrees/task-capability-forge`, branch
  `task/capability-forge`) — 11 tasks, each with independent implementer + reviewer + fix
  loops, plus a whole-branch review with its own fix wave. Full crate suite: 44/44 tests
  passing, including a real end-to-end test against an ephemeral local `nats-server` that
  proves server-side NATS scope enforcement and revocation-specific reconnect rejection.
- **PR**: https://github.com/elasticdotventures/_b00t_/pull/1091 (`task/capability-forge` →
  `main`), pushed and open.
- Known, disclosed follow-up gaps are recorded in the plan doc's own "Status" section (revoke
  has no production wiring yet; several parked hardening items) — read that before treating
  revocation as a finished capability.

## What's blocked: live deployment to the Vultr node

Deploying capability-forge against the *live* NATS server (the one from PR #93's pingap/NATS
foundation) requires the real operator's NATS signing key — the live node runs with
`auth_required: true` in full operator/JWT mode (confirmed in
`terraform/b00t/cloud-init/b00t-node.yaml.tpl`'s existing `b00t-daprd.service` comment, which
documents the exact same gap for `daprd`).

- Terraform's `terraform/b00t/nsc-data-sources.tf` expects that key at a **local filesystem
  path**: `${path.root}/../../../b00t-backend-nats/nsc-setup` (sibling to this `infrastructure`
  repo checkout). **That directory does not exist in this session's environment.**
- Checked and ruled out during this session:
  - AWS Secrets Manager, `global` zone (`./export-aws-dotenv.sh --tf --zone global`): has
    `VULTR_API_KEY` and unrelated Azure app credentials, no NATS operator key.
  - AWS Secrets Manager, `live` zone: empty.
  - GCP Secret Manager (`./export-gcloud-dotenv.sh --tf --zone global`, the store the backend
    config's own comment says is actually preferred — "$0.06/mo vs AWS $0.40"): **was blocked
    on expired `gcloud` auth for the whole session; auth was refreshed just before this
    handoff was written** (`gcloud auth list` now shows `brianh@elastic.ventures` active) —
    **this is the next thing to check, not yet done.**
  - Local `nsc` CLI credential store: `nsc` isn't installed on this machine, no store
    directory found.

## Next steps (in order) for whoever picks this up

1. Now that `gcloud` auth is refreshed, run from `infrastructure/` (repo root, not this
   worktree — the script resolves paths via `module.thisRepo`):
   ```
   ./export-gcloud-dotenv.sh --tf --zone global
   ```
   and check whether a NATS operator key (something like `NATS_OPERATOR_JWT`/`_SEED`, or a
   `b00t-operator`-named entry — exact naming not yet confirmed) is present. This was the
   next unblocking step when this session stopped.
2. If the key is there: it needs to be wired into `terraform/b00t/nsc-data-sources.tf` (or a
   new data source next to it) so Terraform can inject it into the Vultr node's cloud-init —
   currently that file only reads from the missing local path, with placeholder fallbacks.
3. If the key genuinely isn't in either secret store: it may only exist on whatever machine
   originally ran `nsc` to set up `b00t-backend-nats` — track that down, or decide whether to
   mint a **new**, capability-forge-specific NATS account under the existing operator instead
   of reusing the SYS-account setup (the design doc left the exact production secrets flow
   underspecified beyond "AWS Secrets Manager via `module.globalEnvy`" — worth revisiting
   given the actual store turned out to be GCP-preferred, not AWS, and per-account rather than
   necessarily reusing SYS).
4. Once the key is available to Terraform: capability-forge needs actual deployment wiring
   that doesn't exist yet — no systemd unit, no binary distribution path in the cloud-init
   template (unlike `b00t-nats`/`b00t-pingap`/`b00t-daprd`/`b00t-maintenance`, all already
   present), and no plan for how `CAPFORGE_ACCOUNT_SEED`/`OPENAI_API_KEY` reach the node
   securely. This is genuinely new scope, not a small addition — budget it as such rather than
   bolting it onto the existing cloud-init template ad hoc.
5. This is also the point to close the "revocation has no production wiring" gap noted in the
   plan doc's Status section, since production revocation only makes sense once
   capability-forge is actually running against the live operator/account.

## Why this stopped here

The user asked for a handoff and to stop pursuing the live-test goal for this session, right
after confirming `gcloud auth` succeeded — deliberately not proceeding straight into live
infrastructure changes on a freshly-authenticated session without a fresh look. This doc is
that stopping point.

## Update 2026-08-15: bootstrap pattern built, operator seed still missing

Confirmed via `gcloud auth list` that GCloud auth now works, and re-ran
`./export-gcloud-dotenv.sh --tf --zone global` from the `infrastructure` repo root — the full
secret dump has no NATS/operator-related entry (only Azure/Cloudflare/Grafana/HuggingFace/etc.
credentials unrelated to this work). Combined with the earlier AWS `global`/`live` zone checks,
**the operator signing seed is not in either cloud secret store under the `global` zone.** It
most likely only ever existed in the local `nsc-setup` checkout on whichever machine originally
ran `nsc` to create it — not reachable from this session.

Also discovered `pods/nats/nats-pod-configured.yaml` (committed, not a secret — JWTs are
public/verifiable by design) already has a **real operator baked in**: `b00t-operator`,
identity pubkey `OCTX6B2BDFWGOJVN3PTBBDR6Y3WZIZJMKYAVGFV23OKK3J4EYXKPJ74T`, with one designated
signing key `ODMSVCODGVEUVCCQUV36MPVDTQJ36Z4EA2BMW6X6KQCRG2FGF6OX2DJL`. Only the `SYS` account
is currently preloaded — no account exists yet for capability-forge/agents to live under.

**Built** (commit `a6aab28a`): `capability-forge/src/bin/mint_account` — a one-time bootstrap
tool. Given `NATS_OPERATOR_SIGNING_SEED` (the seed for that designated signing key, not the
operator's root identity key), it mints a new `CAPFORGE` account and prints:
1. The `resolver_preload` entry to append to `pods/nats/nats-pod-configured.yaml`.
2. `CAPFORGE_ACCOUNT_SEED`/`CAPFORGE_ACCOUNT_PUBKEY` — what capability-forge's running service
   actually needs day to day. **The operator key is never needed again after this one run.**

Tested against a locally-generated fake operator keypair (2 passing tests: a real mint proving
the JWT is well-formed and signed correctly, and a rejection test for an invalid seed) — the
tool itself is proven correct. It has not been run against the real operator key, which still
isn't available anywhere reachable.

### Still needed before a real live deployment

1. **The actual operator signing seed**, from wherever the original `nsc-setup` run's output
   lives (not found on this machine or in either checked secret store) — or a decision to
   generate a *new* operator+account structure from scratch if the original is truly
   unrecoverable (would orphan the existing `SYS` account/live NATS trust chain, a bigger
   decision, not one to make unilaterally).
2. Once `mint_account` runs for real: update `pods/nats/nats-pod-configured.yaml` with the new
   resolver_preload entry, store `CAPFORGE_ACCOUNT_SEED`/`_PUBKEY` in the secret store (matching
   the `VULTR_API_KEY` pattern — `config/global/capforge-account-seed` etc. — wired into
   `terraform/b00t/nsc-data-sources.tf` or a new file alongside it), and push/reload the live
   `nats-server`.
3. **A second, smaller mint**: agents need *some* initial NATS credential just to reach the
   `capability.request.*` subject before they have any real scope — per the design doc, "a
   shared, narrowly-scoped requester NATS credential baked into the node's cloud-init" (publish
   to `capability.request.>` only, subscribe to its own inbox). That's a NATS *user* JWT under
   the new CAPFORGE account, not another account-level mint — capability-forge itself can
   already mint arbitrary-permission user JWTs once it holds the account signing key, so this
   could be a one-line addition to `mint_account` or a separate tiny tool.
4. **The "authenticate via b00t.promptexecution.com" question the user raised** is still open:
   for an agent with *zero* prior NATS credentials (not even the shared requester one — e.g. a
   brand-new external integration), is there an HTTP-fronted enrollment path through
   b00t.promptexecution.com/pingap, or is the shared requester credential itself distributed
   out-of-band (e.g. baked into the b00t-cli release binary, or handed out at agent
   provisioning time)? This is exactly the Phase 2 HTTP dashboard scope the original design
   doc deferred — worth an explicit decision before building it, not an assumption.
5. Cloud-init/systemd wiring for capability-forge itself on the Vultr node (binary
   distribution, `b00t-capability-forge.service` unit, env vars sourced via `module.globalEnvy`)
   — none of this exists yet, unlike `b00t-nats`/`b00t-pingap`/`b00t-daprd`/`b00t-maintenance`.

## Update 2026-08-15 (later): operator regenerated, validated, PRs open

Per explicit user decision, generated a brand-new NATS operator trust root from scratch
(old signing key confirmed unrecoverable — see above) via `capability-forge/src/bin/bootstrap_operator`:
new operator + designated signing key, new SYS account/user, new CAPFORGE account.

**Validated for real**, not just generated: ran an actual local `nats-server` v2.14.5 against
the exact committed `pods/nats/nats-pod-configured.yaml` content (extracted, not paraphrased)
— clean startup, `Trusted Operators: Operator: "b00t-operator"` logged. Then minted a real
user JWT under the new CAPFORGE account via `jwt_mint::mint_user_jwt` and connected/published
successfully. This is `capability-forge/examples/validate_local_bootstrap.rs`, kept in the
repo as a repeatable tool.

**Open PRs:**
- elasticdotventures/_b00t_#1091 — capability-forge phase-1 implementation (unchanged status).
- elasticdotventures/_b00t_ `task/capability-forge` also now has `capability-forge/src/bootstrap.rs`,
  `src/bin/bootstrap_operator`, and the validation example (commit `a8191249`).
- PromptExecution/infrastructure#95 — the regenerated `pods/nats/nats-pod-configured.yaml`,
  on a new branch (`capforge-nats-operator-genesis`), not the retirement branch this handoff
  doc originally lived on.

**Still not done, still needs explicit human action:**
1. Store the seeds (`NATS_OPERATOR_ROOT_SEED`, `NATS_OPERATOR_SIGNING_SEED`,
   `NATS_SYS_ACCOUNT_SEED`, `NATS_SYS_USER_SEED`, `CAPFORGE_ACCOUNT_SEED`) in the secret store
   — they exist only in this session's conversation transcript and the `bootstrap_operator`
   run's stdout right now, nowhere durable. **Treat this run's specific values as sensitive
   and get them into the secret store (or regenerate fresh) before relying on them.**
2. The actual live cutover: `tofu apply` (once the secrets above are wired into
   `terraform/b00t/nsc-data-sources.tf` or similar) + reloading the real Vultr node's
   `nats-server` — real production action with brief downtime, deliberately not done this
   session, needs its own explicit go-ahead at the moment it happens.
3. Item 3 from the "still needed" list above (the shared low-privilege "requester" NATS user
   for agents with zero prior credentials, and the open `b00t.promptexecution.com` bootstrap
   question) is unchanged — still open.
4. Cloud-init/systemd wiring for capability-forge itself on the node — still doesn't exist.
