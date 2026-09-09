# Build-plane identity — consume the existing plane, don't rebuild it

**Date:** 2026-09-09 · plan gleaming-jingling-nygaard.
**Correction:** an earlier draft of PR #1280 authored a parallel keyless
identity plane (a new `external-idp` WIF pool, a `modules/identity-aws`,
`deploy/k0s/spire/` to helm-install SPIRE, an invented `b00t.promptexecution.com`
trust domain). That was **wrong** — the plane already exists and is live. Those
files were removed.

## What already exists (in `PromptExecution/infrastructure`, ADMIN)

| Layer | State |
|---|---|
| **SPIRE** | Running. Trust domain `spiffe://promptexecution.com`. OIDC discovery live at `https://spire-oidc.promptexecution.com` (Cloudflare-proxied HTTPS). Agents `sm3lly`, `fung1`. |
| **GCP WIF** | `terraform/google/google-oidc-spire.tf` — `spire-pool` / `spire-provider`, `attribute_condition` **generated from `fleet/spire-agents.json`**, `principal://…/subject/spiffe://promptexecution.com/agent/<x>` bindings to the `spire-agent` SA (no roles attached yet). |
| **AWS** | `terraform/aws/aws-oidc-spire.tf` (+ `aws-oidc-k0s.tf`) — IAM OIDC providers for `spire-oidc.promptexecution.com` and `k0s-oidc.promptexecution.com`; `spire-agent-<x>` roles. |
| **Entra** | `msft-corp/entra/agent-identity-federation.bicep` — same `fleet/spire-agents.json`. |
| **k0s** | `k0s/spire/` manifests + `spire-agent/` install; the k0s-agent-identity control plane is planned in `docs/superpowers/plans/2026-08-22-k0s-agent-identity-*.md`. Open blocker: `PromptExecution/infrastructure#100` (AWS→GCP impersonation). |

`fleet/spire-agents.json` is the **single source of truth** — one edit
propagates the allow-list to GCP + AWS + Entra.

## What the build plane needs (the real delta)

### In `PromptExecution/infrastructure` (proposed — see that repo's PR)

1. **`fleet/spire-agents.json`** — add the build-plane workload identity.
   Decision needed on shape: the existing entries are bare *hosts*
   (`agent/sm3lly`); the CI workload is a k8s Pod on `vultr1`, so either
   - add `vultr1` as a host agent and let the pod use the node identity, or
   - a workload-scoped ID (`spiffe://promptexecution.com/build-plane/ci`),
     which needs `spire-provider.attribute_condition` widened beyond the
     `agent/${x}` template.
2. **`terraform/google/`** — attach the *first real roles* to the bound SA
   (or a dedicated `b00t-buildplane` SA): `roles/storage.objectViewer` on
   `b00t-buildcache-promptexecution` + `roles/artifactregistry.reader` on the
   `b00t` AR repo. The file today is trust scaffolding only.
3. **SPIRE server registration entry** for the build-plane workload (k8s
   workload attestor: `ns:b00t-ci`, `sa:b00t-ci`, image digest).

### In `_b00t_` (PR #1280, what remains)

- **SOCI snapshotter on k0s** — `nats/pyinfra/deploy_k0s_soci.py` + files.
  Identity-independent; lazy image pull for build-plane pods.
- **dstack `kubernetes` backend** — `deploy_cp_node.py` + the
  `dstack-server-config.yml.j2` block (rendered with `--data k0s_kubeconfig=`)
  + `fetch-k0s-kubeconfig.sh`.
- **`dev-env/k0s-ci-test.task.yaml`** — the compile-free run stage as a k8s
  Pod. Expects a SPIRE JWT-SVID + an `external_account` ADC config at
  `/var/run/gcp/credential-configuration.json` (delivered by the SPIRE CSI
  driver / `spiffe-helper` per infra).
- **`dev-env/gcs-obj.sh`** — `external_account` (WIF/SVID) credential path,
  issuer-agnostic; baked into the `b00t-build` image. Offline test
  `dev-env/tests/gcs-obj-jwt-test.sh`.
- **`.github/workflows/b00t-build-image.yml`** — the `soci-index` job.

## Rule going forward

Anything touching workload identity / cloud federation / SPIRE for
PromptExecution goes through **`PromptExecution/infrastructure`** and
`fleet/spire-agents.json` — never a parallel construction in `_b00t_` or
`b00t-tf`. `_b00t_` only *consumes* SVIDs.
