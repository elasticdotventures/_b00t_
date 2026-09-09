# SPIFFE/SPIRE as a portable cross-cloud identity plane — does it compose with WIF?

**Date:** 2026-09-09 · plan gleaming-jingling-nygaard.
**Questions (operator):** does bidirectional Entra↔GCP WIF "work for" a
SPIFFE/SPIRE portable identity plane? Is there a better/more robust k0s-native
option than jinja (kustomize, …), or is jinja the right fit with pyinfra?

## Part 1 — SPIFFE/SPIRE + WIF

### Short answer

**Yes, they compose — and SPIRE *is* the "portable identity plane" layer, with
WIF as its cloud on-ramp.** But they solve different problems and you do **not**
need all three (k0s-issuer WIF, Entra↔GCP WIF, SPIRE) at once. Pick by scale.

### How each piece relates

| Layer | Identity it issues | Trust anchor | Good for |
|---|---|---|---|
| **k8s SA issuer** (what PR #1280 uses) | projected SA JWT, `sub = system:serviceaccount:ns:name` | the cluster's signing key (public JWKS) | one cluster → one or more clouds |
| **Entra ↔ GCP WIF** | cloud tokens (Entra app token; GCP SA ID token) | each cloud's IdP | service-to-service *between* two clouds |
| **SPIFFE/SPIRE** | SVID — X.509-SVID (mTLS) or JWT-SVID (`spiffe://trust-domain/workload/...`) | the SPIRE server's CA / signing key (its own OIDC discovery endpoint) | *many* workloads across *many* clusters/clouds, with workload attestation + mTLS |

**They stack:**

```
workload ──attested by──> SPIRE agent ──issues──> JWT-SVID (aud = GCP WIF audience)
                                                      │
                          GCP WIF provider  ◀── trusts SPIRE's OIDC issuer ──┐
                          Azure WIF fed-cred ◀── trusts SPIRE's OIDC issuer ─┤
                                                                             │
   ⇒ one SVID → exchanged for a GCP access token OR an Entra token, keyless ─┘
```

SPIRE replaces the *k0s SA issuer* as the federated IdP: you register **SPIRE's
OIDC discovery endpoint** (`spire-oidc-discovery-provider`) as the `issuer_uri`
in the GCP WIF provider (and as an Entra federated credential), instead of the
raw cluster issuer. SPIRE adds: workload attestation (process/k8s/unix
selectors, not just "any pod with this SA"), X.509-SVIDs for **mTLS between
services** (which WIF alone gives you nothing for), automatic SVID rotation
(minutes), and **SPIRE-to-SPIRE federation** across trust domains/regions.

Bidirectional Entra↔GCP WIF is **orthogonal**: it's cloud-plane auth and stays
useful whether or not SPIRE is present. With SPIRE you'd often point *both*
clouds at SPIRE and stop maintaining a direct Entra↔GCP trust.

### Cost / when to adopt

SPIRE is real operational weight: an HA SPIRE server + datastore, node
attestation bootstrap, and the OIDC discovery provider must be publicly
reachable — **the same JWKS-exposure risk** as the k0s issuer, now for the
whole fleet.

| Situation | Identity plane |
|---|---|
| 1 k0s node + the build plane (**now**) | k0s SA issuer → GCP WIF. Add the Entra provider (already in `wif-k0s.tf`) when a workload needs Azure. |
| A few clusters, 2 clouds | keep per-cluster-issuer WIF, or make Entra the single root both clouds federate from |
| Many clusters / many clouds / need service-to-service mTLS / per-workload (not per-SA) identity | **SPIRE** trust domain as the root; GCP + Azure WIF both trust SPIRE's OIDC; optional SPIRE↔SPIRE federation |

**Recommendation:** ship the k0s-issuer + Entra WIF (this PR). Treat SPIRE as a
*documented future consolidation* triggered by the 1000s-of-services / multi-
cloud reality — not now. When it lands, the WIF providers change their
`issuer_uri` to SPIRE's; the SA bindings and `gcs-obj.sh` do not change.

## Part 2 — templating: jinja/pyinfra vs kustomize vs …

Two different config surfaces, two different right answers:

| Surface | Files | Tool | Why |
|---|---|---|---|
| **dstack server config** on the control node | `nats/pyinfra/templates/dstack-server-config.yml.j2` | **jinja via pyinfra `files.template`** — keep as-is | it's *one non-k8s YAML file*, rendered as part of a pyinfra run that also installs packages / units / does post-checks. Conditionals (`{% if k0s_kubeconfig %}`) are exactly what jinja is for. kustomize can't touch a non-k8s file; a second tool here buys nothing. |
| **k0s cluster config** | `k0s.yaml` (`spec.api.extraArgs`) | a single file — **k0sctl** if we want managed lifecycle, else pyinfra `files.template` / a documented edit | k0s config is a k0s-specific schema, not k8s manifests; kustomize doesn't apply. k0sctl owns *install/upgrade of k0s itself*. |
| **k8s manifests** (WIF SA, ConfigMaps, the AR-pull CronJob, RBAC, later the pfnet webhook, SPIRE) | `deploy/k0s/` | **kustomize** (`kubectl kustomize` / `kubectl apply -k`) | native, declarative, **no templating language**, overlays for env differences, `configMapGenerator` for the ADC config + script. Ships in `kubectl`. |

### Why not …

- **Helm** — overkill for ~8 objects we author and fully control; adds a
  templating language (Go tpl) and a release lifecycle we don't need. Reach for
  it only to *consume* third-party charts (e.g. SPIRE's official chart).
- **jsonnet / ytt / cue** — more powerful than kustomize, but a new language +
  toolchain for the team to learn for a small manifest set. Not justified yet.
- **jinja for the k8s YAML** — works, but you lose `kubectl`'s schema
  validation, `kustomize` overlays, and you reintroduce string-templating YAML
  (the class of bug kustomize exists to remove). Don't.
- **Pure `kubectl apply -f` of static files** — no way to vary
  `PROJECT_NUMBER` / namespace per env without sed pipelines; kustomize's
  `namespace:` + `replacements:` do it cleanly.

### Net

- pyinfra + jinja stays for the **control-node** server config (right fit).
- **kustomize** is the standard for everything **inside the cluster**
  (`deploy/k0s/`, added in this PR).
- **k0sctl** if/when we want k0s's own lifecycle under IaC (separate concern).
- **Helm** only to install upstream charts (SPIRE, the pfnet webhook, cert-
  manager) — not for our own manifests.

The `PROJECT_NUMBER`/`PROJECT_ID` placeholders in `deploy/k0s/` are substituted
by a one-line `sed` at apply time (documented in the README) rather than a
kustomize `replacement`, only so `kubectl kustomize` builds with zero inputs in
CI; switch to a `replacements:` block driven by a small `env.yaml` once there's
a second environment.

## Sources

- https://spiffe.io/docs/latest/keyless/ / SPIRE OIDC discovery provider
- https://github.com/spiffe/spire  ·  https://artifacthub.io/packages/helm/spiffe/spire
- https://cloud.google.com/iam/docs/workload-identity-federation (OIDC providers)
- https://learn.microsoft.com/entra/workload-id/workload-identity-federation
- kustomize: https://kubectl.docs.kubernetes.io/references/kustomize/
