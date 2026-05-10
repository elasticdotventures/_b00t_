# Issue #395 — Codebase-Memory-MCP Integration: Upgrade + Gap Analysis

## Current State

- **Vendor path**: `vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/`
- **Upstream**: https://github.com/DeusData/codebase-memory-mcp
- **Fork**: https://github.com/PromptExecution/codebase-memory-mcp-b00t-ir0n-ledg3rr
- **Previous pin**: `5aadb96` (b00t integration baseline)
- **Updated pin**: `31ae79d` (feat/upgrade-to-v0.6.1 — latest upstream + b00t scripts)

## Upgrade Performed

Advanced submodule from `5aadb96` → `31ae79d`. Changes pulled in:

| Category | Details |
|----------|---------|
| **Critical fixes** | search_graph default cap 500K→200 (#231), OOM prevention (#206), thread-safety (#207), ES import extraction (#224) |
| **Features** | Graph temporal properties on nodes/edges (#257), Nix flake (#265), safe memory allocators |
| **Infra** | 7 CI/dependency bumps, AUR package instructions, release automation |
| **b00t integration** | Preserved: `_b00t_/datums/`, `hooks/`, `scripts/`, `sdd/`, justfile, embedded UI assets |

## MCP Interface Verification

- **Binary**: `build/c/codebase-memory-mcp` — 252MB, exists
- **MCP config**: `_b00t_/codebase-memory.mcp.toml` — stdio transport, correct path
- **Tools exposed**: Already available as `mcp_codebase_memory_*` MCP tools in the session
- **Submodule**: Updated and committed

## Gap Analysis & Good-Faith Critique

### Gap 1: No .gitmodules entry
**Severity**: Medium
**Issue**: The submodule exists as a directory + gitlink in the parent repo's tree, but has no entry in `.gitmodules`. This means `git submodule init`/`update` doesn't work — new clones of b00t won't populate it.
**Plan**: Add `.gitmodules` entry for `vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr` pointing to `git@github.com:PromptExecution/codebase-memory-mcp-b00t-ir0n-ledg3rr.git`, branch `feat/upgrade-to-v0.6.1`.

### Gap 2: Integration scripts not wired into b00t ecosystem
**Severity**: Low
**Issue**: The `_b00t_/datums/` directory has 5 documentation datums (b00t-install-hermes.md, just-mcp-trait.md, etc.) but these are `.md` files, not `.tomllmd` datum files. They're readable but not discoverable by b00t's ontology or grok systems.
**Plan**: Convert key integration docs to `.tomllmd` format with `[b00t]` header blocks so they're indexed by `b00t ontology query` and `b00t grok ask`.

### Gap 3: No automatic MCP registration
**Severity**: Medium
**Issue**: The `_b00t_/codebase-memory.mcp.toml` config exists but must be manually installed via `b00t mcp install codebase-memory claudecode` or similar. There's no bootstrap hook that auto-registers it.
**Plan**: Add a reference to the MCP config in `_b00t_/bootstrap.toml` (or equivalent auto-load mechanism) so the codebase-memory server is auto-registered when b00t initializes.

### Gap 4: Test coverage for integrated path
**Severity**: Medium
**Issue**: No tests verify the end-to-end flow: codebase-memory MCP tool called from b00t context → response returned correctly. The `_b00t_/scripts/datum-lint.py` validates datum files but doesn't test MCP tool integration.
**Plan**: Add a smoke test in `tests/` or as a `just mcp-test-codebase-memory` command that calls `search_graph` and `list_projects` to verify the server responds correctly.

### Gap 5: feat/upgrade-to-v0.6.1 branch not merged to main
**Severity**: Low
**Issue**: All the upstream fixes and b00t integration are on the `feat/upgrade-to-v0.6.1` branch. The `main` branch of the fork doesn't have these changes. If someone checks out `main`, they get the stale upstream version.
**Plan**: Merge `feat/upgrade-to-v0.6.1` into `main` in the fork repo and add `.gitmodules` pointing to main.

## Recommendations (Priority Order)

1. **Add `.gitmodules` entry** — blocks clean cloning
2. **Merge feat/upgrade-to-v0.6.1 → main** in fork — keeps main canonical
3. **Add smoke test** — `just mcp-test-codebase-memory`
4. **Convert datums** `.md` → `.tomllmd`
5. **Auto-register** in bootstrap
