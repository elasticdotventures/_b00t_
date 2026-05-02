# b00t Naming Conventions — Ergonomics Audit

## What's Working ✅

| Convention | Pattern | Verdict |
|------------|---------|---------|
| Datum type suffixes | `<name>.<type>.<ext>` | Excellent — grep-friendly, self-describing |
| Skill names | kebab-case | Idiomatic for Claude Code plugins |
| Rust structs | PascalCase | Standard |
| Rust functions | `snake_case` + verb prefix (`handle_`, `run_`, `get_`) | Standard |
| Git branches | `feat/`, `fix/`, `chore/`, `refactor/` | Conventional commits — good |
| Env vars | `SCREAMING_SNAKE_CASE` | Standard |

---

## Issues & Fixes 🔴

### 1. Env var prefix split: `B00T_*` vs `_B00T_*`

**Current:** Both used interchangeably — `B00T_SESSION_ID` and `_B00T_ROLE`.

**Problem:** `_` prefix on env vars is a POSIX convention for *shell-internal/private* vars. Using `_B00T_` signals "internal" unintentionally. Grep for `B00T_` misses `_B00T_` hits.

**Fix:** Standardize on `B00T_` for all public/agent-facing vars. Reserve `_B00T_` strictly for shell-internal use (auto-set, not user-facing).

---

### 2. `0` substitution in names: `b00t`, `sm0l`, `ch0nky`

**Current:** `sm0l`, `ch0nky` as TOML keys and tier names.

**Problem:** `grep sm0l` works but `grep small` doesn't. Humans mis-type. Docs look unprofessional to newcomers. Not valid identifiers in some languages.

**Fix:** Use as *aliases/labels* only — not as primary keys in TOML or code.

```toml
# Bad
[tiers.sm0l]

# Better
[tiers.small]
alias = "sm0l"
```

**Exception:** `b00t` itself is the brand — keep as-is.

---

### 3. MCP tool separator inconsistency: hyphens vs underscores

**Current:** Server names use hyphens (`b00t-mcp`, `context7`), operation names use underscores (`b00t_agent_message`, `resolve_library_id`). Mixed within same namespace.

**Problem:** `mcp__b00t-mcp__b00t_agent_message` — three different separators in one identifier.

**Fix:** Can't change external MCP servers. For b00t-owned tools, standardize operation names to kebab-case matching server name style: `b00t-agent-message`. Already too entrenched to change short-term — document the pattern.

---

### 4. `.toml` vs `.tomllm` / `.tomllmd` extension inconsistency

**Current:** `orchestrator.role.toml` and `executive.role.tomllm` — same type, different extension.

**Problem:** `.tomllm` is the enriched format with `# @tribal:` annotations and `b00t:map` blocks. `.tomllmd` is the theorized superset for diagram/DSL/structured-markdown affordances and currently downgrades to generic `.tomllm` handling in b00t core. `.toml` is plain. But they're not consistently applied — `orchestrator.role` didn't get upgraded.

**Fix:** Migration rule — any datum with `# @tribal:` or `# b00t:map` annotations SHOULD use `.tomllm`. Any datum that also needs diagram/DSL/structured-markdown affordances MAY use `.tomllmd`, but MUST remain compatible with downgrade to `.tomllm` until specialized parser support is enabled. Plain config-only datums use `.toml`.

---

### 5. `whatismy` command

**Current:** `b00t whatismy <thing>` — queries agent context.

**Problem:** Non-standard English, odd CLI ergonomics. `b00t whoami` already exists for identity. `whatismy` reads like broken grammar.

**Fix:** Alias or rename to `b00t context <key>` or `b00t inspect <key>`. Keep `whatismy` as deprecated alias.

---

### 6. Datum type `ai` vs `ai_model` — underscore inconsistency

**Current:** `anthropic.ai.toml` (type=`ai`) and `claude-3-5-sonnet.ai_model.toml` (type=`ai_model`).

**Problem:** `ai` = provider, `ai_model` = specific model. Distinction is correct but `ai_model` with underscore breaks the `<name>.<type>.<ext>` pattern — types should be single tokens without underscores.

**Fix:** Rename type `ai_model` → `model`. Files become `claude-3-5-sonnet.model.toml`. Cleaner, greppable.

```
anthropic.ai.toml          # provider (unchanged)
claude-3-5-sonnet.model.toml  # model (renamed from ai_model)
```

---

### 7. Agent names: roles vs proper names mixed

**Current:** `alpha.agent.toml`, `beta.agent.toml`, `executive.agent.toml`, `ralph.agent.toml`.

**Problem:** `alpha`/`beta`/`executive` are role-descriptive; `ralph` is a proper name. No consistent convention — is an agent named by role or by persona?

**Fix:** Pick one scheme per use case:
- **Role agents** (functional): `orchestrator`, `reviewer`, `shipper` — what they *do*
- **Persona agents** (character): keep names like `ralph` in `*.persona.toml` or `*.agent.toml` with `persona = true`

---

### 8. `lfmf` — opaque acronym

**Current:** `b00t lfmf <tool> <lesson>` — "Learn From My Failure".

**Problem:** Undiscoverable to newcomers. `b00t --help` lists it but the name doesn't hint at function.

**Fix:** Add alias `b00t lesson` or `b00t learn-from`. Keep `lfmf` as primary (it's tribal shorthand — intentional). Document in help text: `lfmf (Learn From My Failure) — record a lesson`.

---

## Priority Recommendations

| Priority | Issue | Effort | Impact |
|----------|-------|--------|--------|
| 🔴 High | `B00T_` vs `_B00T_` env var prefix | Low — grep+replace | Breaks grep, confuses agents |
| 🔴 High | `ai_model` → `model` datum type | Medium — file renames | Cleaner type namespace |
| 🟡 Medium | `sm0l`/`ch0nky` as TOML primary keys | Low — add `alias` field | Grep-ability & professionalism |
| 🟡 Medium | `.toml` vs `.tomllm` migration rule | Low — add to CLAUDE.md | Clarity, no renames needed |
| 🟢 Low | `whatismy` → `context` / `inspect` | Low — add alias | Discoverability |
| 🟢 Low | Agent naming scheme | Medium — convention doc | Consistency |
| ⬜ Skip | MCP separator inconsistency | High — external deps | Not worth fixing |
