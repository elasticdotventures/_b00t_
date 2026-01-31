# Dagu Workflows for b00t

This directory contains Dagu DAG (Directed Acyclic Graph) workflows for testing and CI operations.

## Setup

1. Install Dagu:
   ```bash
   b00t-cli cli install dagu
   ```

2. Copy DAG files to Dagu config:
   ```bash
   mkdir -p ~/.config/dagu/dags
   cp .dagu/dags/*.yaml ~/.config/dagu/dags/
   ```

## Available Workflows

### test-simple.yaml
Quick sanity check workflow to verify Dagu is working correctly.
- Checks git status
- Verifies Rust toolchain
- Lists Cargo projects

**Usage:**
```bash
dagu start test-simple
```

### quick-check.yaml
Fast development workflow for local testing (recommended for daily use).
- Runs `cargo check --workspace`
- Builds b00t-cli and b00t-mcp binaries
- Tests version detection (dagu)
- Tests MCP registry sync
- Continues on test failures (non-blocking)

**Usage:**
```bash
dagu start quick-check
```

### b00t-cli-tests.yaml
Comprehensive test suite for the b00t-cli crate.
- cargo check
- cargo clippy (with warnings as errors)
- cargo test
- Release build
- Binary verification

**Usage:**
```bash
dagu start b00t-cli-tests
```

### full-workspace-tests.yaml
Complete workspace testing (all crates).
- Workspace check
- Workspace tests
- Workspace clippy
- Release builds
- Binary verification

**Usage:**
```bash
dagu start full-workspace-tests
```

## Dagu Web UI

Start the Dagu web interface to monitor and manage workflows:

```bash
dagu start-all
# Open http://localhost:8080 in your browser
```

## Environment Variables

Workflows use these environment variables:
- `PROJECT_ROOT=/home/brianh/.dotfiles` - Project root directory
- `CARGO_TERM_COLOR=always` - Colored cargo output
- `RUST_BACKTRACE=1` - Full backtraces on errors

## Tips

1. **Continue on Failure**: The `quick-check` workflow uses `continueOn.failure: true` for tests, allowing builds to proceed even if tests fail.

2. **Parallel Execution**: Dagu automatically runs independent steps in parallel when possible.

3. **Status Checking**:
   ```bash
   dagu status quick-check
   ```

4. **Re-running Failed Steps**: Dagu tracks which steps succeeded, making it easy to retry failed steps.

5. **Logs**: Check `~/.config/dagu/logs/` for detailed execution logs.

## Converting GitHub Actions

To convert GitHub Actions workflows to Dagu:

1. Map `jobs` to `steps`
2. Use `depends` for job dependencies
3. Set `dir` for working directories
4. Use `env` for environment variables
5. Add `continueOn.failure` for non-blocking steps

Example:
```yaml
# GitHub Actions
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - name: Run tests
        run: cargo test
        working-directory: ./my-crate

# Dagu
steps:
  - name: test
    command: cargo test
    dir: ./my-crate
```
