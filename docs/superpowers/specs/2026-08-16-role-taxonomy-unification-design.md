# Design: Role Taxonomy Unification (agentic_role + b00t-c0re-hierarchy::Role)

**Status**: Proposed
**Related**: #905, #909 (documented "no parallel vocabularies" collision on `Operator`,
previously unresolved). Motivated by a review of PRs #1091/#1092 that found b00t-mcp's
ACL has no concept of caller role at all — this is phase A of that larger effort; the
role-aware ACL itself (phase B) and an installer flow to configure it (phase C) are
follow-on specs, written after this one, since both depend on the shape defined here.

## Context

Two independent, non-interoperating role systems exist today:

1. **`b00t-cli::agentic_role`** (internal module of `b00t-cli`, not its own crate) — a
   type-level system: `AgenticRole` sealed trait, ZST types `Worker`/`Executive`/
   `Operator`/`AppProvider` each implementing it, `RoleRef<T>` (a `Cow<'static, str>`
   wrapped with a phantom `T: AgenticRole` marker, guaranteeing the string matches
   `T::NAME`), and `KnownRole` — a sealed ADT enum wrapping one `RoleRef<T>` per known
   variant, giving both compile-time type preservation and runtime dispatch.
   `KnownRole::resolve(role_override)` is the actual runtime entry point: it reads an
   explicit override, then the `_B00T_ROLE` env var, defaulting to `Worker`. This is
   what `b00t whoami --role=<role>` and `b00t-cli`'s other role-aware commands use
   today — the only one of the two systems with real runtime resolution wired up.

2. **`b00t-c0re-hierarchy::Role`** (in its own crate, `b00t-c0re-hierarchy`, which
   `b00t-cli` depends on but which cannot depend back on `b00t-cli` without a cycle) —
   a plain `#[derive(Serialize, Deserialize)]` enum: `Captain`, `Executor`, `Mate`,
   `Player`, `Operator`, `Specialist`, `Bouncer`. Used by `Team`/`Agent` structs to
   model crew membership and roster buckets (`Team.captain_id`, `.executor_ids`,
   `.operator_ids`, `.specialist_ids`, `.bouncer_ids`, `.player_ids`). Has no runtime
   resolution mechanism of its own — nothing sets "this running session is
   `Role::Captain`" anywhere in the codebase today.

The two systems already use the bare string `"operator"` for arguably-different
meanings, a collision the code has flagged in a doc comment since #905/#909 without
resolving it. Separately, `Role::Mate` and `Role::Player` are already documented as
legacy aliases with real replacements in normal use (`Mate` → `Executor`/`Specialist`,
`Player` → `Agent.is_player: bool`) — `crew_handler.rs`'s own role-bucketing match arm
already treats them that way (`Role::Mate | Role::Player => specialists.push(agent)`).
`Role::Bouncer` similarly doesn't represent a distinct identity — it's a handoff to a
different system prompt/context for validation work, not a role an agent *has*.

This design deletes both existing systems' overlapping surface and replaces them with
one crate both can depend on, extending `agentic_role`'s `KnownRole`/`RoleRef` design
(chosen over a fresh plain enum) because it is the one with real runtime plumbing
already wired to `_B00T_ROLE`, and because the ZST/trait design's `AgenticCrew`
associated type and `AgenticRole::peers()` already carry the graph structure needed to
visualize the role hierarchy with `ledgrrr` later, at no extra cost in this design.

## Goals

1. One role taxonomy, one crate, used by both `b00t-cli` and `b00t-c0re-hierarchy`.
2. Resolve the `Operator` naming collision for real: one variant, one meaning (the
   two documented meanings — "administrative privileges, recruits/trains/enlists
   agents" and "crew dispatch, spins typed crews via k0mmand3r" — describe the same
   job from two angles; this design treats them as the same concept).
3. Retire `Bouncer`/`Mate`/`Player` as role variants. `Mate`/`Player` were already
   aliases in practice; `Bouncer` is reframed as a handoff/context-switch, not a role.
4. Model "specialist" as an open-ended stereotype family, not a fixed 5th variant:
   `Worker` stays a single generalized name with no sub-stereotyping; `Specialist`
   becomes the bucket for anything else, preserving the specific name given (e.g.
   `"appprovider"`, `"rust-specialist"`, `"security-auditor"`) for datum lookup.
   `AppProvider` survives only as the most common conventional *name* within that
   bucket, not as a distinct Rust type.

## Non-goals

- **The role-aware ACL for b00t-mcp itself.** This design produces the `Role`/
  `KnownRole` type the ACL will consume; the ACL's policy model (Allow/Escalate/Deny
  per role, `discoverable` flag, elicitation-based escalation) is a separate,
  already-partially-designed follow-on spec.
- **The installer flow for choosing an ACL policy.** Follow-on spec, depends on the
  ACL's config shape, not on this design directly.
- **Moving `capability-forge` under the `b00t ai` subsystem.** Noted as a requirement
  for when PR #1091 is revisited; unrelated to this role-taxonomy work and not
  actioned here.
- **Changing how role datums (`--role=<role>.md`, `.role.tomllmd`) are authored or
  loaded.** `whoami.rs`'s `load_role_datum` keeps working exactly as today — this
  design only changes what produces the `role_name: &str` it's called with.
- **Auditing every other consumer of `_B00T_ROLE` or role datum files for correctness.**
  Out of scope; this design's job is the type unification, not a behavior audit of
  unrelated call sites beyond the ones enumerated below.

## Architecture

### New crate: `b00t-c0re-role`

A new workspace member, following the existing `b00t-c0re-*` naming convention
(`b00t-c0re-gov`, `b00t-c0re-hierarchy`, `b00t-c0re-a2a`, `b00t-c0re-lib`) for
cross-cutting concerns both `b00t-cli` and `b00t-c0re-hierarchy` need without either
depending on the other. Contents, moved verbatim from `b00t-cli/src/agentic_role.rs`
except where noted:

- `mod sealed { pub trait Sealed {} }` and the `AgenticRole` / `AgenticCrew` traits —
  unchanged.
- `RoleRef<T: AgenticRole>` — unchanged.
- ZST types: **`Executive`, `Operator`, `Worker`, `Specialist`** (renamed from
  `Executive`/`Operator`/`Worker`/`AppProvider` — only `AppProvider` is renamed, to
  `Specialist`, per Goal 4; the `AppProvider` struct/impl is deleted, not kept
  alongside).
- `KnownRole` — same 4-variant sealed ADT shape (`Executive(RoleRef<Executive>)`,
  `Operator(RoleRef<Operator>)`, `Worker(RoleRef<Worker>)`,
  `Specialist(RoleRef<Specialist>)`), but `KnownRole::resolve`'s fallback arm changes:

  ```rust
  // Before (agentic_role.rs today): unknown name -> Worker, name preserved but
  // bucketed under the generalized type.
  KnownRole::from_str(&name).unwrap_or_else(|| {
      KnownRole::Worker(RoleRef::new_owned(name))
  })

  // After: unknown name -> Specialist. Worker is reserved for the exact literal
  // "worker" (or whatever explicit override/env value matches Worker::NAME);
  // anything else is a specialist stereotype, name preserved for datum lookup.
  KnownRole::from_str(&name).unwrap_or_else(|| {
      KnownRole::Specialist(RoleRef::new_owned(name))
  })
  ```

- `resolve_role(role_override: Option<String>) -> KnownRole` — unchanged behavior,
  still reads `_B00T_ROLE`, still defaults to `Worker` when nothing is set at all
  (the default-empty case is distinct from the unknown-non-empty-name case above:
  no override and no env var still means "worker", not "specialist named ''").

### `b00t-c0re-hierarchy` changes

- Delete `roles.rs`'s `Role` enum entirely. Add `b00t-c0re-role` as a dependency.
- `Agent.role: Role` becomes `Agent.role: KnownRole`.
- `Team` bucket fields rename to match the surviving 4 variants:
  `captain_id → executive_id`, `executor_ids → worker_ids`,
  `specialist_ids` stays `specialist_ids` (name already matches), `operator_ids`
  unchanged. `bouncer_ids` is deleted (no replacement field — Bouncer is not a role).
  `player_ids` is unchanged — `Agent.is_player: bool` already exists and already is
  the correct model for "human vs. software participant," orthogonal to `KnownRole`.
- `cake_economy.rs`, `governance_bridge.rs`, `recruitment.rs` — grep for `Role::`
  usage during implementation; expected to be limited to the same kind of
  match-on-variant pattern as `crew_handler.rs` below, updated the same way.

### `b00t-cli` changes

- Delete `src/agentic_role.rs`. Add `b00t-c0re-role` as a dependency; `whoami.rs`
  imports `resolve_role`/`KnownRole` from there instead.
- `commands/crew_handler.rs`'s role-bucketing match:

  ```rust
  // Before
  Role::Captain => captains.push(agent),
  Role::Executor => executors.push(agent),
  Role::Operator => operators.push(agent),
  Role::Specialist => specialists.push(agent),
  Role::Bouncer => bouncers.push(agent),
  Role::Mate | Role::Player => specialists.push(agent),

  // After
  KnownRole::Executive(_) => executives.push(agent),
  KnownRole::Worker(_) => workers.push(agent),
  KnownRole::Operator(_) => operators.push(agent),
  KnownRole::Specialist(_) => specialists.push(agent),
  ```

  The `println!("  Bouncers:")` roster line and its printing logic are deleted along
  with the bucket.
- Any other `Role::`/`agentic_role::` reference found during implementation (grep
  `params.rs`, `mcp_tools.rs`, `blessing.rs`, `wow.rs`, `doctor_cmd.rs`,
  `commands/agent.rs` — all surfaced by the earlier code search as touching one of
  the two role concepts) gets updated the same way: `Role::X` → the matching
  `KnownRole::Y(_)` pattern, old variant names mapped per Goal 4's table.

### Breaking changes to call out explicitly

- The default role name when no override/env var is set is **unchanged** (`"worker"`).
- Any *unknown, non-empty* role name previously bucketed as `Worker` (preserving the
  custom name) now buckets as `Specialist` instead. Anything reading `KnownRole::name()`
  and getting a custom string back is unaffected (same string); anything pattern-
  matching on `KnownRole::Worker(_)` specifically to catch "unknown roles" needs to
  match `KnownRole::Specialist(_)` instead. Flagged as a grep-and-check step during
  implementation, not resolved here.
- `Team`'s field names change (`captain_id`, `executor_ids`, `bouncer_ids` all gone).
  Any serialized `Team`/`Agent` data on disk using the old field names will fail to
  deserialize under `serde`'s default (non-`#[serde(alias = ...)]`) behavior. Whether
  any such data exists and needs a migration is an implementation-time question, not
  resolved here — flagged so it isn't missed.

## Testing

- Port `agentic_role.rs`'s existing unit tests (`resolve_role` default/override
  cases, `KnownRole::from_str` round-trips) into `b00t-c0re-role`, updated for the
  renamed `Specialist` type and the changed fallback-bucket behavior.
- Port `b00t-c0re-hierarchy/tests/hierarchy_test.rs`'s `Bouncer`/`Mate`/`Player`
  references — since those variants are deleted, the tests exercising them are
  deleted too (not adapted); replace with a test asserting an unknown role name
  resolves to `Specialist` with the name preserved (covers the changed fallback).
- No behavior in this design is conditionally compiled or feature-flagged — the ZST
  system's "formal provability" is a property of the type system itself, not of an
  optional test suite; ordinary `cargo test -p b00t-c0re-role -p b00t-c0re-hierarchy
  -p b00t-cli` coverage is sufficient, no extra heavy proof-obligation tests are
  required to ship this.
