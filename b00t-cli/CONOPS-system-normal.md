# CONOPS: `b00t is <checklist>` — stateful system-normal checks

> **Status (2026-08-10): superseded for now.** After drafting this, direction
> from the user was "make the alias, don't reimplement a new subsystem" — `is`
> was added as a plain clap alias for the existing `whatismy` command
> (`#[clap(aliases = ["inspect", "is"])]` on `Commands::Whatismy` in
> `main.rs`), reusing `whatismy status` rather than building the new
> checklist datum type / `Satisfies<ChecklistConstraint>` machinery described
> below. This doc is kept as the design for the not-yet-built stateful
> boolean-checklist feature, in case that's picked up later — but the
> immediate `b00t is` UX today is just `whatismy`'s existing subcommands
> (`b00t is status`, `b00t is role`, etc.), not `b00t is system-normal`.

## Origin

Raised while closing out the Favorite Sounds on-device verification loop
(2026-08-09): once a fix is proven working, there's no single command that
answers "is the system in a known-good state right now?" as a single
boolean an agent or a shell `if` can act on. `b00t doctor check` gets close
but reports a *list* of pass/fail rows, not one aggregate answer, and
nothing about it is stateful (no persisted history, no "same as last time?"
comparison).

Ask: design `b00t is system-normal?` — a stateful true/false response
backed by a list of named checks that must **all** pass, with the
pass/fail composition expressed as a boolean-logic script (Rhai, since
that's already the embedded scripting engine in this codebase) rather than
hardcoded Rust `&&` chains.

## Existing building blocks (don't reinvent these)

This repo already has every primitive this feature needs — the work is
composition and a CLI surface, not new evaluation machinery.

1. **`GateSpec` (`src/gates.rs`)** — a single precondition with `command`,
   `file`, `env`, `rhai`, `knowledge_backend` fields. `rhai` already lets a
   check be an arbitrary boolean Rhai expression. `eval_disposition()`
   returns a 3-valued `ufo_types::Disposition` (`Satisfied` / `Violated {
   reason }` / `Unknown`) — **not** a bare bool.

2. **The 3-valued Disposition is load-bearing, not incidental.** Issue
   #927 was exactly the bug of collapsing a Rhai eval *error* into `false`
   (i.e. treating "couldn't determine" as "determined to be false"). The
   fix (see `rhai_gate_disposition()` and the `#927` regression tests at
   the bottom of `gates.rs`) keeps eval errors as `Unknown`, distinct from
   a clean `false` (`Violated`). **Any new "system normal" boolean must
   preserve this 3-way split** — collapsing back to bool=true/false at the
   top level would silently reintroduce #927 one layer up.

3. **`Satisfies<C>` / `IsoAuditable` (ufo-types, external crate, already a
   workspace dep in b00t-cli, b00t-c0re-lib, b00t-lib-chat)** — the
   project's actual convention for "does X satisfy constraint C". `gates.rs`
   already has `impl Satisfies<GatePreconditions<'_>> for BootDatum`. Per
   `_b00t_/ufo-types-adoption-baseline.md`, this repo explicitly tracks and
   discourages "parallel vocabularies" that reimplement what ufo-types
   already models — a new ad-hoc `SystemNormalResult` bool type would be
   exactly that anti-pattern.

4. **`b00t doctor check` (`src/commands/doctor_cmd.rs`)** — already runs a
   ~20-item checklist (binaries, submodule drift, gutted gitdirs, DNS,
   GitHub API, soul.db, task queue, epoch state, skill symlinks, stray
   dirs...) and prints `N/M satisfied`. This is 90% of "system normal"
   today, just not exposed as one boolean, not user-extensible via TOML,
   and not stateful.

5. **`datum health-check --all` / `health-report` (`commands/datum.rs`)**
   — per-datum `HealthState::{Pass,Warn,Fail}` gate evaluation, already
   wired to `GateSpec`.

6. **Append-only stateful logging pattern (`src/datum_guard.rs`)** —
   `usage_log_with_base()` writes JSONL events to `<base>/.b00t/*.jsonl`.
   This is the existing precedent for "stateful" persistence in this repo
   (not a database, not a single mutable status file) and is what a
   system-normal history should reuse rather than inventing a new store.

## Gap this fills

`doctor check` answers "show me every row." Nothing today answers "one
boolean, all rows ANDed, scriptable, with the *rows themselves*
user-defined per checklist (not hardcoded in `all_deps()`)." Concretely
missing:

- A single exit code / boolean an agent or `if` statement can branch on.
- User-defined, TOML-declared checklists (`system-normal` is one instance;
  a project could also define `deploy-ready`, `release-ready`, etc. — same
  mechanism, different check list).
- The AND-composition itself expressed as data/script (Rhai) rather than a
  hardcoded Rust loop — so the *rule* for "what counts as normal" (all
  required checks pass? 90% pass? critical checks pass AND at most 1 warn?)
  is editable without a recompile.
- Persisted history: last result, last-changed timestamp, so flapping vs.
  stable-good vs. stable-bad is distinguishable.

## Proposed design

### 1. Checklist datum: `<name>.checklist.toml`

Reuses `GateSpec` verbatim for each item — zero new evaluation code:

```toml
# _b00t_/system-normal.checklist.toml
[b00t]
name = "system-normal"
type = "checklist"
hint = "Baseline system-health gate — all checks must be Satisfied"

[[b00t.check]]
id = "git-clean"
rhai = "true"                    # placeholder; real checks below
hint = "no destructive git state pending"

[[b00t.check]]
id = "docker-daemon"
command = "docker"

[[b00t.check]]
id = "b00t-repo"
file = "~/.b00t/.git"

[[b00t.check]]
id = "gh-auth"
rhai = ''' `sh:gh auth status 2>&1 | grep -q "Logged in"` '''
# (exact command-embedding syntax TBD — see Open Question 3)

# Top-level composition rule — the "boolean logic script" the ask calls for.
# Defaults to implicit AND (all Satisfied) if omitted; only needed when the
# rule is NOT a flat AND.
[b00t.checklist]
compose_rhai = "git_clean && docker_daemon && b00t_repo && gh_auth"
```

`compose_rhai` runs in a Rhai scope pre-populated with one bool variable
per `check.id` (`Satisfied` → `true`, `Violated`/`Unknown` → `false` for
this scope only — the top-level aggregate disposition below still
preserves the 3-way split; `compose_rhai` is deliberately allowed to be a
simpler bool-in/bool-out predicate since that's what "boolean logic
script" asked for). Omit `compose_rhai` and the behavior is exactly "all
must be true" — matching the ask's literal spec — with no script to write
for the common case.

### 2. Aggregate disposition (not aggregate bool)

```rust
pub enum ChecklistDisposition {
    Satisfied,                          // every check Satisfied (or compose_rhai true)
    Violated { failing: Vec<String> },  // >=1 genuine Violated, or compose_rhai false
    Unknown  { undetermined: Vec<String> }, // no Violated, but >=1 Unknown and none forced false
}
```

Exit codes for shell scripting: `0` = Satisfied, `1` = Violated, `2` =
Unknown — so `b00t is system-normal` is usable directly in `if`, and a
caller that only wants "did anything definitely break" can test `$? -eq
1` specifically instead of any-nonzero.

### 3. CLI surface

```bash
b00t is system-normal            # exit code only + one-line summary
b00t is system-normal --json     # full per-check disposition + reasons
b00t is system-normal --explain  # human-readable, shows which check(s) failed and why
b00t is                          # list all *.checklist.toml checklists + last-known state
```

`is` as a new top-level verb (not nested under `doctor`) because the ask's
phrasing — "`b00t is system-normal?`" — is itself the intended UX: natural
enough to type without checking `--help`. `doctor check` stays as the
detailed diagnostic tool; `is <checklist>` is the one-shot gate a script or
another agent calls.

### 4. Statefulness

Append to `~/.b00t/is-<checklist>.jsonl` (mirrors `datum_guard.rs`'s
`model-usage.jsonl` pattern) on every evaluation:

```json
{"ts":"2026-08-10T14:02:11Z","checklist":"system-normal","disposition":"Satisfied","failing":[]}
```

Enables, without new infra:
- `b00t is system-normal --since-last` — only print if disposition changed
  since the previous entry (flap/regression detection).
- A cheap dashboard: `tail -1 ~/.b00t/is-*.jsonl` per checklist.
- No daemon, no DB — consistent with this repo's existing "no docker,
  appendonly JSONL" bias ([[feedback_no_docker_podman_only]] equivalent
  preference already established for this codebase's infra choices).

## Phasing

**Phase 1 (minimal, ships the literal ask):** `checklist.toml` datum type
reusing `GateSpec` as-is, implicit-AND-only (no `compose_rhai` yet),
`b00t is <name>` CLI printing pass/fail + exit code. No persistence yet.
Gets "true/false, all(true), named checks" working end to end with the
smallest diff.

**Phase 2:** `compose_rhai` top-level composition for non-AND rules.

**Phase 3:** JSONL statefulness + `--since-last` / `--explain`.

**Phase 4:** Seed a real `system-normal.checklist.toml` by porting
`doctor_cmd.rs`'s `all_deps()` hardcoded Rust checks into declarative
`GateSpec` entries where possible (some, like `check_submodule_drift()`,
shell out to a script and may stay as `command`/`rhai`-wrapped checks
rather than pure declarative gates — that's fine, `GateSpec.rhai` can wrap
a shell one-liner the same way `doctor_cmd.rs`'s `sh()` helper does today).

## Open questions (for a human to decide, not inferable from the code)

1. **Command grammar**: top-level `b00t is <name>` (new verb) vs. `b00t
   doctor is <name>` (nested under the existing diagnostic surface) vs.
   folding into `datum health-check` with a new `--checklist` flag on the
   existing machinery. This doc assumes the first (matches the ask's exact
   phrasing) but it's a new top-level CLI verb, worth confirming before
   wiring into `main.rs`'s `Commands` enum.

2. **Where do `.checklist.toml` files live?** Same `_b00t_/` directory as
   every other datum (consistent, but mixes "installable software" datums
   with "assert this about the running system" datums in one directory),
   or a new `_b00t_/checklists/` subdir?

3. **Rhai access to shell commands**: today's `GateSpec.rhai` runs in a
   bare `rhai::Engine::new()` with *no* registered functions or scope
   variables (the doc comment claiming "Available vars: name, datum_type,
   path" in `gates.rs` is currently aspirational — not implemented). A
   real `gh-auth`-style check needs either (a) a registered `sh(cmd) ->
   bool` Rhai function, or (b) staying as a plain `command`/`file`/`env`
   gate and reserving `rhai` for genuinely-boolean logic over other
   checks' results (as `compose_rhai` does in Phase 2). Recommend (b) for
   individual checks, Rhai-for-composition-only — keeps the sandbox small
   and avoids giving arbitrary shell-exec power to a field that's meant to
   be a readable boolean predicate.

## Non-goals

- Not a monitoring/alerting system — no daemon, no push notifications, no
  scheduled evaluation (an agent or cron calls `b00t is <name>` when it
  wants an answer; `ScheduleWakeup`/cron is a separate concern if periodic
  checking is later wanted).
- Not a replacement for `doctor check`'s detailed diagnostic output —
  `is` is the boolean gate, `doctor check` remains the "show me
  everything" view.
