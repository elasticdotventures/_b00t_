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
# OR
brew install wrkflw           # macOS/Linux Homebrew
```

## Core Commands

```bash
wrkflw                                    # Launch interactive TUI
wrkflw list                               # List all discovered workflows
wrkflw validate <workflow.yml>            # YAML + schema validation only
wrkflw validate --verbose <workflow.yml>  # verbose validation output
wrkflw validate .gitlab-ci.yml --gitlab   # GitLab CI support
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

## b00t CI/Testing Patterns

### Validate before commit (pre-push gate)

```bash
wrkflw validate .github/workflows/ci.yml
# exit 0 = clean; exit 1 = schema/syntax error; exit 2 = file not found
```

### Run full pipeline locally

```bash
wrkflw run --runtime secure-emulation .github/workflows/ci.yml
```

### Run single stage (fastest feedback loop)

```bash
wrkflw run --job stage-1-rhai-parser-tests --runtime secure-emulation .github/workflows/wrkflw-docgen.yml
```

### Diff-aware CI (only run jobs touching changed files)

```bash
wrkflw run --diff --event push .github/workflows/ci.yml
```

### Just recipes (l3dg3rr pattern)

```just
# Run full docgen visualization pipeline
wrkflw-docgen-test emulation="secure-emulation":
    @if ! command -v wrkflw >/dev/null 2>&1; then echo "error: wrkflw not found — run: cargo install wrkflw"; exit 1; fi
    wrkflw run --runtime {{emulation}} .github/workflows/wrkflw-docgen.yml

# Validate YAML only (fast gate)
wrkflw-validate:
    wrkflw validate --verbose .github/workflows/wrkflw-docgen.yml

# Run one job
wrkflw-job job="stage-1-rhai-parser-tests" emulation="secure-emulation":
    wrkflw run --job "{{job}}" --runtime {{emulation}} .github/workflows/wrkflw-docgen.yml

# Open TUI
wrkflw-tui:
    wrkflw tui

# Full gate: validate then run
wrkflw-full-test emulation="secure-emulation":
    wrkflw validate .github/workflows/wrkflw-docgen.yml
    wrkflw run --runtime {{emulation}} .github/workflows/wrkflw-docgen.yml
```

### Test harness script pattern

```bash
# scripts/wrkflw_test.sh modes:
./scripts/wrkflw_test.sh              # full pipeline (all stages)
./scripts/wrkflw_test.sh --validate  # YAML validation only
./scripts/wrkflw_test.sh --stage S5  # single stage (S1–S9)
./scripts/wrkflw_test.sh --list      # list stage names
```

## Writing wrkflw-Compatible Workflows

### Required: `actions/checkout@v4` in every job

```yaml
jobs:
  my-job:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4   # REQUIRED — wrkflw mounts empty dirs
      - name: Run tests
        run: cargo test
```

### Avoid composite action references under emulation

```yaml
# ❌ fails under secure-emulation
steps:
  - uses: dtolnay/rust-toolchain@stable

# ✓ use inline instead
steps:
  - name: Install Rust
    run: rustup update stable
```

### Expression support limitations

```yaml
# ❌ unsupported: ${{ }} in continue-on-error
continue-on-error: ${{ matrix.allow_failure }}

# ✓ use literal
continue-on-error: true

# ❌ unsupported: needs.<id>.result in summary jobs
if: ${{ needs.stage-1.result == 'success' }}

# ✓ use sequential job chaining with `needs:` only
needs: [stage-1-rhai-parser-tests]
```

### sccache across stages (secure-emulation)

```yaml
env:
  RUSTC_WRAPPER: sccache          # shared daemon across all emulation stages
  SCCACHE_DIR: /tmp/sccache       # 🤓 daemon is a host process — persists across jobs
  CARGO_TERM_COLOR: always
```

## Key Limitations (b00t tribal knowledge)

- `curl | sh` patterns blocked by `secure-emulation` — use `emulation` runtime instead
- Each stage gets a fresh sandbox — no shared `target/` cache between jobs
- Full multi-stage Rust pipeline: 20-60+ minutes due to recompilation per stage
- `needs.<id>.result` and `toJSON(needs)` expressions not supported in summary jobs
- `${{ runner.temp }}` expression in `env:` may not evaluate — fallback to `/tmp`

## Features Supported

- Expression evaluation: `${{ ... }}` incl. `toJSON`, `fromJSON`, `contains`, `startsWith`
- Artifacts & inter-job outputs (`needs.<id>.outputs.*`)
- Matrix builds (`include`, `exclude`, `max-parallel`, `fail-fast`)
- Secrets management: env, file, Vault, AWS, Azure, GCP (AES-256-GCM masked)
- Reusable workflows: local + remote `owner/repo/path@ref`
- Container actions, JavaScript actions, composite actions
- `GITHUB_OUTPUT`, `GITHUB_ENV`, `GITHUB_PATH`, `GITHUB_STEP_SUMMARY` emulation
- Watch mode with trigger-aware re-execution
- Interactive TUI: Workflows / Execution / DAG / Logs / Trigger / Secrets / Help tabs

## 9-Stage l3dg3rr Docgen Pipeline (reference)

| Stage | ID | What it tests |
|-------|----|---------------|
| S1 | `stage-1-rhai-parser-tests` | mdbook-rhai-mermaid unit tests |
| S2 | `stage-2-iso-lint` | ledger-core iso lint tests |
| S3 | `stage-3-viz-tests` | LayoutSolver / to_mermaid / to_html |
| S4 | `stage-4-legal-z3` | Z3 legal solver integration |
| S5 | `stage-5-docgen-build` | mdBook + rhai→mermaid injection |
| S6 | `stage-6-kasuari-constraints` | Kasuari constraint solver |
| S7 | `stage-7-iso-objects` | HasVisualization impls lint |
| S8 | `stage-8-live-editor-js` | browser live-editor JS tests |
| S9 | `stage-9-xero-mcp` | Xero MCP smoke (build + unit) |

## Agent Role Coverage

| Role | Primary wrkflw use |
|------|-------------------|
| developer | `wrkflw validate` pre-push gate; `--job` for fast iteration |
| orchestrator | `wrkflw run --diff` for change-set CI evaluation |
| analyst | full pipeline runs for coverage audits |

## b00t Crew Quick Ref

```bash
# Before pushing: always validate
wrkflw validate .github/workflows/*.yml

# Fast TDD loop: one job at a time
wrkflw run --job <job-id> --runtime emulation .github/workflows/ci.yml

# Full gate before PR
just wrkflw-full-test

# Debug with TUI
wrkflw tui
```

---
🤓 wrkflw surfaces pre-existing compile errors not caught by `cargo check` alone — run it early on new crate integrations.
🤓 Use `sccache` with `RUSTC_WRAPPER=sccache` to share compile cache across the daemon lifetime in emulation mode.
🤓 Secure emulation runs steps as sandboxed host processes — no Docker socket needed, but `curl | sh` is blocked for safety.
