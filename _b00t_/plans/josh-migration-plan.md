# Josh Migration Plan — b00t Hive (c0re → b00ty-verse)

## Current State

- 30 submodules in this node (c0re)
- 30+ forks across PromptExecution + elasticdotventures (b00ty-verse)
- Total: 60+ repos needing constant upstream sync
- Current: git submodules — dirty state, 3-PR dance, contributor friction

## Josh Architecture for b00t

### Phase 1: Josh Proxy (c0re node)

```
┌─────────────────────────────────────────────────┐
│                b00t mono-repo                    │
│  vendor/rust-docs/  vendor/ledgrrr/  vendor/*/   │
│       ↓ josh filter          ↓ josh filter       │
│       /:vendor/rust-docs     /:vendor/ledgrrr    │
└─────────────────────────────────────────────────┘
              ↓ josh-proxy (port 4242)
  github.com/PromptExecution/rust-docs-mcp-server  ← served as independent
  github.com/PromptExecution/ledgrrr               ← served as independent
```

### Phase 2: josh-sync (bidirectional)

```
  upstream/main ──pull──→ b00t/vendor/X (josh-sync auto-PR)
  b00t/vendor/X  ──push─→ fork/main (atomic cross-boundary PR)
```

### Phase 3: Workspace splitting

Each hive node gets its own workspace filter:
- `/workspace=c0re` → b00t-c0re-lib, b00t-cli, b00t-admin
- `/workspace=ledgrrr` → vendor/ledgrrr only
- `/workspace=b00ty-verse` → all 60+ vendors

## Implementation Steps

### Step 1: Install Josh
```bash
cargo install josh-proxy josh-cli
```

### Step 2: Create workspace TOML
```toml
# josh-workspace.toml
[workspace.b00t]
path = "/"
filters = ["::c0re/", "::vendor/"]
```

### Step 3: Migrate one submodule (rust-docs first)
```bash
# Convert submodule to subtree (vendor code lives in b00t)
git submodule deinit vendor/rust-docs-mcp-server
git rm vendor/rust-docs-mcp-server
git commit -m "de-submodule: prep for Josh migration"

# Import vendor code with history via josh
josh filter --import vendor/rust-docs-mcp-server \
  --from https://github.com/PromptExecution/rust-docs-mcp-server

# Register workspace
josh workspace add --name rust-docs --path vendor/rust-docs-mcp-server
```

### Step 4: Set up josh-sync GitHub Actions
```yaml
# .github/workflows/josh-sync-rust-docs.yml
on:
  schedule: [{cron: '0 */6 * * *'}]  # every 6 hours
jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: rust-lang/josh-sync@v1
        with:
          direction: pull
          subproject: vendor/rust-docs-mcp-server
          upstream: PromptExecution/rust-docs-mcp-server
```

### Step 5: Repeat for remaining 59 repos
- Batch 1 (5 repos): rust-docs, ledgrrr, irontology-mcp, embed-anything, tomllm
- Batch 2 (10 repos): codebase-memory, hermes-agent, Proxy-Pointer-RAG, arc-kit-au, etc.
- Batch 3 (15 repos): elasticdotventures forks (zellij-gate, gemm, moltis-b00t, etc.)
- Batch 4: remaining repos

## Migration DoD (Definition of Done)

- [ ] Josh proxy running on b00t infrastructure (port 4242)
- [ ] All 60+ repos filterable via workspace paths
- [ ] josh-sync GitHub Actions for bidirectional sync
- [ ] Cross-boundary PRs atomic (1 PR, not 3)
- [ ] `git clone` without --recursive works
- [ ] No more submodule dirty state on branch switch
- [ ] joshmodule.toml in each vendor directory

## Risks

- Josh proxy requires dedicated infrastructure (Rust binary, HTTP server)
- josh-sync can create merge conflicts that need manual resolution
- Migration is one-way — can't easily go back to submodules
- All hive nodes must agree on workspace structure
- This node only has c0re — full b00ty-verse migration requires coordination

## Decision

**Proceed with Phase 1 (Josh proxy + 1 subproject) as proof of concept.**
Target: migrate `vendor/rust-docs-mcp-server` first.
If successful, roll out to remaining 59 repos in batches.
