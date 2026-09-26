# ADR: b00t / mise / Usage Ownership Boundary

**Status:** Accepted
**Date:** 2026-09-26
**Issue:** #1337

## Context

b00t has three optional integration layers for CLI tooling:

- **b00t** — typed capability datums, install/update policy, status, governance, evidence.
- **mise** — project environments, tool pins, task execution.
- **usage** — structured CLI grammar (KDL): flags, arguments, subcommands, completions, docs, man pages.

Each tool owns a distinct concern. None replaces the other. The integration must
keep mise and usage optional — neither becomes a b00t build or runtime dependency.

## Decision

### Ownership boundary

| Concern | Owner |
|---|---|
| Capability/datum ontology | b00t |
| Install/update/version/status policy | b00t |
| Project tools and environment activation | mise |
| Task execution | mise |
| CLI grammar and parsing specification | usage KDL |
| Help/completions/docs/man pages/SDKs | usage generators, recorded by b00t |
| Trust, governance, and artifact evidence | b00t |

### Integration rules

1. **b00t datums remain authoritative.** KDL references are structured metadata,
   not a replacement for typed datum fields (`install`, `desires`, `version`,
   `version_regex`, `depends_on`, maintenance checks, governance).

2. **usage is an optional CLI provider**, not a first-level dependency. The
   `UsageCliProvider` follows the same `CliProvider` contract as `MiseCliProvider`.
   b00t builds and runs without usage installed.

3. **Generated artifacts are derived.** Shell completions, docs, and man pages
   generated from KDL specs are recreatable from the recorded source + generator
   version. They are never treated as proof that the CLI is installed or trusted.

4. **Explicit output only.** Generation commands require an explicit `--out`
   directory. Generated scripts/completions are never installed or executed
   implicitly.

5. **Provenance is recorded.** Source URL/path, generator version, KDL digest,
   and generated artifact digests are stored in b00t evidence metadata
   (`[b00t.usage_artifacts]`).

6. **Fail closed on conflicts.** If a KDL spec changes, existing generated
   artifacts become stale until revalidated. Conflicts require explicit resolution.

7. **Mise integration is task-level.** b00t-managed mise tasks for usage
   validation/generation are optional and depend on an explicitly registered
   `usage.cli` datum.

## Phased rollout

- **Phase 0** (this ADR): `usage.cli` datum, hidden passthrough, ownership boundary.
- **Phase 1**: Runtime provider + KDL linting.
- **Phase 2**: Artifact generation + provenance.
- **Phase 3**: Mise task integration.
- **Phase 4**: Optional typed Rust adapter (feature-gated, separate crate).

## Consequences

- `b00t usage --help` works via passthrough when usage is installed.
- `b00t usage lint <spec>` validates KDL files.
- b00t's build is unaffected — no usage crate dependency.
- mise tasks for usage validation are optional add-ons.