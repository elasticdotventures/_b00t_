# gcp-bootstrap

One-time bootstrap for the b00t GCP build plane. **Run once**, then leave it alone.

## What it creates

| Resource | Detail |
|---|---|
| `google_storage_bucket.tf_state` | `b00t-tf-state-promptexecution`, `australia-southeast1`, versioned, UBLA, public-access `enforced`, `prevent_destroy = true`. Holds the **root module's** remote state. |
| `google_project_service.api` | Enables `cloudresourcemanager`, `compute`, `iam`, `iamcredentials`, `sts`, `storage`, `serviceusage`. `disable_on_destroy = false`. |

## Why it is separate

The root `b00t-tf` module will use `backend "gcs"` pointed at the bucket above.
You cannot configure that backend against a bucket that does not exist yet, and
you do not want the state bucket's own lifecycle tangled in the state it stores.
So this module has **local state** (no `backend` block) and is applied by hand
with operator Application Default Credentials.

## Run

```sh
gcloud auth application-default login          # if the ADC token is stale
gcloud config set project promptexecution
cd b00t-tf && just gcp-bootstrap               # tofu -chdir=modules/gcp-bootstrap init && apply
```

`.terraform/` and `*.tfstate*` here are git-ignored. Keep
`modules/gcp-bootstrap/terraform.tfstate` as a local artifact (or commit it
encrypted, operator choice) — it only tracks the bucket + the API toggles.

## Break-glass (no OpenTofu)

```sh
gcloud storage buckets create gs://b00t-tf-state-promptexecution \
  --location=australia-southeast1 --uniform-bucket-level-access \
  --public-access-prevention
gcloud storage buckets update gs://b00t-tf-state-promptexecution --versioning
gcloud services enable cloudresourcemanager.googleapis.com compute.googleapis.com \
  iam.googleapis.com iamcredentials.googleapis.com sts.googleapis.com \
  storage.googleapis.com serviceusage.googleapis.com --project promptexecution
```

## Decommission

1. Migrate root state back to local: `cd b00t-tf && tofu init -migrate-state`.
2. Remove the `lifecycle { prevent_destroy = true }` block from `main.tf` and
   re-apply this module.
3. `just gcp-bootstrap-destroy`.
