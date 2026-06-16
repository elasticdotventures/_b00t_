# b00t grit patterns

GritQL patterns for b00t ecosystem enforcement.

## patterns

| pattern | level | language | purpose |
|---|---|---|---|
| `b00t_sandbox_block` | error | bash | block destructive commands (rm -rf /, dd, mkfs, fork bombs, shutdown) |
| `b00t_sandbox_warn` | warn | bash | warn on guarded commands (pip→uv, docker→podman, hf-cli→hf) |
| `b00t_no_raw_secrets` | error | bash | detect hardcoded API keys/tokens/passwords in env exports |
| `b00t_overlay_scope` | warn | rust | flag overlay.toml references outside project module |

## usage

```bash
# install grit CLI
curl -fsSL https://docs.grit.io/install | bash

# run all patterns as diagnostics
grit check

# apply a specific pattern (auto-fix)
grit apply b00t_sandbox_warn

# list available patterns
grit patterns
```

## user-level patterns

Node-local patterns live in `~/.grit/patterns/` and are merged with
repo patterns. Use these for per-node rules without modifying the repo:

```bash
mkdir -p ~/.grit/patterns
# add node-specific patterns here
```

## CI integration

```yaml
# .github/workflows/grit.yml
- name: grit check
  run: grit check --level error
```

## design

These patterns serve triple duty in the b00t ecosystem:

1. **CI gating** — `grit check --level error` blocks PRs with violations
2. **Runtime sandboxing** — `b00t exec` calls grit to analyze commands before execution
3. **Enclave validation** — `b00t project commit` validates staged files before committing

Pattern language note: GritQL does not natively parse TOML. Overlay datum
validation (secret detection in `.overlay.toml`) is handled by the b00t CLI
at commit time via `b00t project commit`, not by grit patterns. These patterns
focus on languages grit CAN parse: bash, rust, json, yaml.
