# deploy/k0s — keyless GCP identity for the kubernetes dstack backend

Kustomize base that makes k0s-on-Vultr talk to GCP (GCS archives + Artifact
Registry pulls) with **no long-lived key** — Workload Identity Federation off
k0s's own ServiceAccount-token issuer. Retires the two 🚩 keys from the
k0s/SOCI PR. Full design: `docs/superpowers/specs/2026-09-09-keyless-gcp-identity-k0s.md`.

## Contents

| File | What |
|---|---|
| `namespace.yaml` | `b00t-ci` namespace |
| `serviceaccount.yaml` | `b00t-ci` KSA — its token `sub` is what GCP WIF trusts; annotations drive the pfnet webhook |
| `files/credential-configuration.json` | `external_account` ADC config (projected token → STS) |
| `ar-pull-refresher.yaml` | CronJob (+ SA/Role/RoleBinding) refreshing the `ar-pull` dockerconfigjson Secret with a short-lived WIF token every 40 min |
| `files/ar-pull-refresh.sh` | the refresher script (ConfigMap-mounted) |
| `publish-oidc-discovery.sh` | run on b00t-node: push the OIDC discovery doc + JWKS to a public bucket |
| `kustomization.yaml` | ties it together; `PROJECT_NUMBER` / `PROJECT_ID` are placeholders |

## Apply order

1. **Terraform** — set in `modules/gcp-build-plane`:
   `k0s_oidc_issuer_uri = "https://storage.googleapis.com/b00t-k0s-oidc"`
   (and `entra_tenant_id` / `entra_app_id` for the Entra provider). `tofu apply`.
2. **b00t-node** — public bucket + publish discovery:
   ```sh
   gcloud storage buckets create gs://b00t-k0s-oidc --location=US --uniform-bucket-level-access
   gcloud storage buckets add-iam-policy-binding gs://b00t-k0s-oidc --member=allUsers --role=roles/storage.objectViewer
   BUCKET=b00t-k0s-oidc deploy/k0s/publish-oidc-discovery.sh
   ```
   Point k0s at the issuer (`k0s.yaml` → `spec.api.extraArgs`:
   `service-account-issuer`, `service-account-jwks-uri`), restart k0s.
3. **Cluster** — apply this base with the project placeholders substituted:
   ```sh
   PN=$(gcloud projects describe promptexecution --format='value(projectNumber)')
   kubectl kustomize deploy/k0s \
     | sed -e "s/PROJECT_NUMBER/$PN/g" -e "s/PROJECT_ID/promptexecution/g" \
     | kubectl apply -f -
   ```
4. **Webhook (recommended)** — install
   [`pfnet-research/gcp-workload-identity-federation-webhook`](https://github.com/pfnet-research/gcp-workload-identity-federation-webhook)
   so dstack-created Pods using the `b00t-ci` SA get the token volume + ADC
   config injected automatically. Without it, dstack task configs must mount
   `cm/gcp-wif-credential-config` + a projected `serviceAccountToken` volume
   themselves, and set `GOOGLE_APPLICATION_CREDENTIALS` — which
   `dev-env/gcs-obj.sh` already understands (`external_account` branch).
5. Drop the `gcs_sa_key` / `ar-pull` **key**-based secrets from the runbook once
   a k8s task run succeeds keyless.

## Validate offline

```sh
kubectl kustomize deploy/k0s >/dev/null && echo OK
bash -n deploy/k0s/files/ar-pull-refresh.sh deploy/k0s/publish-oidc-discovery.sh
```
