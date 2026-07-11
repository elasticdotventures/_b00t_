# gh-runner — b00t Self-Hosted GitHub Actions Runner

**b00t gh-runner** manages self-hosted GitHub Actions runners via podman kube play.
Runners are deployed as rootless podman pods using Kubernetes-style YAML.

## References
- [actions/runner](https://github.com/actions/runner) — official GitHub self-hosted runner
- [ghcr.io/actions/actions-runner](https://github.com/actions/runner/pkgs/container/actions-runner) — official container image
- [GitHub docs: self-hosted runners](https://docs.github.com/en/actions/hosting-your-own-runners)
- datum: `~/.b00t/_b00t_/gh-runner.cli.toml`
- hive template: `~/.b00t/_b00t_/gh-runner.hive.toml`

## Prerequisites

- `gh` CLI authenticated: `gh auth status`
- `podman` rootless: `podman info`
- Docker socket: `ls -la /var/run/docker.sock`
- 10+ GB free disk in workdir partition
- (optional) `wrkflw` for local workflow validation

## Commands

```bash
# Install and register a runner
b00t gh-runner install \
  --repo app4dog/middleware \
  --labels "self-hosted,app4dog,linux,x64" \
  --workdir /var/lib/gh-runner/middleware \
  --ephemeral

# Check runner status (podman pod + GitHub API)
b00t gh-runner status --repo app4dog/middleware

# View logs (podman pod logs passthrough)
b00t gh-runner logs --repo app4dog/middleware
b00t gh-runner logs --repo app4dog/middleware --follow

# Diagnose health
b00t gh-runner doctor --repo app4dog/middleware

# Deregister and remove
b00t gh-runner deregister --repo app4dog/middleware --remove-workdir
```

## Architecture

```
gh-runner.cli.toml (abstract interfaces)
    ↓
gh-runner.hive.toml (template, {{placeholders}})
    ↓
b00t gh-runner install → {workdir}/gh-runner.yaml (concrete pod spec)
    ↓
podman kube play → gh-runner-{slug} pod → actions-runner container
    ↓
Container auto-registers → polls GitHub job queue via WebSocket
```

## Install Sequence

1. Validate `gh auth status`, `podman info`, and repo admin access
2. Create workdir: `mkdir -p {workdir}/_work`
3. Pull image: `podman pull ghcr.io/actions/actions-runner:latest`
4. Fetch 1h registration token via `gh api`
5. Generate podman kube YAML from template (substitute placeholders)
6. `podman kube play {workdir}/gh-runner.yaml`
7. Container auto-registers with GitHub, receives permanent credentials
8. Verify: `gh api repos/{o}/{r}/actions/runners`

## Token Lifecycle

- Registration token: 1h TTL, fetched via `gh api`, passed as `RUNNER_TOKEN` env var
- Permanent OAuth: stored inside container at `/runner/.credentials`
- Removal token: 1h TTL, fetched at deregister time

## Pod Management

```bash
# Pod lifecycle
podman kube play {workdir}/gh-runner.yaml        # deploy
podman pod ps --filter name=gh-runner-{slug}      # status
podman pod logs gh-runner-{slug}                  # logs
podman kube down {workdir}/gh-runner.yaml         # tear down

# Container access
podman exec -it gh-runner-{slug}-runner bash     # debug shell
podman inspect gh-runner-{slug}-runner            # inspect config
```

## Ephemeral Runners

`RUNNER_EPHEMERAL=true` env var causes the runner to accept one job then exit.
The pod's `restartPolicy: OnFailure` ensures it restarts for the next job.

```yaml
# PR jobs → ephemeral runner (untrusted code isolation)
runs-on: [self-hosted, app4dog, linux, x64, pr]

# Main push → persistent runner (trusted, low latency)
runs-on: [self-hosted, app4dog, linux, x64, trusted]
```

## Security

- Runner runs in rootless podman container (UID 1001, not host root)
- seccomp profile blocks ~300 syscalls
- `no new privileges`: privilege escalation blocked
- Repo-scoped registration (not org-wide) for least privilege
- Registration tokens never committed to git (in-memory only)
- Ephemeral mode for PR jobs from external contributors
- Docker socket mount for CI builds (use rootless podman socket where possible)

## wrkflw Integration

```bash
# Local gate before push (no Docker needed)
wrkflw validate .github/workflows/ci.yml
wrkflw run --job test --runtime emulation .github/workflows/ci.yml

# Full CI: push → GitHub dispatches to self-hosted runner pod
git push origin main
```

## Kubernetes Migration

The same YAML works with `kubectl apply` if k0s/k8s is available:

```bash
# podman (rootless, local)
podman kube play gh-runner.yaml

# k0s / kubectl (cluster)
kubectl apply -f gh-runner.yaml
```

No YAML changes needed — the pod spec is standard Kubernetes API v1.

## Troubleshooting

| Symptom | Check |
|---------|-------|
| Runner offline | `b00t gh-runner doctor --repo X/Y` |
| Token expired | `b00t gh-runner deregister && b00t gh-runner install ...` |
| Pod crashed | `podman pod logs gh-runner-{slug}` |
| Docker unavailable | Check docker socket: `podman exec gh-runner-{slug}-runner docker info` |
| Network blocked | `podman exec gh-runner-{slug}-runner curl -I https://github.com` |
| Image pull failed | `podman pull ghcr.io/actions/actions-runner:latest` |
| Permission denied | Check SELinux/AppArmor for hostPath volume mounts |
