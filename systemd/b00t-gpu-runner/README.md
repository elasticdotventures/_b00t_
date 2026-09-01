# b00t-gpu-runner (bare-metal / systemd GitHub Actions runner)

Reviewable manifest (not applied/installed/registered) for offering this host's
NVIDIA RTX 3090 to GitHub Actions CI as a **bare-metal, systemd-managed**
self-hosted runner — using GitHub's own officially-supported `actions-runner`
release + `config.sh` / `svc.sh` flow, the same way GitHub's docs describe
installing a self-hosted runner "as a service."

This is the **alternative** to PR #1232's k8s/ARC-based approach
(`k8s/gh-runner-gpu/`), not a replacement for it. Both target #1226 (CI
slowness — 30-45+ min `cargo test --workspace` on cold `ubuntu-latest`); pick
whichever fits how this box is meant to be used.

## systemd (this PR) vs. k8s/ARC (#1232) — which to use

| | **systemd bare-metal (here)** | **k8s/ARC (#1232)** |
|---|---|---|
| GPU access | Direct — host driver/CUDA, no device plugin, no time-slicing config | Shared across pods via NVIDIA device plugin + time-slicing (`nvidia.com/gpu: 3` allocatable on one physical 3090) |
| Concurrency | One job at a time (whole GPU, whole box, for the job's duration) | Multiple concurrent runner pods can share the GPU |
| Scaling | Fixed: exactly the runners you `svc.sh install` | Autoscaling `AutoscalingRunnerSet` — runner pods come and go with queued jobs |
| Moving parts | GitHub's own `actions-runner` tarball + systemd unit it generates | ARC controller, Helm chart, RuntimeClass, device plugin, k0s |
| Isolation between jobs | None beyond whatever the job itself cleans up — same host, same systemd user, run to run | Each job gets a fresh ephemeral pod |
| Best for | Simple, single-job-at-a-time GPU CI (e.g. one `cargo test` run at a time) where the whole box is fine to dedicate for a few minutes | Multiple concurrent workflows/PRs wanting GPU access, or wanting the runner pool to scale to zero when idle |
| Prerequisite blocking both | No GitHub token exists yet for either — see below | Same — see `k8s/gh-runner-gpu/README.md` |

Use **this** (systemd) if you want the simplest possible path to "a GPU job
runs on this box" and don't need more than one GPU CI job running at once.
Use **#1232's k8s/ARC** approach if you want the runner pool to scale, or to
share the GPU across concurrent jobs.

Nothing prevents running both — they're independent registrations (different
runner names/labels) against the same repo.

## Prerequisites

**Nothing in this directory has been applied.** No runner is installed, no
systemd unit exists yet, and no registration token/secret has been created.
This is the same missing-prerequisite state as #1232.

You need one of:

- A **runner registration token** (short-lived, ~1 hour TTL, generated
  specifically for registering a new self-hosted runner) — obtained via
  either:
  - `gh api repos/OWNER/REPO/actions/runners/registration-token -X POST --jq .token`, or
  - the repo's **Settings → Actions → Runners → New self-hosted runner** page
    (GitHub generates and displays the `config.sh` command with the token
    pre-filled).

This is **not** a personal access token (PAT). A registration token is a
narrow-purpose, short-lived credential minted only for the runner-registration
handshake (`config.sh` exchanges it once during setup for the runner's own
longer-lived credentials, which it then stores locally and refreshes itself —
you don't manage those). A PAT is a general, longer-lived credential scoped to
a user's API access and is the wrong kind of token for this flow; GitHub's own
runner registration UI issues (and expects) the short-lived token, not a PAT.

## Setup flow (official `actions-runner` pattern)

This mirrors exactly what GitHub's own "Add self-hosted runner" instructions
generate — `setup-runner.sh` in this directory automates these same steps:

1. Download the pinned `actions-runner-linux-x64-<version>.tar.gz` release
   asset from `actions/runner` and verify its sha256.
2. Extract it to an install directory (e.g. `/opt/gh-runner-gpu`).
3. Run the runner's own `./config.sh --url <repo-url> --token <reg-token> \
   --name <runner-name> --labels <labels> --unattended`.
4. Run the runner's own `sudo ./svc.sh install` — **this is what actually
   creates the systemd unit** (typically
   `/etc/systemd/system/actions.runner.<org>-<repo>.<name>.service`), running
   as a dedicated non-root service user. We deliberately do not hand-write a
   unit file here: `svc.sh` is the officially-supported installer, and a
   hand-written unit risks drifting from what a future runner self-update
   expects (working directory layout, env file location, service user, etc).
5. Run `sudo ./svc.sh start` to start the service.

See `setup-runner.sh` for the scripted version of steps 1-3 (config only) and
`environment.example` for the env vars it reads. Steps 4-5 (`svc.sh
install`/`start`) require root and are intentionally left as explicit manual
commands — see the script's final printed instructions.

## GPU access

No device plugin, no `RuntimeClass`, no time-slicing config is needed here —
none of that machinery exists to share one physical GPU across many
scheduled pods. This runner is a normal OS process under systemd; it inherits
this host's NVIDIA driver and CUDA install directly, and the GPU is entirely
available to whatever job that runner process executes, for the duration of
that job. Confirm the driver is visible before registering anything real:

```
nvidia-smi --query-gpu=name,driver_version --format=csv,noheader
```

(Verified present on this host during this session: `NVIDIA GeForce RTX 3090`,
driver `580.167.08`.)

## Security posture (same as #1232)

`elasticdotventures/_b00t_` is a **public** repo. A self-hosted runner that
fires on `pull_request` (including from forks) is a known attack vector — the
workflow gets arbitrary code execution directly on this host, not a
disposable GH-hosted VM. This is exactly the same risk #1232 already
identified and the same tradeoff — do not re-litigate it in a second issue.

Accordingly, the companion workflow in this PR
(`.github/workflows/ci-gpu-systemd.yml`) is **`workflow_dispatch`-only**: it
is not added to `ci.yml`'s `pull_request`/`push` triggers, and cannot fire
from a PR (fork or same-repo).

The decision on if/when to widen trigger scope for *either* the systemd or
the k8s/ARC GPU runner is already tracked centrally in **#1233** — see that
issue for the options under consideration. This PR does not open a second,
duplicate follow-on issue.

## What this PR does NOT do

- Does not download, extract, or run anything from `setup-runner.sh`.
- Does not create a registration token or any secret.
- Does not run `config.sh` or `svc.sh install`/`start`.
- Does not create or touch any systemd unit file.
- Does not change `.github/workflows/ci.yml`'s existing triggers.
- Does not replace or conflict with #1232's k8s/ARC manifest.

## References

- Relates to #1226 (CI slowness).
- Trigger-scope decision tracked in #1233 (shared with #1232).
- Companion PR: #1232 (k8s/ARC alternative — read that PR's README for the
  scaling/shared-GPU approach).
- Upstream: https://github.com/actions/runner (official release + `config.sh`/`svc.sh`).
