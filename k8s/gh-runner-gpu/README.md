# gh-runner-gpu — GPU self-hosted CI runner (reviewable artifact, NOT applied)

Offers sm3llsl1k3s0ld3r's GPU (and warm local NVMe caches) to GitHub Actions
CI for `elasticdotventures/_b00t_`, via the actions-runner-controller (ARC)
install already running on this box's k0s cluster. Directly targets the
CI-slowness pain in [#1226](https://github.com/elasticdotventures/_b00t_/issues/1226)
(30–45 min `cargo test --workspace`) as a complement to the sccache work in
PR #1227 — this gives CI a warm `$CARGO_HOME` and a GPU instead of a cold
`ubuntu-latest` runner every time.

**Nothing here has been applied or registered.** No secret has been
created, no `helm install` has been run, no workflow has been wired into
the normal PR trigger path. This directory + `.github/workflows/ci-gpu.yml`
are for review first.

## What already exists on the cluster (verified, not assumed)

- ARC controller is installed and healthy: `arc-gha-rs-controller` deployment
  in namespace `arc-systems`, chart `gha-runner-scale-set-controller-0.14.2`
  (`helm list -n arc-systems`). It runs with `--auto-scaling-runner-set-only`
  and a `ClusterRoleBinding`, so it can manage an `AutoscalingRunnerSet` in
  any namespace — it does not need to live in `arc-systems`.
- **Zero** `AutoscalingRunnerSet` resources exist anywhere yet
  (`k0s kubectl get autoscalingrunnersets -A` → no resources found). The
  controller is ready; nothing is registered against it.
- GPU is fully wired: node label `nvidia.com/gpu.present=true`,
  `nvidia.com/gpu: 3` allocatable (time-sliced RTX 3090),
  `nvidia-device-plugin-daemonset` running in `kube-system`, and a
  pre-existing `RuntimeClass nvidia` (handler: `nvidia`) that
  `values.yaml` in this directory uses.
- The node has no taints, so no `tolerations` block is required (unlike the
  stale manifest below, which tolerates `node-role.kubernetes.io/control-plane`
  for the same reason — same single-node cluster).

## What does NOT exist yet (the actual blocker)

**A GitHub token secret.** `k0s kubectl get secrets -n arc-systems` shows
only the Helm release secret itself — nothing containing a PAT or GitHub
App installation token. `values.yaml` references a secret named
`gh-runner-gpu-creds` in a new `arc-runners` namespace
(`githubConfigSecret: gh-runner-gpu-creds`) that does not exist. Create it
before installing anything:

```bash
k0s kubectl create namespace arc-runners
k0s kubectl create secret generic gh-runner-gpu-creds \
  --namespace arc-runners \
  --from-literal=github_token=<PAT-or-GitHub-App-installation-token>
```

See `secret.example.yaml` for the exact shape and token-scope notes. This
step is intentionally NOT automated by this PR — minting a token is a human
decision (which credential, whose account, PAT vs. GitHub App).

Once the secret exists, the install itself would be:

```bash
helm install gh-runner-gpu \
  oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set \
  --version 0.14.2 \
  --namespace arc-runners \
  -f k8s/gh-runner-gpu/values.yaml
```

That command is documented, not run, by this PR.

## Security tradeoff: why the companion workflow is `workflow_dispatch`-only

`elasticdotventures/_b00t_` is a **public** repo (`gh repo view` →
`"visibility":"PUBLIC"`). Self-hosted runners on a public repo that accepts
`pull_request`-triggered CI are a well-known attack vector: a malicious
fork PR's workflow run gets arbitrary code execution on the runner's actual
host — here, that host is this physical box with a GPU and root-adjacent
k0s access, not a disposable GH-hosted VM.

Checked PR history (`gh pr list --repo elasticdotventures/_b00t_ --state all
--limit 20 --json author`): every one of the last 20 PRs was authored by
`elasticdotventures` — there is no external-contributor traffic today, so
practical risk is currently low. **That can change at any time on a public
repo without any config change on our side**, so this PR does not rely on
"no one else sends PRs" as a control.

Mitigation implemented: `.github/workflows/ci-gpu.yml` is gated on
`workflow_dispatch` only — it is **not** added to the existing
`on: pull_request` / `on: push` triggers in `.github/workflows/ci.yml`.
A workflow_dispatch-only job cannot be triggered by a PR (fork or
same-repo) at all; it requires a repo collaborator to manually run it from
the Actions tab or `gh workflow run`. This is the conservative default —
a human should explicitly decide before this ever fires automatically on
PR traffic, and before its trigger scope is widened. See the follow-on
issue referenced from the PR description for that decision.

## What NOT to copy: the stale prior art

`k8s.🚢/gh-runner/deployment.yaml` is an existing, unrelated, non-ARC
manifest — a raw `Deployment` running `myoung34/github-runner:latest`
against a *different* repo (`PromptExecution/rust-docs-mcp-b00t`), requiring
a human to manually refresh its registration token every hour
(`gh api -X POST .../actions/runners/registration-token`). It is left
untouched by this PR. It is exactly the manual-token-refresh problem ARC
exists to solve — do not extend or resurrect that pattern for this repo.

## Files here

| File | Purpose |
|---|---|
| `values.yaml` | Helm values for the `gha-runner-scale-set` chart (v0.14.2, matching the installed controller). GPU request/limit, NVMe-backed `CARGO_HOME`/work-dir hostPath cache, `runtimeClassName: nvidia`, `maxRunners: 1`. |
| `secret.example.yaml` | Documents the `gh-runner-gpu-creds` secret shape and the imperative `kubectl create secret` command. Not meant to be applied literally — see the CHANGEME placeholder. |
| `README.md` | This file. |

Companion workflow: `.github/workflows/ci-gpu.yml` (repo root, not in this
directory — GitHub only reads workflows from `.github/workflows/`).
