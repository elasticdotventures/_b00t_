# b00t identity & trust plane — canonical map

**Status:** reference (the standing map), not a spec. Last reconciled against
live GCP + the vultr1 SPIRE server on **2026-10** (session
`session_01Cj7a4Bkh8pRQuzcErD1j2E`). Supersedes the scattered account of
identity across `docs/superpowers/specs/2026-09-09-build-plane-identity.md`,
`PromptExecution/infrastructure` docs, and `b00t-tf` comments — those remain
the *design rationale*; this is *what exists and who owns it*.

## 0. Ownership rule

Anything touching **workload identity / cloud federation / SPIRE** for
PromptExecution goes through **`PromptExecution/infrastructure`** and
`fleet/spire-agents.json` (+ `terraform/google/google-oidc-spire*.tf`).
`_b00t_` only *consumes* SVIDs. `b00t-tf` (in `_b00t_`) owns the **GCP
build-plane compute + its GitHub-Actions WIF + a couple of custom roles** —
never a SPIRE pool.

| Concern | Repo / path |
|---|---|
| SPIRE server, agents, CSI driver, registration | `infrastructure` `k0s/spire/`, `spire-agent/` |
| SPIRE→cloud WIF (GCP/AWS/Entra/Alicloud) | `infrastructure` `terraform/{google,aws,alicloud}/*-oidc-spire.tf`, `msft-corp/entra/` |
| build-plane workload SA (`b00t-buildplane-ci`) + its grants | `infrastructure` `terraform/google/google-oidc-spire-buildplane.tf` |
| build-plane compute (control node, Spot VMs, buckets, AR) | `_b00t_` `b00t-tf/modules/gcp-build-plane` |
| GitHub-Actions→GCP WIF for `_b00t_` | `_b00t_` `b00t-tf/modules/gcp-build-plane` (pool `github-actions`) |
| custom roles `b00tCpWaker`, `b00tDstackBackend` | `_b00t_` `b00t-tf/modules/gcp-build-plane` (kept mode-independent so infra can bind them) |
| tailnet waker pod manifest | `_b00t_` `deploy/k0s-waker/` |
| Cloudflare zone / `spire-oidc` DNS / edge TLS | `infrastructure` `terraform/b00t/{vultr_node,cloudflare,cloudflare-tls-hardening}.tf` |

## 1. SPIRE topology

| Property | Value |
|---|---|
| Trust domain | `spiffe://promptexecution.com` |
| Server | `spire-server` 1.15.3, k0s ns `spire` on **vultr1** (the only node), `replicas: 1`, `strategy: Recreate` |
| Root CA | **self-signed** — no `UpstreamAuthority` |
| DataStore | `sql` / **sqlite3** at `/var/lib/spire/server/datastore.sqlite3` (local disk) |
| KeyManager | **`disk`** — `keys.json` (local disk) |
| JWT signing key | **`rsa-2048`** (migrated from EC-P256 for Alicloud; one active type domain-wide) |
| `jwt_issuer` | `https://spire-oidc.promptexecution.com` (must be set — Azure `AADSTS90014` without it) |
| Node attestor | `join_token` only (manual, single-use, over SSH) |
| Workload attestors | `unix:uid` (bare hosts), `k8s` (vultr1 pods, via the CSI driver) |
| OIDC Discovery Provider | sidecar, `insecure_addr 0.0.0.0:8443`, `allow_insecure_scheme = true` — TLS terminated at Cloudflare |

**This is the single biggest risk in the whole plane** — one node, self-signed
CA, sqlite on local disk, `keys.json` on local disk. See
`docs/superpowers/specs/2026-09-10-spire-server-ha-datastore.md` (task **#191**).

### Registration entries (live, 2026-10)

| SPIFFE ID | Parent | Selectors | JWT TTL |
|---|---|---|---|
| `…/agent/sm3lly` | self / `join_token/*` | `unix:uid:1000` | default |
| `…/agent/fung1` | self / `join_token/*` | `unix:uid:1000` | default |
| `…/agent/vultr1-k8s` | `join_token/*` | — (k8s node agent) | default |
| `…/ns/b00t-ci/sa/b00t-ci` | `…/agent/vultr1-k8s` | `k8s:ns:b00t-ci` + `k8s:sa:b00t-ci` | **900s** |

The `ns:b00t-ci/sa:b00t-ci` entry has **no image-digest selector** — any pod in
ns `b00t-ci` running as SA `b00t-ci` gets this identity (today: the waker; soon:
the CI test pod). `fleet/spire-agents.json` still lists only
`["sm3lly","fung1"]`; the workload subject is added separately via
`google-oidc-spire-buildplane.tf`'s `spire_workload_subjects` local.

## 2. Service accounts (GCP project `promptexecution`, num `308167228204`)

| SA | Owner | Who can assume it | Permissions | Notes |
|---|---|---|---|---|
| **`b00t-buildplane-ci`** | infra `google-oidc-spire-buildplane.tf` | `principal://spire-pool/subject/spiffe://…/ns/b00t-ci/sa/b00t-ci` | `b00tCpWaker`¹ on `b00t-dstack-control` (cond); `storage.objectViewer` on `b00t-buildcache-promptexecution`; `artifactregistry.reader` on repo `b00t` | The tailnet waker **and** (future) CI test pod run as this. ¹ live still `compute.instanceAdmin.v1`; PR #229 → `b00tCpWaker` on next apply |
| **`b00t-ci`** | `b00t-tf` gcp-build-plane | `principalSet://github-actions/attribute.repository/elasticdotventures/_b00t_` | `storage.objectViewer` on buildcache; `artifactregistry.writer` on repo `b00t` | GitHub Actions `build` job. **Different SA** from `b00t-buildplane-ci` despite the similar name and the shared k8s SA name `b00t-ci`. |
| **`b00t-build-vm`** | `b00t-tf` | attached to dstack-provisioned Spot VMs (metadata SA); `dstack-server` has `actAs` | `storage.objectAdmin` on buildcache; `artifactregistry.reader` | Not federated — plain attached SA. `objectAdmin` is write+delete (tighten → `objectCreator`+`objectViewer`). |
| **`b00t-dstack-server`** | `b00t-tf` | attached to `b00t-dstack-control` (metadata, `creds: type: default`) | custom `b00tDstackBackend` (least-priv compute provision/teardown) | The dstack GCP backend identity. |
| **`b00t-cp-waker`** | `b00t-tf` | Cloud Run service (public mode only) | custom `b00tCpWaker` on `b00t-dstack-control` (cond) | **public `network_mode` only.** Tailnet mode uses `b00t-buildplane-ci`. PR #1287 gates this SA on `is_public`. |
| **`spire-agent`** | infra `main-gcloud.tf` | `spire-pool` (all agents) **and** `entra-hub-pool` (sm3lly/fung1 SP object IDs) | **none** (verified — zero project roles) | ⚠️ Bound by two pools. Keep it roleless — a role here is wieldable via the Entra-hub path too (b00t #199). |

## 3. Workload Identity pools (GCP, all `global`)

| Pool / provider | Issuer | Audience | `attribute_condition` (subjects) | Target SA | Owner |
|---|---|---|---|---|---|
| `spire-pool` / `spire-provider` | `https://spire-oidc.promptexecution.com` | *(GCP default: full provider RN)* | `agent/sm3lly`, `agent/fung1`, `ns/b00t-ci/sa/b00t-ci` | per-subject `principal://` → `spire-agent` (agents) / `b00t-buildplane-ci` (workload) | infra |
| `entra-hub-pool` / `entra-hub-provider` | `https://sts.windows.net/1fd87b50-…/` (v1) | `https://management.core.windows.net/` | sm3lly/fung1 Entra SP object IDs | `spire-agent` (roleless) | infra `feat/pgduck-postgres-replacement-clean` — **live in GCP, TF not on main** |
| `github-actions` / `github-oidc` | `https://token.actions.githubusercontent.com` | — | `repository == elasticdotventures/_b00t_` | `b00t-ci` | `b00t-tf` |
| `github-pool` / `github-provider` | `https://token.actions.githubusercontent.com` | — | `repository in [PromptExecution/infrastructure]` | `prefect` | infra |
| `aws-machine-pool` / `aws-provider` | *(AWS account `968589500754`)* | — | — | — | infra (AWS→GCP machine identity) |

## 4. Credential flows

- **Tailnet waker** (k0s pod `b00t-ci/b00t-cp-waker`, `hostNetwork`): SPIRE
  JWT-SVID (`k8s:ns:b00t-ci`+`sa:b00t-ci`) → `spire-pool` STS exchange →
  impersonate `b00t-buildplane-ci` → `compute.instances.start` on
  `b00t-dstack-control`. Reachable at `http://100.109.101.1:8088` over the
  tailnet; proxies to `100.92.193.0:3000` (dstack).
- **Keyless GCS** (`dev-env/gcs-obj.sh` `external_account`): same SVID →
  `spire-pool` → `b00t-buildplane-ci` → `storage.objectViewer` on the
  buildcache. Used by the waker today; the CI test pod once it carries the
  `csi.spiffe.io` volume (dstack 0.21.5 can't inject it — kubectl/Dagster path,
  b00t #193).
- **GitHub Actions `build` job**: GitHub OIDC → `github-actions` pool →
  impersonate `b00t-ci` → buildcache read + AR write.
- **dstack server**: attached `b00t-dstack-server` SA via metadata (no key, no
  federation) → `b00tDstackBackend` role → provisions Spot VMs with
  `b00t-build-vm` attached.
- **Azure Key Vault** (sm3lly/fung1): SPIRE JWT-SVID (`aud:
  api://AzureADTokenExchange`) → `az login --federated-token` → Entra v1.0
  token → `kv-pe-agent-secrets`. The one fully-productionised SPIRE→cloud path.

## 5. Entra

- **SPIRE → Entra (live):** `federatedIdentityCredential` on
  `b00t-agent-sm3lly`/`-fung1` app regs, trusting `spire-oidc…` with the
  SPIFFE ID as subject. This is the link the whole "Entra as hub" idea
  depends on; unchanged.
- **Entra as hub (design, scaffolded, NOT cut over):** AWS/GCP/Alicloud each
  gain a *second* WIF trust to Entra's own `sts.windows.net` issuer so SPIRE
  only ever has to satisfy Entra. `*-oidc-entra.tf` exist (GCP's is live in
  the project but its TF is on `feat/pgduck-postgres-replacement-clean`, not
  `main`), bound to the roleless `spire-agent` SA. No dated cutover plan.
  See `infrastructure/docs/entra-hub-federation-design.md`.

## 6. Cloudflare is a trust root

`spire-oidc.promptexecution.com` is a `proxied=true` Cloudflare record →
vultr1 reserved public IP :443 → pingap → the OIDC discovery provider.
**Whoever holds Cloudflare zone admin for `promptexecution.com` can repoint
`spire-oidc` and serve an arbitrary JWKS — i.e. forge any SPIFFE identity
fleet-wide.** Treat CF zone access as tier-0 (hardware-key 2FA, minimal admin
set, audit). Hardening plan (Authenticated Origin Pulls, `ssl=strict`,
firewall :443 to CF ranges, drop `allow_insecure_scheme`):
`docs/superpowers/specs/2026-09-10-spire-oidc-origin-hardening.md` (b00t #200;
edge-TLS-floor step already merged, infra `16ee324`).

Note: Alicloud's OIDC provider pins the **thumbprint of the Cloudflare edge
cert** (not SPIRE's signing key), so a CF cert rotation can break Alicloud
federation independently.

## 7. Cross-cloud (pointers)

- **AWS**: `aws-oidc-spire.tf` (IAM OIDC provider for `spire-oidc…`, aud
  `sts.amazonaws.com`, `spire-agent-*` roles) + `aws-oidc-k0s.tf` (trusts
  k0s's *own* issuer `k0s-oidc.promptexecution.com`, IRSA-style). AWS→GCP
  machine identity via `aws-machine-pool`. Blocker: `infrastructure#100`.
- **Alicloud**: `alicloud-oidc-spire.tf` + `alicloud-oidc-entra.tf`
  (proof-of-concept, RSA-JWKS requirement forced the domain-wide key
  migration).
- **Tailscale**: OIDC trust is **console-managed, no IaC**. Tags:
  `tag:b00t-control-plane`, `tag:vultr1`, `tag:ci-runner` (added this session
  for CI-runner tailnet join). ACL uses the `grants` model + a wildcard
  `{*→*}` grant still present.

## 8. Open risks → where they're tracked

| Risk | Tracker |
|---|---|
| SPIRE server SPOF / self-signed CA / sqlite+disk-key on one node | **#191** + `…spire-server-ha-datastore.md` |
| Cloudflare edge = fleet-wide forgery; unauthenticated CF→origin; `:443` public | **#200** + `…spire-oidc-origin-hardening.md` |
| Entra-hub pools live/scaffolded with no cutover plan; `spire-agent` dual-bound | **#199** (guard shipped); needs a dated Entra-hub decision |
| `b00t-buildplane-ci` still holds `compute.instanceAdmin.v1` (vs `b00tCpWaker`) | **#199** / infra PR #229 — needs `tofu apply` |
| `ns:b00t-ci/sa:b00t-ci` entry has no image-digest selector | this doc §1; tighten when the CI pod ships (#193) |
| `b00t-build-vm` has `storage.objectAdmin` (write+delete) on the buildcache | tighten → `objectCreator`+`objectViewer` |
| `fleet/spire-agents.json` doesn't express pod-scoped IDs (drift risk on regen) | infra `google-oidc-spire.tf` now `concat`s a `spire_workload_subjects` local — keep both in sync |
| SA naming collision: `b00t-ci` (GH WIF) vs `b00t-buildplane-ci` (SPIRE) vs k8s SA `b00t-ci` | this doc §2 — do not consolidate; they are three things |
