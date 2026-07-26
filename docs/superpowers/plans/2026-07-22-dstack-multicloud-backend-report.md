# Report: Multi-Cloud Backend Options for the dstack Provider

**Date:** 2026-07-22
**Status:** Findings + recommendation, not yet acted on
**Prompted by:** operator question — "we have all the major clouds... what others tools would commonly be used with dstack and are those tools or similar tools or patterns already in use or implemented in b00t?"

## What's actually authorized on this host

Checked directly (not assumed):

```
$ which az gcloud aws terraform tofu
/usr/bin/az
/home/brianh/.b00t/google-cloud-sdk/bin/gcloud
/usr/local/bin/aws
/usr/local/bin/tofu

$ az account show      → live Azure subscription (AzureCloud, tenant 1fd87b50-...)
$ gcloud config list   → account=brianh@elastic.ventures, project=app4dog
$ aws sts get-caller-identity → live AWS account 968589500754
```

All three cloud CLIs are installed **and already authenticated** on this host. This confirms the operator's premise: Azure, GCP, and AWS are each independently usable right now, not a hypothetical.

## What app4dog's existing multi-cloud infrastructure actually is

`app4dog/terraform` symlinks to a sibling repo: `~/promptexecution/infrastructure/terraform/app4dog/` — a real, actively-used OpenTofu stack (`.terraform.lock.hcl`, remote state present). Provider blocks confirm: `aws`, `azurerm`, `google`/`google-beta`, `cloudflare`, `github`.

Surveyed what each cloud's resources actually are (`resource "..."` blocks, not just provider declarations):

| File | Cloud | What it provisions |
|---|---|---|
| `cloud_run.tf` | GCP | Cloud Run domain mapping + service accounts/IAM for the **middleware API** — app hosting, not ML |
| `azure_app4dog.tf` | Azure | Resource group + Azure Maps account — not compute |
| `cvat_hitl.tf` | Azure | **CVAT (annotation tool) on Azure Container Apps** — serverless containers, relevant to ML *dataset labeling*, not GPU training/inference |
| `ecr_image_segmenter.tf` + `image-segmenter.tf` | AWS | ECR registry + CI push IAM roles for a "segmenter" image — a container **registry**, not confirmed GPU compute; where this image actually *runs* isn't in this Terraform (open question, not assumed) |
| `r2_gameplay.tf` | Cloudflare | R2 object storage for gameplay assets — storage, not compute |

**Conclusion: none of app4dog's existing multi-cloud Terraform footprint is GPU compute.** It's all stable, always-on application infrastructure (API hosting, DNS/storage, CI registries, one annotation tool). There is no existing GKE/AKS/EKS GPU node pool, no existing GPU VM, nothing dstack could attach to and inherit for free.

## What dstack actually needs per cloud (verified against dstack's own docs, not guessed)

| Backend | Credentials | Pre-existing infra required? |
|---|---|---|
| AWS | access key/secret (or default creds) | No — self-provisions; optional `vpc_name`/`vpc_ids` and IAM instance profile if you want to reuse existing network |
| GCP | service account file | No — self-provisions; optional `vpc_name` for existing network |
| Azure | client ID/secret (or default creds) | No — self-provisions a resource group and network if none specified |

dstack's default behavior on all three clouds is the same shape as RunPod: point it at credentials, it provisions ephemeral compute itself. Bring-your-own-VPC/IAM is *supported*, not required.

## Recommendation

1. **Don't wire Azure/GCP/AWS as dstack backends in this plan.** There's no existing GPU infra to integrate with, and dstack would create brand-new, disconnected resources in each cloud if pointed at them — that's additional surface area and idle-cost risk (a stray GPU VM in a cloud nobody's watching), not a win. RunPod-via-dstack (this plan's actual scope) already solves the stated pain (cycle time, reliable PASS/FAIL) without it.
2. **One thing worth a future look, not now:** GCP Cloud Run added GPU support recently, and app4dog already has a live GCP project + IAM (`cloud_run.tf` already manages `app4dog_middleware_sa`). If a workload ever wants serverless-scale-to-zero GPU inference (not batch training/mesh-gen), Cloud Run GPU could reuse an already-authenticated project instead of a new backend from scratch. Flagging it, not scoping it — no action here.
3. **`ecr_image_segmenter.tf`'s actual runtime target is an open question worth a 10-minute follow-up**, independent of this plan: it provisions a registry and CI push permissions but not confirmed compute — worth checking whether there's an AWS Batch/ECS/Lambda job actually running that image, since if so it's a second, currently-undocumented ML-pipeline compute path alongside RunPod/dstack that this plan's audit didn't know about.
4. If multi-cloud GPU orchestration does become a real need later (not indicated by current evidence), dstack already supports it with zero new IaC required — it's a config.yml addition (per the spec's Task 8 datum), not a new subsystem to build.

# b00t:map v1
# summary: Multi-cloud backend research for the dstack provider — confirms Azure/GCP/AWS CLI auth is live, app4dog's existing Terraform infra has no GPU compute to integrate with, dstack self-provisions per-cloud with no Terraform dependency, recommends staying RunPod-only for now
# tags: dstack, multi-cloud, terraform, azure, gcp, aws, report
# tier: frontier
# complexity: 2
