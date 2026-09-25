# b00t: Project Scoping, ProjectProvider, and Ontology→SysML v2→kr0ki Pipeline

Status: approved by user (via `/goal`), proceeding directly to implementation.
Branch: `feat/rep0-r00t-scope` (worktree `/home/brianh/b00t-work/.worktrees/feat-rep0-r00t-scope`).
Base: `fix/provider-batch-spec` @ `53aaae57` (that branch's own in-progress cherry-pick is untouched and out of scope).

## Origin

Started as two small asks — fix a dead auth-header schema in the MCP `dotmcpjson`
install path, and add ergonomic `rep0`/`r00t` CLI discovery for the existing
`--path _b00t_` datum-scoping workaround. Escalated, by user direction, into a
larger initiative: a `ProjectProvider` abstraction (pluggable backends: Jira,
mise, bl/bailian) that projects hang task/requirement tracking off of, plus
validating b00t's ontology export as SysML v2 and bridging it to kr0ki
(kroki.io) as a rendering/derivation source.

## Corrected assumptions (verified in-codebase before designing)

These were assumed missing or broken going in; verified otherwise by reading
the actual source:

- **SysML v2 / OWL2 export already exists and works.** `b00t viz entangle
  --format sysml-v2` (and `owl2`, `cytoscape`, `mermaid`, `json`, `ascii`,
  `svg`) renders the datum ontology graph via a shared `SceneGraph` IR
  (`src/viz/mod.rs`: `scene_to_sysmlv2`, `scene_to_owl2`). Real, tested,
  minimal-but-functional — not a stub. **Do not rebuild this.**
- **"Ontology" is formally grounded**, not just a name: `ufo_types` crate
  (`Stereotyped`, `UfoStereotype`, `IsoAuditable`) implements UFO (Unified
  Foundational Ontology) stereotyping already used for ISO-standard audit
  evidence on datums (`src/commands/ontology.rs`).
- **`b00t ontology export --format=mermaid/cytoscape`**, referenced in
  `whoami.rs` help text, **does not exist as such** — `OntologyCommands` only
  has `Query`/`Sparql`/`FindAgent`. The real command is `b00t viz entangle
  --format ...`. The help text is stale; low-priority doc fix, not a design
  driver.
- **`ProjectProvider` does not exist anywhere.** Genuinely new.
- **`soul_scope.rs`'s `ShardKind::Project`** (issue #1102) exists as library
  code with real tests but is wired into zero CLI subcommands.
- **`-p/--path` (`_B00T_Path`) has no directory walk-up discovery** — always
  either an explicit flag or the hardcoded global default
  (`~/.dotfiles/_b00t_`). This is the actual gap `rep0`/`r00t` fill.
- **`McpHttpStreamMethod`'s `bearer_token_env_var` / `http_headers` /
  `env_http_headers`** (`src/datum_mcp.rs`) are fully defined, documented with
  `(Codex: ...)` annotations, but consumed nowhere — confirmed dead schema,
  affecting both the `dotmcpjson` and Codex install targets equally.
- **`reqif` support today is a native-schema reinterpretation** ("REQIF-style
  but native to the schema datum", `src/datum_schema.rs`), not real ReqIF XML
  import/export. Building on this native form rather than real ReqIF XML is
  the pragmatic choice — no design driver requires the XML interchange format
  itself.

## Sub-projects, in directed order (#3, #2, then the rest)

### A. `ProjectProvider` + `rep0`/`r00t`/`pr0ject` CLI (was #3, folding in #1)

New file `src/datum_project.rs` (matches existing `datum_*.rs` naming:
`datum_repo.rs`, `datum_mcp.rs`, `datum_stack.rs`).

```rust
trait ProjectProvider {
    fn status(&self) -> Result<ProjectStatus>;
    fn list_tasks(&self) -> Result<Vec<Task>>;
    fn create_task(&self, task: NewTask) -> Result<Task>;
    fn link_requirement(&self, req: RequirementRef) -> Result<()>;
}
```

Backends (each a thin shell-out/HTTP client, matching existing patterns
elsewhere in the codebase — e.g. `check_command_available` gating, `duct::cmd`
subprocess calls as seen in `datum_repo.rs`):
- `MiseProvider` — shells out to `mise` (there's already a
  `feat/issue-1336-mise-bridge` branch in this same repo family; check it for
  reusable patterns before writing this from scratch, but don't depend on that
  branch merging).
- `JiraProvider` — REST API, needs an API token via a datum/env var.
- `BlProvider` — shells out to the `bl` CLI (bailian).

Provider selection: a `[b00t.project]` section in the project's `_b00t_/`
scope (see below) names the active backend; default to `MiseProvider` if
unset and `mise` is available, else a no-op/local-only provider that just
records into the native-schema requirement/task records without an external
system.

**CLI:**

- `b00t rep0 init` — create `./_b00t_/` (plain, unhidden dir) if absent,
  idempotent if present. This is the literal filesystem anchor.
- `b00t rep0 where` — walk up from cwd looking for `_b00t_/` (git-`.git`-style
  ancestor search); report the nearest hit, or "none — run `b00t rep0 init`".
  Report what the global fallback would be too.
- Wire the walk-up result into `--path`'s default resolution: precedence is
  (1) explicit `--path`/`_B00T_Path` — unchanged, highest priority; (2)
  nearest `_b00t_/` found walking up from cwd (new); (3) existing global
  default `~/.dotfiles/_b00t_` — unchanged, final fallback.
- `b00t r00t init` / `b00t r00t where` — same primitive one tier further out:
  `where` skips the first `_b00t_/` hit and reports the *next* one up the
  ancestor chain (real example already on disk:
  `/home/brianh/promptexecution/_b00t_`, shared by sibling project repos
  underneath it — inspect read-only, do not modify). r00t is exposed via its
  own subcommand rather than folded into the silent `--path` default chain,
  since silently reaching two directories up by default is more surprising
  than useful; a caller who wants r00t-scoped resolution asks for it
  explicitly.
- `b00t pr0ject` — **not** a plain alias of `soul` (per explicit clarification
  from the user: "a metapattern that does #1 \[soul init\]"). `b00t pr0ject
  init` = `rep0 init` (create `_b00t_/`) + `soul init` (create `._b00t_/`) +
  provider selection/registration, as one onboarding command for "this
  directory is now a project." `b00t pr0ject task ...` / `b00t pr0ject reqif
  ...` dispatch to the active `ProjectProvider`. Give it a `visible_alias`
  only if, after implementing, it turns out to be a strict superset with no
  behavioral divergence worth a separate name — otherwise keep it a distinct
  subcommand so the composition is legible.

### B. Header/auth fix for `dotmcpjson` install (was #2)

In `src/dispatch.rs`, `dotmcpjson_install_mcp`'s httpstream branch currently
emits only `{"url": command}`. Fix:

1. Add `client_type: Option<String>` to `McpHttpStreamMethod`
   (`src/datum_mcp.rs`) — `None` means `"http"` for output purposes; a TOML
   author can set `client_type = "sse"` for SSE-only servers (real example:
   Home Assistant's `mcp_server` integration at `.../mcp_server/sse`).
2. Emit `"type"` (from `client_type`, default `"http"`) and, when any header
   field is set, a `"headers"` object in the generated `.mcp.json` server
   config.
3. Value semantics (secret-safety matters here):
   - `http_headers` entries: literal static strings, pass through as-is.
   - `env_http_headers` entries (`{header: ENV_VAR_NAME}`): emit
     `"${ENV_VAR_NAME}"` (Claude Code's own env-interpolation syntax) — never
     resolve/bake in the actual runtime value.
   - `bearer_token_env_var`, if set: emit `"Authorization": "Bearer
     ${VAR}"` using the same non-resolved convention; if the user also set an
     explicit `Authorization` via `http_headers`, the explicit one wins
     (document this precedence inline).
4. Extend/add unit tests near the existing `mod tests` in `dispatch.rs`
   (`missing_dotmcpjson_is_initialized_with_an_empty_server_map` already
   exists there) covering: no-headers (unchanged shape + new `"type":
   "http"` default), bearer-only, and http_headers+env_http_headers merged.
5. Stretch goal only if trivial: apply the same fix to `codex_sync_dotmcpjson`
   in the same file, since its neighboring doc comments explicitly promise
   Codex support for these fields. Do not let this expand scope.

### C. Task + ReqIF linkage to projects

Extend the native-schema "REQIF-style" validation-requirement concept
(`datum_schema.rs`) so a requirement record can carry a project-id reference
(from sub-project A's `ProjectProvider`/`rep0` identity). `b00t pr0ject task
create` and `b00t pr0ject reqif link` land records in both the active
provider backend (mise/jira/bl) *and* a local datum-schema record for
traceability, so requirement/task state is queryable locally even when the
backend is unreachable.

### D. Ontology → SysML v2 validation + kr0ki bridge

The exporter (`scene_to_sysmlv2`) already exists; the actual new work:

1. **Validate** its output against a real SysML v2 grammar/LSP. Which of the
   three named tools (Open-MBEE/sysml-toolkit, daltskin/sysml-v2-lsp,
   elan8/spec42, and whatever `cargo install sysmlv2-lsp` really resolves to)
   is usable headless — pending the research fork's findings (maturity,
   install method, whether it exposes anything beyond an editor extension).
2. Fix whatever non-compliance the validator finds in the current minimal
   `part def` / `connection def` output.
3. **kr0ki bridge** — shape depends on research findings: kroki natively
   supports a fixed set of text DSLs (Mermaid, PlantUML, GraphViz, etc., not
   SysML natively per current understanding, to be confirmed) — so this is
   either (a) a transform step from `scene_to_sysmlv2` output into one of
   kroki's native formats, most likely PlantUML given PlantUML's own SysML/UML
   support, or (b) standing up a custom kroki "companion" service that accepts
   SysML v2 directly, if kroki's plugin architecture supports adding one
   cleanly. Do not assume (b) is straightforward until the research confirms
   kroki's companion-service extension model actually supports it.
4. **"b00t as a live source dataset for kr0ki"** — reuse the existing `b00t
   soul serve` HTTP server precedent (`src/soul_writer.rs`/soul CLI, serves
   K/V over HTTP on port 7700) rather than inventing a new serving mechanism:
   add a small `/viz/<format>` HTTP endpoint alongside it that returns
   freshly-rendered graph output on each call. Whether kroki itself can treat
   this as a "live" pull source, versus this just being "b00t is the
   always-current producer that something else polls," depends entirely on
   whether kroki's architecture has any live-fetch concept at all — the
   research fork was asked to report the truth here rather than assume a
   feature exists. If kroki is purely stateless request/response with no
   fetch-from-URL concept, "b00t as a kr0ki source dataset" means: a thin
   glue script/service periodically re-renders and re-POSTs to kroki, not a
   kroki-native pull integration — say so plainly in the follow-up report
   rather than describing the glue script as if it were kroki doing the
   pulling.

## Explicitly out of scope for this pass

- Real ReqIF XML import/export (staying on the native-schema form).
- Rewriting/replacing the existing SysML v2 or OWL2 exporters wholesale —
  only fixing spec-compliance issues the LSP validation surfaces.
- Touching `/home/brianh/b00t-work`'s in-progress cherry-pick, or anything
  under `/home/brianh/promptexecution/_b00t_` (read-only reference only).
- Pushing branches or opening PRs — local commits on `feat/rep0-r00t-scope`
  only, for the user to review and push themselves.
- A wider audit of the ~150-file codebase beyond what's directly touched by
  A–D above.

## Sequencing

A and B are independent of the research fork's findings and of each other —
implement in parallel. C depends on A. D depends on the research fork
(SysML v2 LSP viability + kroki architecture facts) landing first, and
partially on A (project identity for any per-project export scoping).
