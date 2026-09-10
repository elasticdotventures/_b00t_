# k0s CI test Job — SPIRE-identity, keyless GCS

`dev-env/k0s-ci-test.task.yaml` documents this shape as a dstack task, but
dstack 0.21.5's `kubernetes` backend has **no pod-spec injection** — it can't
add the `csi.spiffe.io` volume, the `spiffe-helper` sidecar, the ADC
ConfigMap, or `serviceAccountName`. So the identity-carrying variant runs as
a plain k8s Job instead (b00t task #193). Plain, identity-free compile-free
partitions still go through `dstack apply -f dev-env/ci-test.task.yaml`.

## What it proves

```
Pod (SA b00t-ci, ns b00t-ci)
  └─ spiffe-csi-driver mounts the Workload API socket
  └─ spiffe-helper (native sidecar) fetches a JWT-SVID  -> /svid/jwt_svid.token
        aud //iam.googleapis.com/projects/308167228204/…/providers/spire-provider
  └─ run: gcs-obj.sh (external_account) -> STS token exchange
        -> impersonate b00t-buildplane-ci  (roles/storage.objectViewer on the bucket)
        -> KEYLESS GET gs://b00t-buildcache-promptexecution/<archive>
  └─ cargo nextest run --archive-file … --partition …   (no rustc)
```

No SA key, no GCE metadata server. Identity + WIF binding live in
`PromptExecution/infrastructure` (`terraform/google/google-oidc-spire-buildplane.tf`,
`k0s/spire/`). Prereqs already on the cluster: the `spire-agent` +
`spiffe-csi-driver` DaemonSets in ns `spire`, the `b00t-ci` SA in ns
`b00t-ci`, and the SPIRE registration entry
`spiffe://promptexecution.com/ns/b00t-ci/sa/b00t-ci` (selectors `k8s:ns:b00t-ci`
+ `k8s:sa:b00t-ci`).

## Run

```sh
# CI_ARCHIVE_KEY is what ci-build.task.yaml published, e.g.:
deploy/k0s-ci-test/run.sh ci-archives/34428356633-1 1/16
```

`run.sh` applies the ConfigMap + a `$`-substituted Job, waits, streams logs,
prints the Job condition. Sized for vultr1 (2 vCPU / ~3.7 GiB / ~67 GiB eph)
— keep the partition fraction small (`1/16` default); the full test fan-out
belongs on GCP Spot (`ci-build-plane.yml`). `kubectl` is reached over the
tailnet (`ssh root@100.109.101.1 k0s kubectl`); set `KUBECTL="k0s kubectl"`
to run on the node.

## Dagster

`orchestration/dagster/` currently drives `dstack apply` only. A
`KubernetesJobResource` op that shells this `run.sh` is the natural follow-up
so `ci_build_plane` can route the identity-bearing partition through k0s
while the rest stay on Spot.
