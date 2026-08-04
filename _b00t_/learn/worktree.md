---
Fresh git worktrees do NOT have submodules initialized. 3 crates require submodule
path-dependencies to resolve for cargo check/test: vendor/ledgrrr (b00t-reflect-types),
vendor/runpod-sdk, and vendor/embed-anything-b00t. Any fresh worktree hits 'failed to
load manifest ... No such file or directory' on first cargo invocation.

Fix: immediately after `git worktree add <path> <branch>`, run:
```
git submodule update --init vendor/ledgrrr vendor/runpod-sdk vendor/embed-anything-b00t
```
before any cargo command. Do NOT `--recursive` — that pulls the full nested submodule
history of every vendored fork (~16GB observed in one session), nearly all of it unused
by this workspace.

After checking out embed-anything-b00t, verify it's at rev ee16a22 (v0.7.2, candle-core
0.11.0), not an older rev — the root repo's own recorded submodule pointer has drifted to
a stale rev (bed9e6ea, pre-dating the real upstream v0.7.2 rebase) more than once. If
cargo reports two versions of candle_core in the dep tree, check this pointer first.

## Where to put the worktree (recorded 2026-08-04)

- Real disk only — NEVER `/tmp`. tmpfs is RAM-backed and shared across every concurrent
  session on the host; it has hit 0 bytes free mid-build from unrelated sessions' churn,
  with no warning until a write fails.
- `~/.dotfiles` itself is a **bare** repo (`core.bare=true`) with legacy stray working
  files sitting in it — NOT a real working tree. Never edit files or run cargo there
  directly; a concurrent session checking out a different branch there can silently mix
  its state into your uncommitted edits. Always `git worktree add <path> <branch>` first.
- Suggested location: `~/scratch/<slug>` or `~/.b00t/.claude/worktrees/<slug>` (create the
  parent dir first — it's the documented convention but isn't always pre-created).
- `export CARGO_TARGET_DIR=$HOME/.cache/b00t-cargo-target` before building, so every
  worktree of this repo reuses already-compiled dependency artifacts instead of each
  paying the ~7-13GB cold-build cost. This MUST stay a per-shell env var, not a
  checked-in `.cargo/config.toml` — CI runs this repo as a different user with no
  `$HOME/.cache` to write to, and a committed absolute-path `target-dir` breaks its
  build with `Permission denied` (hit in elasticdotventures/_b00t_#964).
- `git worktree remove <path>` (and `git worktree prune` if removed by hand) when the
  task is done — don't leave throwaway worktrees accumulating on disk. The shared
  target-dir survives worktree removal by design; no need to `cargo clean` it.
<!-- salvaged:topic_overflow -->
