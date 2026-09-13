# CONOPS: `b00t is` as a k0mmand3r Verb — System-Normal in Agent-Coordination Syntax

> **Status**: Concept paper — not yet implemented.
> **Supersedes**: `CONOPS-system-normal.md` (which incorrectly says "superseded";
> the Phase 1 implementation shipped — `checklist.rs`, `is_cmd.rs` — but the
> CONOPS was never updated to reflect that, and it predates k0mmand3r entirely).
> **Depends on**: `b00t-cli/src/checklist.rs` (Phase 1), `b00t-cli/src/k0mmand3r/mod.rs` (typed commands), `b00t-cli/src/gates.rs` (GateSpec + Disposition).

## Problem

`b00t is system-normal` answers one question: "is the system in a known-good
state right now?" Phase 1 shipped a working `checklist.rs` + `is_cmd.rs` that
evaluate `.checklist.toml` files using `GateSpec` with 3-valued Disposition
(Satisfied/Violated/Unknown). But:

1. **No checklists exist yet** — `b00t is` prints "No checklists found."
2. **No agent-facing surface** — the CLI is human-interactive; agents in the
   k0mmand3r dispatch pipeline have no `/is` verb, no MCP tool, and no way to
   gate a dispatch or loop iteration on system-normal state.
3. **No persistence** — every evaluation is fire-and-forget; no history, no
   flap detection, no "same as last time?" comparison.
4. **No k0mmand3r integration** — the checklist system and the k0mmand3r
   command system are parallel codepaths that don't talk to each other.

## Design: `/is` as a k0mmand3r Verb

### Syntax

```
/is [checklist-name] [--json] [--explain] [--since-last] [--gate]
```

k0mmand3r form (typed `K0mmand3rCmd` variant):

```rust
// Add to K0mmand3rCmd enum in b00t-cli/src/k0mmand3r/mod.rs
Is {
    name: Option<String>,       // checklist name; None = list all
    json: bool,                 // full per-check disposition as JSON
    explain: bool,              // show failure/undetermined reasons
    since_last: bool,           // only print if disposition changed
    gate: bool,                 // exit non-zero if not Satisfied (for gating)
}
```

REPL examples:

```
/is                                    # list available checklists
/is system-normal                      # run system-normal checklist
/is system-normal --explain            # show why checks failed
/is system-normal --json               # machine-readable output
/is system-normal --since-last         # only report if state changed
/is system-normal --gate               # block dispatch if not Satisfied
```

CLI form (preserves existing `b00t is` surface, adds flags):

```bash
b00t is system-normal                  # Phase 1 behavior (unchanged)
b00t is system-normal --since-last     # new: flap/regression detection
b00t is system-normal --gate           # new: exit 1 if not Satisfied
```

### Integration with k0mmand3r Dispatch Pipeline

The `/is` verb is a **gate verb** — it doesn't dispatch work, it answers a
question. This matters because k0mmand3r's dispatch pipeline needs a way to
evaluate preconditions before sending tasks to agents.

#### Gate Pattern in Dispatch

```
/is system-normal --gate && /dispatch codex --task "deploy staging"
```

When composed in a k0mmand3r pipeline (the `/loop` mechanism or shell `&&`),
`/is --gate` acts as a precondition gate: if the system is not normal, the
pipeline halts before dispatching work to agents that would fail anyway.

#### Gate Pattern in Loop

```
/loop goal:deploy metric:uptime verify:/is\ system-normal\ --gate max:10
```

The loop runner calls `b00t is system-normal --gate` as its `verify` command.
If system-normal fails (exit 1), the loop reverts and retries — the metric
is "did the system stay normal after this iteration's change."

#### Gate Pattern in MCP

Add a `b00t_is` MCP tool to b00t-mcp that exposes the same surface:

```json
{
  "name": "b00t_is",
  "description": "Evaluate a system-normal checklist. Returns 3-valued disposition (Satisfied/Violated/Unknown) with per-check outcomes.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "name": { "type": "string", "description": "Checklist name (without .checklist.toml)" },
      "json": { "type": "boolean", "default": false },
      "explain": { "type": "boolean", "default": false },
      "since_last": { "type": "boolean", "default": false }
    },
    "required": ["name"]
  }
}
```

Agents in the k0mmand3r dispatch pipeline call `b00t_is` before dispatching:

```
agent → mcp__b00t_mcp__b00t_is { name: "system-normal" }
      → if Satisfied: proceed with dispatch
      → if Violated: report failing checks to executive, halt
      → if Unknown: escalate to human, halt
```

### Persistence (Phase 3)

Append to `~/.b00t/is-<checklist>.jsonl` on every evaluation:

```jsonl
{"ts":"2026-09-13T12:00:00Z","checklist":"system-normal","disposition":"Satisfied","failing":[]}
{"ts":"2026-09-13T12:05:00Z","checklist":"system-normal","disposition":"Violated","failing":["gh-auth: not logged in"]}
{"ts":"2026-09-13T12:10:00Z","checklist":"system-normal","disposition":"Satisfied","failing":[]}
```

The `--since-last` flag compares the current disposition against the last
JSONL entry. If identical, prints nothing and exits 0. If changed, prints
the delta. This is the flap detector — agents polling `b00t is --since-last`
in a loop get notified only when the system transitions between states,
not on every evaluation.

JSONL format matches the existing `datum_guard.rs` pattern (`usage_log_with_base()`).
No daemon, no DB — consistent with this codebase's append-only JSONL bias.

### Phase 2: `compose_rhai` (Deferred)

Phase 1's implicit-AND (all checks must be Satisfied) covers the common case.
Phase 2 adds a `[b00t.checklist]` section with a `compose_rhai` field:

```toml
[b00t.checklist]
compose_rhai = "git_clean && docker_daemon && (gh_auth || ssh_auth)"
```

`compose_rhai` runs in a Rhai scope pre-populated with one bool variable per
`check.id` (`Satisfied` → `true`, `Violated`/`Unknown` → `false` for this
scope only — the top-level aggregate still preserves the 3-way split).

This is deferred because:
- The implicit-AND covers the first real use case (system-normal)
- Rhai shell-exec access needs design (Open Question 3 in original CONOPS)
- The `compose_rhai` feature requires no API changes — it's additive

### Seed Checklist: `system-normal.checklist.toml`

The first real checklist, porting `doctor_cmd.rs`'s hardcoded checks into
declarative `GateSpec` entries:

```toml
# _b00t_/system-normal.checklist.toml
[b00t]
name = "system-normal"
type = "checklist"
hint = "Baseline system-health gate — all checks must be Satisfied"

[[b00t.check]]
id = "b00t-cli"
command = "b00t"
hint = "b00t-cli binary on PATH"

[[b00t.check]]
id = "git-repo"
file = "~/.b00t/.git"
hint = "b00t workspace is a valid git repo"

[[b00t.check]]
id = "b00t-mcp-binary"
file = "~/.b00t/target/release/b00t-mcp"
hint = "b00t-mcp binary exists"

[[b00t.check]]
id = "gh-auth"
command = "gh"
env = "GH_TOKEN"
hint = "GitHub CLI available and authenticated"

[[b00t.check]]
id = "just"
command = "just"
hint = "just command runner available"

[[b00t.check]]
id = "cargo"
command = "cargo"
hint = "Rust toolchain available"

[[b00t.check]]
id = "hermes-config"
file = "~/.hermes/config.yaml"
hint = "Hermes agent config exists"
```

This checklist can be evaluated today — `checklist.rs` and `is_cmd.rs` already
handle it. The only missing piece is writing the file to `_b00t_/`.

### Relationship to k0mmand3r Loop Syntax

The existing `LoopSpec` in `k0mmand3r/mod.rs` already has a `verify` field —
a shell command that prints a scalar metric. `/is --gate` fits naturally as
a verify command:

```
/loop goal:stabilize metric:disposition verify:b00t\ is\ system-normal\ --gate max:20
```

But the loop runner currently expects `verify` to print a parseable number,
not just exit 0/1. Two options:

**Option A** (minimal): Treat exit code as the metric (0 = good, 1 = bad).
The loop runner already handles "improved = current < best" for numeric
metrics; mapping exit codes to 0.0/1.0 is a one-line shim.

**Option B** (richer): Add a `verify_disposition` field to `LoopSpec` that
understands the 3-valued exit code (0/1/2) and treats Violated as a hard
failure (revert + retry), Unknown as a soft failure (skip iteration, don't
revert), and Satisfied as success (keep + measure metric).

Recommend Option A for Phase 1, Option B when `compose_rhai` lands.

### Open Questions

1. **Where do `.checklist.toml` files live?** Currently `list_checklists()`
   scans `--path` (default: `_b00t_/`). This works but mixes checklist datums
   with installable software datums. A `_b00t_/checklists/` subdir would
   separate concerns but breaks the "all datums flat in `_b00t_/`" convention.
   **Recommendation**: keep flat in `_b00t_/` for now — the `.checklist.toml`
   suffix already distinguishes them from `.mcp.toml`, `.skill.toml`, etc.

2. **Should `/is` be a k0mmand3r verb or a gate pattern?** The k0mmand3r
   parser currently routes all `/verb` commands through `K0mmand3rCmd::parse()`.
   Adding `/is` as a variant is clean. But `/is` doesn't dispatch work — it's
   a query, like `/status`. This is fine: `/status` is already a variant with
   no dispatch semantics.

3. **MCP tool naming**: `b00t_is` follows the existing `b00t_*` convention.
   But the help text says "Stateful system-normal checklist gate" — should the
   MCP tool be `b00t_checklist` instead (more general)? **Recommendation**:
   `b00t_is` for CLI parity; the description clarifies it's checklist evaluation.

4. **Persistence path**: `~/.b00t/is-<checklist>.jsonl` per the original
   CONOPS. But `~/.b00t` is a symlink to `~/.dotfiles`, so this writes into
   the dotfiles repo. Is that acceptable? The existing `datum_guard.rs` already
   writes to `<base>/.b00t/*.jsonl` which resolves the same way. **Acceptable**
   — consistent with existing pattern; add to `.gitignore`.

## Summary of Changes

| Component | Status | What's needed |
|-----------|--------|---------------|
| `checklist.rs` | Phase 1 shipped | None — working |
| `is_cmd.rs` | Phase 1 shipped | Add `--since-last`, `--gate` flags |
| `K0mmand3rCmd::Is` | New | Add variant + parser |
| `b00t_is` MCP tool | New | Add to b00t-mcp tool surface |
| `system-normal.checklist.toml` | New | Write to `_b00t_/` |
| JSONL persistence | Phase 3 | Append on each evaluation |
| `compose_rhai` | Phase 2 | Deferred |
| `LoopSpec` integration | Phase 1 | Option A (exit code as metric) |
| CONOPS-system-normal.md | Stale | Update to reference this doc |

## Non-Goals

- Not a monitoring/alerting system — no daemon, no push notifications
- Not a replacement for `doctor check` — `is` is the boolean gate, `doctor`
  remains the detailed diagnostic view
- Not a replacement for k0mmand3r's `/status` verb — `/status` reports agent
  state, `/is` reports system health

<!-- b00t:map v1
summary: CONOPS — b00t is (system-normal checklist) as a k0mmand3r verb + MCP tool + loop gate
tags: k0mmand3r, is, checklist, system-normal, gate, mcp, loop, disposition
tier: frontier
cmds: b00t is <name>, /is <name>, mcp__b00t_mcp__b00t_is
complexity: 7
-->
