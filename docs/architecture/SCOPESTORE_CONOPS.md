# CONOPS: ScopeStore — concept of operations

**Status**: Living doc (not an ADR — describes intended usage, not a decision record)
**Related**: _b00t_ issue #893 (ScopeStore umbrella), `SCOPESTORE_CONCURRENCY_ADR.md`
(backend consistency guarantees), `b00t-c0re-gov/src/scope_store.rs`,
`scope_chain_view.rs`, `redb_scope_store.rs`, `redis_scope_store.rs`,
`discovery.rs`, `scope_credential_guard.rs`, `scope_audit.rs`

## What problem this solves

Before ScopeStore, "where does this piece of config/state live" had no single
answer in b00t: some things belonged to one repo, some to one machine, some
were meant to be shared across every agent everywhere — and each of those had
its own bespoke storage path, with no consistent precedence rule when the
same logical key existed at more than one level.

ScopeStore gives that problem exactly one abstraction: a **repo → node →
global** scope chain, one flat get/set contract per scope
(`ScopeStore::get_raw`/`set_raw`), and one JSONPath query layer
(`Queryable::query`) that sits above whichever chain of scopes a caller
assembles. The storage backend (`RedbScopeStore` today, `RedisScopeStore`
tomorrow) is invisible above that seam — callers write against the trait, not
against redb or redis directly.

## The mental model

Think of a `ScopeChainView` as a **most-specific-first override stack**, not
a merge:

```
repo scope    (most specific — "this repository's own setting")
  ↓ falls back to
node scope    (this machine's default, shared across every repo on it)
  ↓ falls back to
global scope  (the one shared default, if nothing more specific exists)
```

A **read** (`get_raw`) walks the stack top-down and returns the first hit —
whichever scope answers first wins, and nothing below it is even inspected
once a hit is found. There is no merging of partial values across scopes: if
repo-scope has the key, node-scope and global-scope's values for that same
key are invisible for that read, full stop.

A **write** (`set_raw`) always names its target scope explicitly. There is no
`set(key, value)` that means "write to whichever scope is closest" — every
call site says `set_raw(&ScopeId::Repo("myrepo"), key, value)` or
`&ScopeId::Node(...)` or `&ScopeId::Global`. This is deliberate: silent
shadowing (a well-meaning write that only takes effect once a more specific
scope's copy is deleted) is exactly the kind of "why didn't my config change
anything" bug this design refuses to allow. If you want to override a value,
you write to the more specific scope that's currently winning — you don't
write blindly and hope.

## Scope stereotypes — what belongs where

| Scope | Identity | Typical contents | Backend (today) |
|---|---|---|---|
| `Repo(id)` | opaque key, e.g. `sha256(remote_url)`; one per repo *and* one per submodule boundary (never flattened into a parent) | repo-specific overrides: feature flags for this project, per-repo skill config | `RedbScopeStore` @ `.b00t/store.redb` |
| `Node(hostname)` | hostname | machine-local defaults shared by every repo/agent on this box | `RedbScopeStore` @ `$XDG_CONFIG_HOME/b00t` |
| `Global` | singleton, no identity needed | fleet-wide defaults meant to apply everywhere unless overridden | `RedbScopeStore` today; redis (`RedisScopeStore`) is the intended v2 for a genuinely shared/distributed global scope |

Skills, souls, and feature-flag datums are the common case that legitimately
lives at all three levels with fallback (a skill's default at global, a
per-machine override at node, a per-repo pin at repo). **Credentials are the
explicit exception**: `scope_credential_guard.rs` rejects any write whose key
matches the `.credential`/`.credentials` datum-type suffix, at *every* scope,
unconditionally. ScopeStore is not a secrets store. If you're tempted to
write a token or API key through `set_raw`, don't — see "What ScopeStore
deliberately does not do," below.

## Common patterns

### 1. Read a config value with sensible fallback

```rust
let value = chain.get_raw("some.setting")?;
// Some(v) if any scope in the chain had it, most-specific wins.
// None if nobody in the chain ever set it — not an error.
```

This is the default pattern for "give me the effective value of X for this
agent, on this machine, in this repo, right now."

### 2. Override at the most specific scope that makes sense

```rust
// Pin a setting for just this repo, leaving node/global defaults alone
// for every other repo on this machine.
chain.set_raw(&ScopeId::Repo(repo_id), "some.setting", json!("repo-specific-value"))?;
```

Because writes are always explicit-target, this is also how you *remove* an
override in spirit: write the desired value back to the scope you actually
want it to live in, rather than relying on any implicit precedence at write
time.

### 3. Reach into a stored JSON document with JSONPath

A scope's stored value can be an arbitrary JSON document, not just a scalar.
`Queryable::query` resolves the key through the chain first (most-specific
wins, same as `get_raw`), then evaluates a JSONPath expression against
whatever was found:

```rust
// config resolved at whichever scope has it, then $.b00t.name extracted
let matches = chain.query("config", "$.b00t.name")?;
```

Empty (`Vec::new()`), not an error, when the key isn't set anywhere in the
chain — a query against nothing is a valid, uninteresting answer, not a
failure.

### 4. Know *why* a read resolved where it did (audited reads)

For any read where provenance matters — debugging "why is this repo picking
up the global default instead of what I thought was a node-level
override," or a compliance/audit trail — use `get_raw_with_audit` instead of
`get_raw`:

```rust
let logger = AuditLogger::open(scope_root.join("audit.jsonl"));
let value = chain.get_raw_with_audit("some.setting", &logger)?;
```

Every scope checked-and-missed on the way to the hit (or every scope checked,
if the key wasn't found anywhere) is appended to `logger` as one JSON line —
an ordered `boundaries_crossed[from, to, direction]` list, not a boolean. A
hit on the very first (most-specific) scope logs zero crossings, and that
zero is itself meaningful (distinguishable from "this key was never audited
at all"). Use `get_raw` for the hot path where you don't need provenance;
reach for `get_raw_with_audit` when you do.

### 5. Assemble a chain via discovery instead of hand-wiring it

For the common case of "walk from wherever I am (the current repo/submodule)
out to node and global, picking up every intermediate submodule boundary
along the way," don't hand-assemble the `Vec<Box<dyn ScopeStore>>` — use
`discovery::walk_lazy_chain` starting from the innermost scope, expanding
each node via its `$.b00t.manifest`:

```rust
let order = walk_lazy_chain([innermost_repo_id], max_depth, |id| {
    // look up id's parent(s) from its published manifest
});
// order is already most-specific-first; open one ScopeStore per entry
// and hand the Vec to ScopeChainView::new.
```

The walker is cycle-guarded and depth-capped, so a malformed or cyclic
submodule graph terminates instead of looping forever, and a graph deeper
than your cap is truncated (not rejected) — the boundary node is still
recorded, its own children just aren't expanded further.

## What ScopeStore deliberately does not do

- **It is not a secrets store.** `scope_credential_guard.rs` rejects
  credential-shaped keys at every scope. Use the reference/delivery pattern
  from `_b00t_/learn/managing-secrets.md` (Infisical/Teller/SOPS/vals at
  delivery time) or `datum_credential.rs`'s dedicated OS-keyring-wrapped
  storage — outside ScopeStore entirely — if a local encrypted copy is
  genuinely required.
- **It does not merge values across scopes.** Most-specific-wins is a
  full-value override, not a deep merge of partial JSON documents. If two
  scopes each set half of a config object, only one half is ever visible at
  a time (whichever scope's copy resolves).
- **It does not guess a write target.** No API exists that infers "the
  closest scope" for a write. Every `set_raw` call names its scope.
- **It does not (yet) give any cross-process concurrency guarantee stronger
  than what the backend itself provides.** See
  `SCOPESTORE_CONCURRENCY_ADR.md` for exactly what `RedbScopeStore` (OS file
  lock, fails fast on a second writer) and `RedisScopeStore` (no lock,
  last-write-wins, no ordering signal) each actually guarantee today, and
  why porting concurrency assumptions from one backend to the other is
  unsafe. A generation-token mechanism for cross-query read consistency is
  tracked as its own follow-up (#897) and is explicitly not built yet.

## Where the ergonomics are still evolving

`ScopeStore`/`ScopeChainView`'s current shape is deliberately narrow: flat
`get_raw`/`set_raw` plus the `Queryable` bridge. A path-addressed `Writable`
trait (`set_path`, writing into a nested location inside a stored document
rather than only replacing the whole value at a key) has been proposed
separately (issue #895) as an ergonomic layer on top of this same seam —
check `b00t-c0re-gov/src/scope_chain_view.rs`'s current state before
assuming it exists; as of this writing it is tracked but not necessarily
merged. This CONOPS describes the `get_raw`/`set_raw`/`query` contract that
is definitely in the tree today.

## Quick reference: which method for which job

| I want to... | Use |
|---|---|
| Read the effective value of a key, fallback included | `ScopeChainView::get_raw` |
| Read the effective value *and* know which scope it resolved at | `ScopeChainView::get_raw` + `resolving_scope`, or `get_raw_with_audit` if you also want it persisted |
| Write/override a value at a specific scope | `ScopeChainView::set_raw(&target_scope, key, value)` |
| Reach into a nested field of a stored JSON document | `Queryable::query(key, jsonpath)` |
| Get an audit trail of exactly which scopes were checked | `get_raw_with_audit` + `AuditLogger::read_all` |
| Build a chain from a repo's actual submodule tree | `discovery::walk_lazy_chain` starting from the innermost scope |
| Store a secret/token | Don't — see "What ScopeStore deliberately does not do" |
