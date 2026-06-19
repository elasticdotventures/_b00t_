# Task Context: Fix b00t up workspace root fallback

Session ID: 2026-06-19-fix-b00t-up-fallback
Created: 2026-06-19T18:37:00Z
Status: completed

## Current Request
Fix `get_workspace_root()` in `b00t-cli/src/utils.rs` — when run from a non-git-repo directory (including $HOME), the fallback `"b00t"` is a brittle relative path. Change it to resolve `$HOME/.b00t` (the system-wide CMDB/soul directory).

## Context Files (Standards to Follow)
- .opencode/context/core/standards/code-quality.md
- .opencode/context/project-intelligence/technical-domain.md
- Cargo.toml (workspace lint config: pedantic clippy, unused_must_use warn)

## Reference Files (Source Material)
- b00t-cli/src/utils.rs (target for change)
- b00t-cli/src/commands/up.rs (consumer of get_workspace_root)
- b00t-cli/src/up_command.rs (the datum-up command path)

## Components
- Single change: `get_workspace_root()` fallback value

## Constraints
- Keep existing `_B00T_TEST_ROOT` env var override
- Keep `git rev-parse --show-toplevel` as primary resolution
- Use `std::env::var("HOME")` pattern (consistent with existing code in model_registry.rs, datum_store.rs, etc.)
- Fallback chain: test env → git → $HOME/.b00t → "."

## Exit Criteria
- [ ] `get_workspace_root()` fallback uses `$HOME/.b00t` instead of `"b00t"`
- [ ] `cargo build -p b00t-cli` succeeds
- [ ] Existing tests pass (`cargo test -p b00t-cli utils`)
