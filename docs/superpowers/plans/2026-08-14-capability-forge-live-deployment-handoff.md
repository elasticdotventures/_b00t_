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
