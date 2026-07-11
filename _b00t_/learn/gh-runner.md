# gh-runner — b00t Self-Hosted GitHub Actions Runner

**b00t gh-runner** manages self-hosted GitHub Actions runners via the b00t hive CMDB.
Runners are installed as systemd user services following the same pattern as
opencode and pi coding agents.

## References
- [actions/runner](https://github.com/actions/runner) — official GitHub self-hosted runner
- [GitHub docs: self-hosted runners](https://docs.github.com/en/actions/hosting-your-own-runners)
- datum: `~/.b00t/_b00t_/gh-runner.cli.toml`
- hive profile: `~/.b00t/_b00t_/gh-runner.hive.toml`

## Prerequisites

- `gh` CLI authenticated: `gh auth status`
- systemd user services enabled, lingering on for runner user
- 10+ GB free disk in workdir partition
- (optional) `wrkflw` for local workflow validation

## Commands

```bash
# Install and register a runner
b00t gh-runner install \
  --repo app4dog/middleware \
  --labels "self-hosted,app4dog,linux,x64" \
  --workdir /var/lib/gh-runner/middleware \
  --user gh-runner \
  --ephemeral

# Check runner status (systemd + GitHub API)
b00t gh-runner status --repo app4dog/middleware

# View logs
b00t gh-runner logs --repo app4dog/middleware
b00t gh-runner logs --repo app4dog/middleware --follow

# Diagnose health
b00t gh-runner doctor --repo app4dog/middleware

# Deregister and remove
b00t gh-runner deregister --repo app4dog/middleware --remove-service
```

## Architecture

```
gh-runner.cli.toml (abstract interfaces)
    ↓
gh-runner.hive.toml (template, {{placeholders}})
    ↓
b00t hive activate → ~/.config/b00t/profiles/gh-runner-{slug}.toml (concrete)
    ↓
generate_systemd_unit() → ~/.config/systemd/user/b00t@gh-runner-{slug}.service
    ↓
systemctl --user start → runner process → polls GitHub job queue
```

## Install Sequence

1. Validate `gh auth status` and repo admin access via `gh api`
2. Create `gh-runner` user if needed, create workdir
3. Download `actions/runner` release via `gh release download`
4. Fetch 1h registration token via `gh api repos/{o}/{r}/actions/runners/registration-token`
5. Generate concrete hive profile from template (substitute placeholders)
6. `b00t hive activate --profile gh-runner-{slug}` → systemd unit + start
7. Runner `config.sh` self-registers, receives permanent `.credentials`
8. Delete registration token file

## Token Lifecycle

- Registration token: 1h TTL, fetched via `gh api`, consumed by `config.sh`, then deleted
- Removal token: 1h TTL, fetched at deregister time
- Permanent OAuth: stored in `{workdir}/.credentials` (0600), managed by runner

## Service Management

```bash
# Via b00t hive
b00t hive status                     # shows all services including gh-runner
b00t hive activate {profile}         # start (re)activation

# Direct systemd
systemctl --user status b00t@gh-runner-{slug}.service
systemctl --user restart b00t@gh-runner-{slug}.service
journalctl --user -u b00t@gh-runner-{slug}.service -f
```

## Ephemeral Runners

`--ephemeral` flag sets `./config.sh --ephemeral` — the runner accepts one job,
executes it, then exits. systemd restarts it (if `Restart=on-failure`).
Use `--once` for true one-shot (deregisters after one job).

```yaml
# PR jobs → ephemeral runner (untrusted code isolation)
runs-on: [self-hosted, app4dog, linux, x64, pr]

# Main push → persistent runner (trusted, low latency)
runs-on: [self-hosted, app4dog, linux, x64, trusted]
```

## Security

- Runner runs as dedicated `gh-runner` user (no sudo, no shell login)
- Repo-scoped registration (not org-wide) for least privilege
- Registration tokens never committed to git
- Workdir `.credentials*` files have 0600 permissions
- Ephemeral mode for PR jobs from external contributors
- `RUNNER_ALLOW_RUNASROOT=0` enforced in systemd unit
- Favor rootless Podman over Docker for containerized job steps

## wrkflw Integration

```bash
# Local gate before push (no Docker needed)
wrkflw validate .github/workflows/ci.yml
wrkflw run --job test --runtime emulation .github/workflows/ci.yml

# Full CI: push → GitHub dispatches to self-hosted runner
git push origin main
```

## Troubleshooting

| Symptom | Check |
|---------|-------|
| Runner offline | `b00t gh-runner doctor --repo X/Y` |
| Token expired | `b00t gh-runner deregister && b00t gh-runner install ...` |
| Disk full | `df -h {workdir}` — `_work/` may need manual purge |
| Service crashed | `journalctl --user -u b00t@gh-runner-{slug}.service -n 50` |
| Docker unavailable | Check user has `docker` group or use rootless Podman |
| Network blocked | `curl -I https://pipelines.actions.githubusercontent.com` |
