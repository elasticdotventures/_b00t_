# wrkflw — Local GitHub Actions Runner (b00t Gospel)

**wrkflw** (v0.8.0) validates and executes GitHub Actions workflows locally before push.
No Docker required in emulation modes. Rust CLI — `cargo install wrkflw`.

## References
- [Repo: bahdotsh/wrkflw](https://github.com/bahdotsh/wrkflw)
- [crates.io](https://crates.io/crates/wrkflw)
- datum: `b00t grok status wrkflw`

## Install

```bash
cargo install wrkflw          # from crates.io
```

## Core Commands

```bash
wrkflw                                    # Launch interactive TUI
wrkflw list                               # List all discovered workflows
wrkflw validate <workflow.yml>            # YAML + schema validation only
wrkflw validate --verbose <workflow.yml>  # verbose validation output
wrkflw run <workflow.yml>                 # Run workflow (default: Docker runtime)
wrkflw run --runtime emulation <wf.yml>  # No containers — host processes
wrkflw run --runtime secure-emulation <wf.yml>  # Sandboxed host processes
wrkflw run --job <job-id> <wf.yml>       # Single job only
wrkflw run --diff --event push <wf.yml>  # Diff-aware: only changed-path jobs
wrkflw watch <wf.yml>                    # Auto-rerun on file changes
wrkflw trigger <name>                    # Remote trigger via GitHub API
wrkflw tui                               # Open TUI explicitly
```

## Runtime Modes

| Mode | Containers | Best for |
|------|------------|----------|
| `docker` (default) | Docker | Full isolation, production parity |
| `podman` | Podman | Rootless, security-conscious |
| `emulation` | None | Quick local testing, no Docker |
| `secure-emulation` | None (sandboxed) | Untrusted workflows, b00t default |

🤓 b00t projects default to `--runtime secure-emulation` — avoids Docker/Podman dependency.

## b00t Workflow: concurrent local + cloud

b00t CI workflows are structured with two jobs:
1. `local-build` (runs-on: `[self-hosted, sm3lly]`) — `cargo build --release`, fast, no QEMU
2. `docker-push` (runs-on: `ubuntu-latest`, needs: `local-build`) — multi-arch Docker, fires only after local passes

This eliminates "wasted electricity" — cloud QEMU ARM64 builds only start after local amd64 compile succeeds.

### Run the local gate before push

```bash
# Validate YAML schema only (instantaneous)
wrkflw validate .github/workflows/docker.yml

# Run just the local-build job (emulation — no containers needed)
wrkflw run --job local-build --runtime emulation .github/workflows/docker.yml

# Watch: auto-rerun on file changes during development
wrkflw watch --job local-build .github/workflows/docker.yml
```

### just targets (b00t root justfile)

```bash
just rust-docs-validate    # wrkflw validate
just rust-docs-local-build # wrkflw run --job local-build --runtime emulation
just rust-docs-watch       # wrkflw watch --job local-build (dev loop)
```

### Self-hosted runner on sm3lly (k8s)

```bash
# One-time RBAC + runner setup
kubectl apply -f _b00t_/k8s.🚢/gh-runner/deployment.yaml

# Create token secret (token expires 1h — refresh before deploy)
TOKEN=$(gh api -X POST repos/PromptExecution/rust-docs-mcp-b00t/actions/runners/registration-token --jq .token)
kubectl create secret generic gh-runner-token \
  --from-literal=RUNNER_TOKEN=$TOKEN \
  --from-literal=REPO_URL=https://github.com/PromptExecution/rust-docs-mcp-b00t \
  -n b00t-gh-runner --dry-run=client -o yaml | kubectl apply -f -

# Verify runner appears in GH
gh api repos/PromptExecution/rust-docs-mcp-b00t/actions/runners
```

Label: `[self-hosted, sm3lly, linux, x64]` — matched by `runs-on: [self-hosted, sm3lly]` in workflows.
