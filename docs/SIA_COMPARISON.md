# SIA vs b00t: Loop-Engineering Pattern Mapping (issue #792)

## Source Verification (important caveat)

Issue #792 cites SIA (`hexo-ai/sia`) as introducing "soul memory," "playbooks,"
and "multi-level durable memory" alongside agent profiles and profile-scoped
tool authorization. Fetching SIA's own README and `docs/architecture.md`
directly (2026-08) shows **SIA's real documentation does not use or describe
"soul memory," "playbooks," or multi-level durable memory anywhere** — those
terms do not appear in the source project. What SIA actually documents:

- A **3-agent generational loop**: Meta-Agent (writes an initial Target Agent
  from a task description) → Target Agent (executes the task, logs actions) →
  Feedback/Improvement Agent (reads the log, rewrites the Target Agent for
  the next generation).
- **Agent profiles**: JSON files bundling `(agent_impl, model, provider,
  agent_reference)`, resolved from `sia/defaults/profiles/` (bundled) or
  `./profiles/` (project override).
- **Run artifacts**: `runs/run_{id}/gen_{n}/` containing the generated agent
  code, an execution-trace log, and (gen ≥ 2) an improvement note — this is
  the closest real analog to what the issue calls "soul memory," but it is a
  flat per-generation file dump, not a queryable/typed store, and SIA's own
  docs never name it "soul" or "playbook."

This matters for triage: three of the issue's four headline "SIA patterns"
(soul memory, playbooks, multi-level durable memory) are the issue author's
own extrapolation/embellishment, not features SIA ships. This repo has one
prior instance of a fabricated architecture summary landing in `CLAUDE.md`
(the "Tax-Lawyer Platform" entry, corrected 2026-07-23) — treat any
issue/doc that attributes named subsystems to an external project as
needing direct source verification before being taken as a spec, as done
here.

## Pattern-by-Pattern Mapping

### 1. Agent Summoning via Profiles

**SIA:** `sia run --target-agent-profile X --meta-agent-profile Y` — flat
JSON bundling `(agent_impl, model, provider, agent_reference)`.

**b00t equivalent — already covered:**
- `AGENTS/--role=<role>.md` — role behavior supplements (worker, executive,
  operator, reviewer, pragmatic-hacker, podman, ux-designer; 8 roles today).
- `just compile-agent role="<role>" n_skills="3" out=<path>` — compiles a
  **single sandboxed AGENTS.md** per invocation: base boilerplate + role
  supplement + `b00t blessing --manifest --role=<role>` (tool authorization,
  see #2) + N randomly assigned transferable skills, timestamped. This *is*
  "summon a profile, get a configured agent" — functionally closer to SIA's
  `sia run --target-agent-profile` than the issue's gap analysis credits.
- `just provision-agent role="<role>" goal="<goal>"` wraps compile+launch
  into one operator command (`justfile:1296+`).

**Real gap:** b00t has no single typed *datum* bundling
`(agent_impl, model/tier, provider, authorized_tools)` as one addressable
artifact the way SIA's profile JSON is. Role (`AGENTS/`), tool auth
(`blessing`), and model/tier routing (the Cognitive Tiers table in
`CLAUDE.md`) are three separate mechanisms, composed at compile-agent time
by a shell script rather than declared as one object. This is real but
narrow: a `DatumType::AgentProfile` that composes the three existing
mechanisms, not a new subsystem.

### 2. Tool Authorization = Profile Contract

**SIA:** each profile pins `agent_impl`/`provider`/`model`; not a
capability/tool allowlist mechanism.

**b00t equivalent — already covered, and more structured:**
`b00t blessing --manifest --role <role>` (`b00t-cli/src/commands/blessing.rs`,
`b00t-c0re-gov/src/discovery.rs`) walks the datum `depends_on` graph for a
role and emits a manifest of required skills (with the tools each unlocks),
optional skills, forbidden command patterns, and a Postel next-hint. Per
`CLAUDE.md`: "Learning a skill datum unlocks the tools in its `unlocks`
field. No learning = no auth." This is graph-derived, per-role tool
authorization — a stronger primitive than SIA's flat profile pinning, since
it's queryable (`b00t-cli ontology sparql`) and composable across roles.

**Real gap:** none identified. This is the strongest "already covered" claim
in the issue — b00t's mechanism is more capable than SIA's on this axis.

### 3. Soul Memory — Ops Log

**SIA:** flat per-generation files (`target_agent.py`,
`agent_execution.json`, `improvement.md`) under `runs/run_{id}/gen_{n}/`.
Not called "soul memory" in SIA's own docs (see caveat above).

**b00t equivalent — already covered, and more structured:**
`b00t soul` (`b00t-cli/src/commands/soul.rs`, `b00t-c0re-lib/src/
soul_dataframerr.rs`) is a persistent per-agent identity/memory system,
already exposed as MCP tools (`soul_table_create`, `soul_table_list`,
`soul_row_insert`, `soul_row_query`, `soul_cursor_create/next/reset`,
`soul_alarm_set/check`). Concretely:
- **Typed tables** (DataFramerr: text/int/float/cake/bool/timestamp/
  token/json columns) — a queryable structured store, not flat files.
- **Cursors** — resumable read position over a table (`FrameCursor`),
  letting an agent page through accumulated history across sessions.
- **Alarms** — threshold/aggregate triggers over table state
  (`SoulAlarm`, `AlarmAggregate`).
- **`b00t soul distill`** — pipes a session transcript through a silent
  sm0l-tier LLM turn, extracts facts as K/V, writes to the soul store. This
  is the direct analog of SIA's Feedback Agent reviewing a log and
  persisting a distilled improvement — but generalized to any session, not
  gated to a benchmark-generation loop.
- **Multi-level scope** — `global_soul_dir()` (`~/._b00t_/`),
  `local_soul_dir()` (workspace `._b00t_/`), `active_soul_dir()` (local if
  present else global); `b00t soul init` provisions the local scope. This
  already implements global/project scoping; see #5 for the third
  (session) level.

**Real gap:** none structural. b00t's soul system is strictly more capable
than SIA's per-generation file dump (typed/queryable vs flat files,
explicit cursors/alarms vs none). If anything is missing it's polish
(discoverability of `soul distill` as the "ops log" pattern), not a new
subsystem.

### 4. Three-Agent Loop Architecture (Meta → Target → Feedback)

**SIA:** a benchmark-scored, generational self-improvement loop — Feedback
Agent literally rewrites the Target Agent's code each generation, scored
against held-out ground truth.

**b00t equivalent — partial, real gap:**
b00t has `AgentManager`/`AgentCoordinator`/`AgentConfig`/`AgentDef`
(`b00t-c0re-lib/src/agent_manager.rs`, `agent_coordination.rs`) for
multi-agent dispatch, voting, and approval gates, and the Worker role
(`AGENTS/--role=worker.md`) already runs a **2-arm A/B dispatch** (control
vs. treatment sub-agents, scored on roi/cost/time/accuracy/utility/risk,
reported to executive) — structurally a sibling of SIA's loop, but scored
by a rubric, not held-out benchmark ground truth, and not self-rewriting.
Lesson accumulation (`b00t lfmf`) plays the role of cross-generation
improvement, but it's operator/agent-authored prose, not an automated
code-rewrite-and-rescore loop.

**Real gap:** b00t has no automated "rewrite my own harness against a
scored benchmark and keep the better generation" loop. This is SIA's actual
novel contribution and the one pattern worth scoping as new work, if
prioritized — everything else in the issue already has a b00t equivalent.

### 5. Playbook Levels (Global / Project / Run)

**SIA:** `sia/defaults/{providers,profiles}/` (global) → `./{providers,
profiles}/` (project) → CLI flags (run).

**b00t equivalent — already covered:**
`_b00t_/` (global datums) → workspace-local `._b00t_/` (soul: `local_soul_
dir()`) / project config → per-invocation CLI flags and `b00t soul init`
for session-scoped local state. `b00t learn` resolution already fans out
`DatumSearchSource`+`GraphAdjacencySource` across this hierarchy. No
canonical `DatumType::Playbook` exists, but the three-level *resolution
order* the issue asks for is already implemented for both config (datums)
and memory (soul).

## Bottom-Line Recommendation

**Already covered (no new subsystem needed):** agent profiles/summoning
(`AGENTS/` + `compile-agent`), tool authorization (`blessing --manifest`),
soul memory (`b00t soul` + DataFramerr + `soul distill`), and multi-level
playbook resolution (datum hierarchy + soul scope). Three of the issue's
four headline SIA patterns were also not verifiably SIA's in the first
place (see Source Verification) — the issue conflates its own proposed
architecture with what SIA documents.

**Adopt as a scoped gap, not the issue's full task list:**
1. `DatumType::AgentProfile` — a thin typed datum composing role + tool
   manifest + model/tier into one addressable artifact, so `compile-agent`
   reads a declared profile instead of assembling one ad hoc per call.
2. (Optional, lower priority) An automated scored-rewrite loop modeled on
   SIA's Meta→Target→Feedback, if there is a concrete benchmark task that
   would benefit — this is the one genuinely novel pattern SIA contributes
   that b00t lacks today.

Do **not** implement `b00t worker summon`, a new `--soul <scope>` flag, or
a new `runs/<run_id>/soul.jsonl` ops log as proposed in the issue body —
each duplicates an existing mechanism (`compile-agent`+`provision-agent`,
soul's existing global/project/active scoping, and `b00t soul` tables/
cursors respectively).
