# Deprecated Bash Scripts — Zellij Interaction

These bash scripts have been superseded by the `b00t zellij` Rust binary.
They remain functional but are no longer the canonical implementation.
All callers should redirect through `_b00t_/scripts/zellij-rust-wrapper.sh`
or use `b00t zellij` directly.

## Script → Replacement Map

| # | Bash Script | Replacement | Notes |
|---|------------|-------------|-------|
| 1 | `init-zellij-agent.sh` | `b00t zellij detect --json` | Detection + KVCache init. Exit 0=inside Zellij, 1=outside. |
| 2 | `zellij-run-interactive.sh` | `b00t zellij {menu,confirm,input}` | Interactive runner with TTY-safe floating panes. |
| 3 | `zellij-user-interaction.sh` | `b00t zellij {subagent,wizard}` | Modal dialogs — now rendered by Rust, no bash templating. |
| 4 | `zellij-kv-cache.sh` | `b00t-c0re-lib::KvStore` (Rust API) | JSON KVCache with atomic file locking (no shell injection). |
| 5 | `zellij-modal.sh` | `b00t-cli::run_in_zellij_floating()` | Confirm dialog rendered by Rust via `zellij run --floating`. |
| 6 | `gate-init-agent.sh` | `ZellijGate` auto-detection in `b00t-c0re-gov` | Gate activates automatically when Zellij session is detected. |

## Quick Reference

```
# Detection (was init-zellij-agent.sh)
b00t zellij detect          # prints JSON, exit 0 = inside

# Confirm dialog (was zellij-run-interactive.sh confirm / zellij-modal.sh)
b00t zellij confirm --title "Deploy?" --prompt "Proceed?"

# Text input (was zellij-run-interactive.sh input)
b00t zellij input --prompt "Enter branch:" --default "main"

# fzf menu (was zellij-run-interactive.sh fzf-menu)
b00t zellij menu --title "Select action" --items '[{"key":"build","label":"Build"}]'

# Sub-agent report (was zellij-user-interaction.sh subagent)
b00t zellij subagent --title "worker-1" --content "Build passed"

# Multi-step wizard (was zellij-user-interaction.sh wizard)
b00t zellij wizard --title "Setup" --file wizard.toml
```

## Migration Path

1. New callers: use `b00t zellij <subcommand>` directly.
2. Existing callers: point at `_b00t_/scripts/zellij-rust-wrapper.sh` (same positional args).
3. The wrapper auto-detects Zellij and translates old args to the new CLI.

## Phase

Phase 5 of [Rust/WASM Rewrite Plan](_b00t_/plans/2026-06-20-zellij-rust-wasm-rewrite.md).
