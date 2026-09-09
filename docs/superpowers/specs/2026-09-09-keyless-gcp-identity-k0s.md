# Keyless GCP identity for k0s-on-Vultr — and cross-cloud unification

**Date:** 2026-09-09 · plan gleaming-jingling-nygaard.
**Question (operator):** can k0s-on-Vultr have a "GCE metadata SA"? Can we unify
Entra ID + GCP credentials and retire long-lived keys?

## TL;DR

- **No metadata SA.** GCE metadata identity is injected by Google's hypervisor —
  a Vultr VM cannot have one. Vultr's own metadata service only exposes Vultr
  data.
- **Yes, keyless is achievable** via **GCP Workload Identity Federation (WIF)**
  with the **k0s cluster's own OIDC issuer** as the trusted IdP. A Pod's
  projected ServiceAccount token is exchanged at Google STS for a
  federated/impersonated access token — no key file. This retires the
  `gcs_sa_key` and `ar-pull` keys from PR #1280.
- **Entra ID unification** is a natural extension: GCP WIF already accepts any
  OIDC/SAML IdP (we use it for GitHub Actions today). Add Entra as a second WIF
  provider, or make one issuer the root of trust for both clouds.
- **`gcs-obj.sh` now understands `external_account` configs** (STS token
  exchange), so the workload code is already ready for the keyless path —
  only the infra wiring remains.

## Why there's no metadata SA on Vultr

| Cloud | Ambient workload identity | Mechanism |
|---|---|---|
| GCP | GCE metadata server (`169.254.169.254`) | hypervisor-injected SA token |
| AWS | IMDSv2 | hypervisor-injected role creds |
| Azure | IMDS | hypervisor-injected managed identity |
| **Vultr** | **none for foreign clouds** | Vultr metadata = Vultr API data only |

So a k0s node in Vultr has no ambient GCP identity. The identity has to come
from a layer we control: **the Kubernetes ServiceAccount token issuer**, or an
**external IdP** (Entra).

## Keyless path: WIF ↔ the k0s OIDC issuer

This is how GKE Workload Identity / EKS IRSA work under the hood; it also works
for any self-managed cluster whose SA issuer is publicly discoverable.

### 1. Make the k0s SA issuer discoverable (one-time)

```sh
# on b00t-node
kubectl get --raw /.well-known/openid-configuration > oidc-config.json
kubectl get --raw /openid/v1/jwks                   > jwks.json
# host them at a stable public HTTPS URL (a public GCS bucket is fine)
gsutil cp oidc-config.json jwks.json gs://b00t-k0s-oidc/   # bucket = allUsers objectViewer
```

Point the k0s API server at that public issuer URL so the `iss` in projected
tokens matches what GCP will fetch:

```yaml
# k0s.yaml -> spec.api.extraArgs
service-account-issuer: "https://storage.googleapis.com/b00t-k0s-oidc"
service-account-jwks-uri: "https://storage.googleapis.com/b00t-k0s-oidc/jwks.json"
```

🚩 **The public JWKS is now a trust anchor.** If an attacker can overwrite it
they can mint tokens for any GSA that trusts this issuer. Mitigations: bucket
write locked to one principal + object versioning; tight WIF
`attribute_condition` (exact namespace + KSA); minimal roles on the bound GSAs;
rotate the SA signing key on a schedule.

### 2. WIF pool + provider (Terraform — extends b00t-tf's existing pool pattern)

```hcl
resource "google_iam_workload_identity_pool" "k0s" { workload_identity_pool_id = "k0s" }

resource "google_iam_workload_identity_pool_provider" "k0s_oidc" {
  workload_identity_pool_id          = google_iam_workload_identity_pool.k0s.workload_identity_pool_id
  workload_identity_pool_provider_id = "b00t-node"
  attribute_mapping = {
    "google.subject"        = "assertion.sub"                  # system:serviceaccount:<ns>:<ksa>
    "attribute.namespace"   = "assertion['kubernetes.io']['namespace']"
    "attribute.ksa"         = "assertion['kubernetes.io']['serviceaccount']['name']"
  }
  attribute_condition = "assertion.sub == 'system:serviceaccount:default:b00t-ci'"
  oidc { issuer_uri = "https://storage.googleapis.com/b00t-k0s-oidc" }
}
```

Bind the existing scoped SAs to the federated principal (no key):

```hcl
resource "google_service_account_iam_member" "gcsread_wif" {
  service_account_id = "…/b00t-k0s-gcsread"
  role   = "roles/iam.workloadIdentityUser"
  member = "principalSet://iam.googleapis.com/${pool}/attribute.ksa/b00t-ci"
}
```

### 3. Pod wiring — two clean options

- **Manual (works now):** projected-token volume + a small `external_account`
  ADC config as a ConfigMap; set `GOOGLE_APPLICATION_CREDENTIALS` to it.
  `gcs-obj.sh` already handles this config type.

  ```yaml
  volumes:
    - name: gcp-token
      projected:
        sources:
          - serviceAccountToken:
              audience: "//iam.googleapis.com/${WIF_AUDIENCE}"
              expirationSeconds: 3600
              path: token
  ```

  `credential-configuration.json` (generate once with
  `gcloud iam workload-identity-pools create-cred-config`):

  ```json
  { "type": "external_account",
    "audience": "//iam.googleapis.com/projects/NUM/locations/global/workloadIdentityPools/k0s/providers/b00t-node",
    "subject_token_type": "urn:ietf:params:oauth:token-type:jwt",
    "token_url": "https://sts.googleapis.com/v1/token",
    "credential_source": { "file": "/var/run/secrets/tokens/gcp-token/token" },
    "service_account_impersonation_url": "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/b00t-k0s-gcsread@promptexecution.iam.gserviceaccount.com:generateAccessToken" }
  ```

- **Automatic (production):** deploy
  [`pfnet-research/gcp-workload-identity-federation-webhook`](https://github.com/pfnet-research/gcp-workload-identity-federation-webhook)
  — a mutating webhook that injects the token volume + ADC config into any Pod
  whose ServiceAccount carries a `cloud.google.com/workload-identity-provider`
  annotation. Same UX as GKE. Preferred once more than one workload needs it.

### 4. Artifact Registry image pull (the fiddly bit)

The Pod's ADC covers *the workload*, not *the kubelet's image pull*. Options:

- A **DaemonSet/CronJob** that exchanges the federated token and refreshes the
  `ar-pull` dockerconfig Secret every ~45 min (short-lived, keyless).
- A **kubelet credential provider plugin** doing the WIF exchange at pull time
  (cleanest, needs a node binary + config).
- Interim: the scoped `artifactregistry.reader` key (PR #1280) until one of the
  above lands.

## Cross-cloud unification (Entra ID + GCP)

GCP WIF is IdP-agnostic — it already trusts GitHub's OIDC in this project. Two
ways to fold Entra in:

1. **Entra as an additional GCP WIF provider.** An Entra-issued token (managed
   identity / app registration / federated credential) → GCP SA, keyless.
   Symmetric: Azure Workload Identity Federation accepts GCP/GitHub OIDC tokens
   into an Entra app, so a GCP workload can call Azure keyless too.
2. **One issuer as the root of trust.** Either Entra ID or a dedicated workload
   identity plane:
   - **SPIFFE/SPIRE** — issues short-lived JWT-SVIDs; both GCP WIF and Azure WIF
     consume them. The canonical "portable workload identity across AWS/GCP/
     Azure" answer. Heavier; justified when the fleet spans clouds at scale.
   - **Entra ID as the org IdP** — if PromptExecution's identity root is already
     Entra, register it once with GCP WIF and let everything federate from it.

**Recommendation:** don't boil the ocean. Sequence:

1. **PR #1280 as-is** — scoped keys as bootstrap (already flagged interim).
2. **k0s-issuer WIF** (steps 1–3 above) — retires `gcs_sa_key`. Small, isolated,
   `gcs-obj.sh` is ready.
3. **AR pull via refresher DaemonSet** — retires `ar-pull` key.
4. **Add Entra as a WIF provider** when a workload actually needs Azure + GCP in
   one place. Evaluate SPIFFE/SPIRE only if the cross-cloud fleet grows past a
   handful of nodes.

## Code status

`dev-env/gcs-obj.sh` credential resolution order is now:
`GCE metadata` → **`external_account` (WIF, keyless STS exchange)** →
`service_account` key (bootstrap). Offline test `dev-env/tests/gcs-obj-jwt-test.sh`
covers both the key path and WIF branch selection + STS request assembly.

## Sources

- https://github.com/salrashid123/k8s_federation_with_gcp
- https://github.com/pfnet-research/gcp-workload-identity-federation-webhook
- https://docs.siderolabs.com/kubernetes-guides/advanced-guides/gcp-workload-identity
- https://www.systemshardening.com/articles/cross-cutting/cross-cloud-oidc-federation/
- https://www.systemshardening.com/articles/cross-cutting/gcp-workload-identity-federation/
- https://docs.gitlab.com/ci/cloud_services/google_cloud/
