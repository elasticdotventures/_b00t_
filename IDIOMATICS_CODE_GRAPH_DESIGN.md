# Idiomatics → Code Graph / Irontology Hook Design

## Overview

This document describes how `IDIOMATICS.datum` hooks into the code graph so that
idiomatic patterns are machine-enforceable against Rust source code. The system
uses the CROSS_EDGE pattern: each idiomatic creates a typed edge from Guard nodes
to CodeElement nodes in the knowledge graph.

## Architecture

```
                         +------------------+
                         | IDIOMATICS.datum  |
                         | (pattern defs)    |
                         +--------+---------+
                                  |
                                  v
+----------+    +---------+    +-----------+    +---------------+
| Source   |--->| b00t-ast |--->| Ontology  |--->| Irontology    |
| .rs files|    | (syn)   |    | Graph     |    | NeumannStore   |
+----------+    +---------+    +-----------+    +---------------+
                                     |                  |
                                     v                  v
                               +-----------+    +---------------+
                               | CodeGraph |    | CROSS_EDGE    |
                               | (tree-    |    | Guard→Element |
                               |  sitter)  |    | violation     |
                               +-----------+    +---------------+
```

## Data Flow (Detailed)

### Phase 1: Source → b00t-ast extracts AST

`b00t-ast` (syn-based, at `~/.b00t/b00t-ast/`) walks a Rust project tree and
produces `Vec<CodeElement>` with qualified names, file paths, line spans, doc
comments, and structural info (fn params, struct fields, enum variants, etc.).

**Entry point:** `b00t_ast::run_extraction(project_root) -> ExtractionResult`

Output: `ExtractionResult { elements: Vec<CodeElement>, file_count, counts, errors }`

### Phase 2: CodeElements → OntologyGraph builds symbol graph

`OntologyGraph::from_extraction(&result)` in `b00t-ast/src/ontology.rs` produces
typed edges between code elements (CONTAINS, CALLS, IMPLEMENTS, DEFINES, EXTENDS,
HAS_FIELD, HAS_VARIANT, HAS_METHOD, IMPL_CONTAINS).

### Phase 3: OntologyGraph → NeumannStore indexes via irontology bridge

`IrontologyBridgeClient::ingest()` converts `DatumNode` to `FactRecord` triples
and `EdgeRecord` edges, then calls `store.upsert_facts()` and `store.upsert_edges()`.

The real `NeumannStore` has:
- `FactRecord { subject, predicate, object }` — RDF triples
- `EdgeRecord { from, to, kind: EdgeKind, weight }` — typed edges
- `EdgeKind`: Defines, Calls, DependsOn, Tests, Contains, ClassifiedAs, StoredIn, Related
- `SymbolRecord { id, blob, path, name, kind, start_line, end_line, signature, content }`

### Phase 4: Idiomatic patterns query the graph for violations

Idiomatic patterns define **code-level heuristics** that map to AST node properties.
When a pattern detects a violation, it creates a `CROSS_EDGE` from the Guard node
to the violating CodeElement node.

## Edge Schema (CROSS_EDGE Pattern)

### New EdgeKinds (in bridge, mapped via predicate strings)

These are NOT NeumannStore `EdgeKind` variants (cannot modify internals). Instead,
they are stored as `FactRecord` predicates with the prefix `b00t:cross:`.

| Predicate | Subject | Object | Meaning |
|-----------|---------|--------|---------|
| `b00t:cross:enforces` | `b00t:guard/<guard_id>` | `b00t:element/<qualified_name>` | This guard enforces an idiomatic violated by this element |
| `b00t:cross:violates` | `b00t:element/<qualified_name>` | `b00t:idiomatic/<pattern_name>` | This code element violates this idiomatic |
| `b00t:cross:implements` | `b00t:guard/<guard_id>` | `b00t:idiomatic/<pattern_name>` | This guard implements/enforces this idiomatic |
| `b00t:cross:has_count` | `b00t:guard/<guard_id>` | `... (literal integer)` | Violation count for this guard |

### URIs

```
b00t:guard/hive-guards.hive.toml:42        — Guard node ID
b00t:idiomatic/polyseme-purge              — Idiomatic pattern node
b00t:element/my_crate::my_module::my_fn     — CodeElement node (qualified_name)
```

## Component Design

### 1. Idiomatics Checker (`b00t-ast/src/idiomatics_checker.rs`) — NEW FILE

Checks extracted `CodeElement` records against active idiomatic patterns.

```rust
/// Result of checking a code element against idiomatic patterns
pub struct IdiomaticViolation {
    pub element_qn: String,
    pub pattern_name: String,
    pub guard_id: String,
    pub file_path: String,
    pub start_line: usize,
    pub description: String,
}

/// Check all extracted elements against all active idiomatic patterns
pub fn check_against_idiomatics(
    elements: &[CodeElement],
    idiomatics: &IdiomaticsRegistry,
) -> Vec<IdiomaticViolation>;
```

**Heuristic checkers per pattern:**

| Idiomatic | Checker Logic |
|-----------|---------------|
| `polyseme-purge` | Scan fn names / struct names for multi-meaning words (`report`, `record`, `ticket`) |
| `type-over-severity` | Check struct fields for `severity: String` used as type category |
| `bitwise-flags` | Check enum variants for compound names (`BugAndWaste`, `BugOrWaste`) |
| `verb-surface` | Check fn names — noun-first public API functions w/ambiguous meanings |
| `severity-tunneling` | Check fn params named `severity` with non-standard type |
| `report-polyseme` | Scan for function names containing `report` as a verb |
| `severity-as-type` | Check enum variants encoding type in severity field |
| `jargon-drift` | Scan doc comments for term usage inconsistent with jargon glossary |

### 2. Guard→CodeElement Bridge (`b00t-c0re-lib/src/idiomatics_guard.rs`) — NEW FILE

```rust
/// Bridge module: idiomatics ↔ code graph ↔ guard documentation
pub struct IdiomaticsCodeGraphBridge {
    neumann: IrontologyBridgeClient,
}

impl IdiomaticsCodeGraphBridge {
    /// Register a guard→idiomatic→violation edge in the knowledge graph
    pub async fn record_violation(
        &self,
        violation: &IdiomaticViolation,
    ) -> Result<IrontologyIngestResult>;

    /// Query all violations for a specific guard
    pub async fn query_guard_violations(
        &self,
        guard_id: &str,
    ) -> Result<Vec<IdiomaticViolation>>;

    /// Get violation count for a guard
    pub async fn guard_violation_count(
        &self,
        guard_id: &str,
    ) -> Result<usize>;

    /// Enrich guard docs output with violation counts
    pub async fn enrich_guard_docs(
        &self,
        entries: &[GuardDocEntry],
    ) -> Result<Vec<EnrichedGuardDoc>>;
}
```

### 3. Enriched Guard Doc Entry

```rust
pub struct EnrichedGuardDoc {
    pub id: String,
    pub action: String,
    pub pattern: String,
    pub message: String,
    pub redirect: String,
    pub idiomatic: Option<String>,       // linked idiomatic pattern name
    pub violation_count: usize,          // number of code locations violating
}
```

### 4. Integration with `b00t guard docs`

Modified `handle_docs()` in `b00t-cli/src/commands/guard.rs`:

```
After loading guard entries from TOML:
1. Load IDIOMATICS.datum and build idiomatic→guard_id mapping
2. Query irontology for ENFORCES edges per guard
3. Append violation count + idiomatic name to each guard doc entry
```

Output format (human):

```
Guard: hive-guards.hive.toml:68
  Pattern:   pip install
  Action:    warn
  Message:   🦨 use uv pip install (faster, reproducible)
  Idiomatic: uv-over-pip
  Violations: 5 locations
```

### 5. Integration with `b00t-ast index`

Modified `b00t-ast dir` / `b00t-ast self` command:

```
After extraction:
1. Run `run_extraction(project_root)` — existing
2. Check idiomatic violations via `idiomatics_checker`
3. Record violations in irontology via `IdiomaticsCodeGraphBridge`
4. Report summary: "3 idiomatic violations found across 2 files"
```

### 6. Integration with Codebase Memory

When `b00t-ast` feeds into `codebase-memory-mcp index_repository`, the
violation edges are pushed alongside the ontology graph:

```
OntologyGraph.to_mcp_payload() now includes VIOLATES edges:
{
  "edges": [
    { "from": "guard:...", "to": "element:...", "rel_type": "ENFORCES" },
    { "from": "element:...", "to": "idiomatic:...", "rel_type": "VIOLATES" }
  ]
}
```

The codebase-memory-mcp already supports custom edge types via `rel_type` field,
so no changes needed on that side.

## Implementation Plan

### Step 1: Create `idiomatics_checker.rs` in b00t-ast (~120 lines)

- Defines `IdiomaticViolation` struct
- Implements heuristic checkers for each idiomatic pattern
- `check_against_idiomatics()` function
- Unit tests for each checker

### Step 2: Create `idiomatics_guard.rs` in b00t-c0re-lib (~100 lines)

- `IdiomaticsCodeGraphBridge` struct
- `record_violation()` — stores facts with `b00t:cross:*` predicates
- `query_guard_violations()` — queries facts by subject
- `guard_violation_count()` — counts edges by guard_id

### Step 3: Modify `ontology.rs` in b00t-ast (~30 lines)

- Add `EnforcesBy` / `Violates` edge types to `OntologyEdge.rel_type` values
- Export VIOLATES edges when idiomatic violations are present

### Step 4: Modify `guard.rs` in b00t-cli (~40 lines)

- `handle_docs()` calls `enrich_guard_docs()` when irontology is available
- Append idiomatic name + violation count to output

### Step 5: Modify `b00t-ast/main.rs` (~20 lines)

- Add `--check-idio` flag to trigger idiomatic checking after extraction
- Integration run: extract → check → record violations → report

### Step 6: Add `CROSS_EDGE` fact predicates to IDIOMATICS.datum (~10 lines)

- Add `b00t:cross:enforces` and `b00t:cross:violates` to the guard_entanglement section

## Yak-Shave Opportunities

### 1. ExtendedEdgeKind in irontology bridge (small, <30 lines)

The `EdgeKind` enum in `b00t-c0re-lib/src/irontology_bridge.rs` only has:
`ClassifiedAs`, `DependsOn`, `StoredIn`, `Related`.

**Needed:** Add edge kinds for idiomatic enforcement. Since we can't modify
NeumannStore's EdgeKind, we store these as `FactRecord` predicates with the
`b00t:cross:` prefix. But if we wanted real petgraph edges, we'd need:

- Add `Enforces` variant to `EdgeKind` in the bridge
- Add `Violates` variant
- Map these to NeumannStore's existing `Related` kind with different weight/meaning

**Workaround:** Use `FactRecord` with predicate=`b00t:cross:enforces` instead
of native edges. This works with the current bridge API (`upsert_facts`).

### 2. Idiomatic pattern parser (~50 lines)

`IDIOMATICS.datum` has structured `[[idiomatics.patterns]]` but there's no
Rust parser that deserializes it into a typed struct.

A simple `IdiomaticsRegistry` struct with `serde` deserialization:

```rust
#[derive(Deserialize)]
pub struct IdiomaticsDatum {
    pub b00t: IdiomaticsB00t,
}

#[derive(Deserialize)]
pub struct IdiomaticsB00t {
    pub idiomatics: IdiomaticsContent,
}
```

This is a one-time yak-shave.

### 3. NeumannStore query by predicate (small, ~20 lines)

The bridge's `SemanticQuery` doesn't support querying facts by predicate value.
To count violations, we need to filter facts by predicate `b00t:cross:enforces`.

The `query()` method already returns all facts; client-side filtering works but
is inefficient. A predicate filter in `SemanticQuery` would be ideal but requires
changing the bridge (not NeumannStore).

**Workaround:** Client-side filter after `query(SemanticQuery { predicate: None })`.
Works fine for typical violation counts (<1000 edges).

## Query Patterns

### Find all violations for an idiomatic

```cypher
// In codebase-memory knowledge graph:
MATCH (g:Guard)-[:ENFORCES]->(e:CodeElement)
WHERE g.id CONTAINS 'polyseme-purge'
RETURN e.qualified_name, e.file_path, e.start_line
```

### Show guard docs with violation counts

```
b00t guard docs --json
→ enriched JSON includes idiomatic + violation_count per guard
```

### Find all code locations with open violations

```
b00t grok ask "show all active idiomatic violations" --topic idiomatics
→ queries irontology for b00t:cross:violates facts
→ returns grouped by guard_id
```

## File Inventory

| File | Action | Description |
|------|--------|-------------|
| `b00t-ast/src/idiomatics_checker.rs` | **CREATE** | Heuristic checkers for idiomatic patterns |
| `b00t-c0re-lib/src/idiomatics_guard.rs` | **CREATE** | Guard↔CodeElement bridge with Irontology |
| `b00t-ast/src/ontology.rs` | **MODIFY** | Add EnforcesBy/Violates edge types |
| `b00t-cli/src/commands/guard.rs` | **MODIFY** | Enrich docs output with violation counts |
| `b00t-ast/src/main.rs` | **MODIFY** | Add `--check-idio` flag |
| `_b00t_/datums/IDIOMATICS.datum` | **MODIFY** | Add cross-edge predicates to guard_entanglement |
| `b00t-c0re-lib/src/irontology_bridge.rs` | **NOTE** | Yak-shave: ExtendedEdgeKind (deferred) |

## Summary

This design connects idiomatic patterns to source code locations through the
existing irontology knowledge graph, using facts (not native edges) to work within
the "no modify irontology-mcp internals" constraint. The CROSS_EDGE pattern
creates a connected graph layer: Guard nodes → (ENFORCES) → CodeElement nodes,
queryable via both the irontology bridge and the codebase-memory MCP.
