# sccache — Rust compiler cache for b00t workspace

## Tribal knowledge

### cdylib kills cache hit rate

`crate-type = ["cdylib", "rlib"]` in Cargo.toml → sccache **cannot cache any compilation unit**
that produces a cdylib output. This drops the hit rate from ~90% (expected) to **0%**.

**Fix**: Default to `rlib` only. Python extension builds (maturin-action, b00t-py) inject
`--crate-type cdylib` via rustc flags automatically — no breakage.

**What didn't work**: `SCCACHE_CACHE_CUSTOM_CRATE_TYPES=cdylib` env var (sccache 0.16.0).

### Workspace: 67 non-cacheable calls from proc-macros

Proc-macro crates (`proc-macro = true` in Cargo.toml) are never cached by sccache.
This is a fundamental limitation — proc-macros execute at build time and sccache can't
dynamically cache their output.

Current workspace has ~10 proc-macro deps. If these become a bottleneck, consider
caching only the rlib outputs and using `--check` for fast iteration.

### CARGO_INCREMENTAL=0 is needed

sccache and incremental compilation conflict. Without `CARGO_INCREMENTAL=0`,
~64 compilations are marked non-cacheable. Set in `~/.cargo/config.toml`:

```toml
[build]
rustc-wrapper = "sccache"
incremental = false
```

### CI: sccache setup

```yaml
env:
  RUSTC_WRAPPER: sccache
  CARGO_INCREMENTAL: 0
steps:
  - uses: mozilla-actions/sccache-action@v0.0.7
```

Cache persists across CI runs via GitHub Actions cache backend.

### Dev branch 0.16% → unavoidable

Source churn on feature branches means the same file changes frequently,
invalidating cached outputs. sccache is most effective on CI/main where
source is stable across PRs.

## Installation

```bash
b00t-cli install sccache
# or: cargo install sccache --locked
```

Datum: `_b00t_/sccache.cli.toml`
