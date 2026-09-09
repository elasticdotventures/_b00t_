# Identity plane — minimum requirements for the multi-cloud end state

**Date:** 2026-09-09 · plan gleaming-jingling-nygaard.
**Framing (operator):** this is a draft of a *much* bigger system — do not
optimise for staying small. Keyless preferred. **AWS is added as soon as this
pattern stabilises** (→ GCP + Azure + AWS). What are the minimum requirements,
and how does the known end state change the calculus?

## The calculus change

Earlier docs (`2026-09-09-keyless-gcp-identity-k0s.md`,
`...-spiffe-spire-cross-cloud.md`) said: start with a per-cluster k0s
ServiceAccount-token issuer federated to GCP WIF, adopt SPIRE "later, if scale
demands." **That hedge assumed the system might stay small. It won't.**

With multi-cloud + large known up front, the per-cluster-issuer approach is a
**dead end**, because of the N×M trust matrix:

| approach | add a cloud | add a cluster | trust configs to hand-maintain |
|---|---|---|---|
| per-cluster OIDC issuer + per-cloud federation | reconfigure **K** providers | reconfigure **C** providers + C conditions | **C × K** |
| **one trust root**, each cloud federates once | +1 provider (once) | cluster just joins the root | **C** |

At C=3 clouds and a growing K, per-cluster federation becomes unmanageable and
has to be unwound. So the end-state identity root is not optional and not
"later" — it is the thing to build **before AWS onboards**.

## Minimum requirements

1. **One trust root issues all workload identity.** A single SPIFFE trust
   domain (`spiffe://b00t.promptexecution.com/...`) backed by SPIRE. Every
   workload — k8s pod, EC2 instance, on-prem VM, CI runner — gets its identity
   from here, not from its local platform.

2. **Every cloud federates against that one root, keyless — no static keys:**
   - **GCP** — Workload Identity Federation OIDC provider, `issuer_uri` = the
     SPIRE OIDC Discovery Provider URL. Consumes **JWT-SVID**.
   - **Azure** — federated identity credential, same issuer. Consumes **JWT-SVID**.
   - **AWS** — pick per workload:
     - **IAM OIDC identity provider** (`AssumeRoleWithWebIdentity`) — JWT-SVID,
       best for k8s pods.
     - **IAM Roles Anywhere** with SPIRE's CA as the trust anchor — **X.509-SVID**,
       best for EC2 / on-prem / non-k8s. AWS is explicitly converging Roles
       Anywhere + OIDC + SPIFFE on one trust-policy condition.
   - SPIRE issues *both* JWT-SVID and X.509-SVID from the same identity, so the
     workload picks the entry path per cloud with no separate credential.

3. **Attestation, not possession.** Workload entries bind to *attested*
   selectors — k8s: `namespace + serviceaccount + image digest`; EC2: instance
   identity document; on-prem: node attestation via join token → node SVID.
   A single pod RCE cannot assume another workload's identity. Raw
   "any pod with ServiceAccount X" (the k0s-issuer bootstrap) is explicitly
   *not* good enough for the end state.

4. **Short-lived, auto-rotated, everywhere.** SVID TTL in minutes; exchanged
   cloud credentials ≤ 1 h; zero static keys, zero long-lived refresh tokens.

5. **Least privilege per cloud identity.** Each GCP SA / Azure identity / AWS
   role scoped minimally; the cloud-side trust condition pins the exact SPIFFE
   ID (or a tightly-scoped path prefix), never a wildcard.

6. **The trust anchor is HA, integrity-protected, and monitored.** This is the
   single "compromise ⇒ mint any identity" point:
   - SPIRE server HA (≥ 3 replicas, a real datastore — CloudSQL/RDS/Cockroach,
     not sqlite).
   - The OIDC Discovery Provider on a **stable public HTTPS URL** with its own
     managed cert (ACME), fronted by a CDN, versioned, **alerted on**.
   - Signing-key rotation on a schedule; JWKS publishes current + next.

7. **Federation-ready from day one.** One trust domain now; **SPIRE ↔ SPIRE
   federation** (per-region / per-business-unit trust domains that cross-trust)
   is the scaling path — additive, not a rebuild.

8. **A registration control loop.** `spire-controller-manager` (or equivalent)
   reconciles workload entries from k8s CRDs / annotations. Humans do not
   hand-register thousands of workloads.

## Revised sequence

| # | Step | Status | Notes |
|---|---|---|---|
| 1 | **#1280 bootstrap keys** (scoped `objectViewer` / `artifactregistry.reader`) | authored (#1280) | unblocks the build plane immediately |
| 2 | **#1280 k0s-issuer WIF (GCP only)** | authored (#1280) | **deliberately temporary.** Proves the WIF mechanics — bucket, provider, attribute conditions, `gcs-obj.sh` `external_account` path — *all of which transfer to SPIRE unchanged* (SPIRE only swaps the `issuer_uri`). Cap the spend; build **no** per-cluster tooling. |
| 3 | **SPIRE trust domain** | **authored (#1280)** — `deploy/k0s/spire/` (Helm values + `ClusterSPIFFEID` CR + CSI-driver delivery), `spire-oidc` provider in `wif-k0s.tf` (gated). Production hardening (datastore, CA, ingress cert, monitoring) is the staffed work. | server HA + datastore; k8s workload attestor + `spire-controller-manager`; OIDC Discovery Provider on a public URL. Re-point GCP + Azure WIF `issuer_uri` at SPIRE. Build plane = tenant #1. |
| 4 | **AWS onboards against SPIRE** | **authored (#1280)** — `modules/identity-aws/` (IAM OIDC provider + web-identity role for JWT-SVID; Roles Anywhere trust anchor + profile + role for X.509-SVID), all gated. `tofu validate` Success. | **Never** a per-cluster AWS issuer. |
| 5 | **SPIRE ↔ SPIRE federation** | design only | `ClusterFederatedTrustDomain` CRs; additive as regions / BUs multiply |

All of steps 1–4 are authored + offline-validated in PR #1280 and gated
(empty vars → zero resources). They are **not applied** — steps 3–4 in
particular need staffed platform work (SPIRE datastore/CA/ingress, AWS account
access) before `tofu apply` / `helm install`.

Steps 1–2 are the current PR and are still worth shipping: they are the
keyless-mechanics spike, they de-risk step 3, and every artifact is reused.
The discipline is to **not extend step 2** — no second per-cluster issuer, no
elaborate bootstrap tooling. The moment a *second* cluster or a *second* cloud
shows up, step 3 is the answer, not another step-2.

## What #1280 already gets right (forward-compatible)

- WIF pool named `external-idp` — generic; the SPIRE provider slots in beside
  `k0s-oidc`, no rename.
- `attribute_mapping: google.subject = assertion.sub` — `sub` becomes the
  SPIFFE ID with no change.
- `dev-env/gcs-obj.sh` `external_account` path — issuer-agnostic; a SPIRE
  JWT-SVID works through the same STS exchange.
- `deploy/k0s/` ConfigMap + refresher-CronJob pattern survives; under SPIRE the
  token *delivery* mechanism changes (SPIRE CSI driver / `spiffe-helper`
  instead of the pfnet webhook), the *consumption* does not.

### Small changes worth making now (cheap, signal intent)

- Label the `k0s-oidc` provider and the `deploy/k0s/` webhook path **"bootstrap
  — superseded by SPIRE (step 3)"** in comments, so nobody builds on it as if
  permanent.
- Add a `trust_domain` input (unused by resources yet) to the WIF module and
  the runbooks, so the SPIFFE naming is fixed before workloads start minting
  IDs.

## Cost honesty

SPIRE is a **platform capability, staffed** — not a side-quest. HA server +
datastore, per-environment node-attestation bootstrap, the registrar control
loop, the public OIDC endpoint + cert lifecycle, and SPIRE's fast release
cadence. Budget it as such.

The payoff: SPIRE is the **only** option that covers the full requirement set —
{ keyless, multi-cloud GCP+Azure+AWS, workload *attestation*, service mTLS,
O(1) onboarding of new clouds/clusters } — which the stated end state requires.
Anything less (per-cluster issuers, a shared Dex, hosted-OIDC-only) fails at
least one of attestation, mTLS, or the N×M problem.

## Sources

- https://spiffe.io/docs/latest/keyless/oidc-federation-aws/
- https://hidekazu-konishi.com/entry/aws_iam_inbound_workload_federation.html — Roles Anywhere + OIDC + SPIFFE convergence
- https://spiffe.io/docs/latest/planning/scaling_spire/
- https://developers.redhat.com/blog/2026/04/23/sky-computing-openshift-service-mesh-spire-multicloud-integration
- https://www.scrambleid.com/learn/cloud-workload-identity-compared
- https://www.systemshardening.com/articles/cross-cutting/spiffe-spire-workload-identity/
