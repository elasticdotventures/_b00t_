# Concise Datum Format

DRY, context-saving approach to datum references and usage examples.

## Problem

The verbose format wastes tokens and duplicates information:

```toml
[[b00t.references]]
url = "https://github.com/elasticdotventures/just"
description = "elasticdotventures just fork"
type = "github"  # Redundant - obvious from URL

[[b00t.references]]
url = "https://just.systems/man/en/"
description = "Official Just Manual"
type = "docs"  # Redundant - obvious from URL
```

**Issues:**
- 4 lines per reference
- Duplicate type information (URL domain reveals type)
- Verbose `[[b00t.references]]` table array syntax

## Solution: Concise Format

### References

**Format:** `["url#fragment-description", ...]`

```toml
references = [
  "https://github.com/elasticdotventures/just#fork-with-enhancements",
  "https://just.systems/man/en/#official-manual",
  "https://github.com/casey/just#upstream"
]
```

**Features:**
- **1 line per reference**
- **Auto-detect type** from URL domain:
  - `github.com|gitlab.com` → GitHub
  - `/docs|doc.|docs.` → Docs
  - `stackoverflow.com` → StackOverflow
  - `.blog|medium.com` → Blog
  - `tutorial|/learn/` → Tutorial
- **Fragment (#text)** becomes description
- **Hyphens** in fragments convert to spaces: `#my-cool-feature` → "my cool feature"
- **No fragment?** Auto-generate from repo name or type

### Usage Examples

**Format:** `["command  # description", ...]`

```toml
usage = [
  "rustc --version  # Check Rust version",
  "cargo build  # Build project",
  "cargo test  # Run tests",
  "cargo clippy  # Lint code"
]
```

**Features:**
- **1 line per example**
- **Command** before `#`, **description** after
- **No description?** Description field will be empty

## Examples

### Concise (NEW - RECOMMENDED)

```toml
[b00t]
name = "rustc"
type = "cli"
hint = "Rust compiler"
lfmf_category = "rust"

usage = [
  "rustc --version  # Check version",
  "cargo build  # Build",
  "cargo test  # Test"
]

references = [
  "https://github.com/rust-lang/rust#official-repo",
  "https://doc.rust-lang.org/book/#the-book",
  "https://doc.rust-lang.org/std/#std-lib"
]
```

**Token savings:** ~70% reduction vs verbose format

### Verbose (OLD - Still Supported)

```toml
[b00t]
name = "rustc"
type = "cli"
hint = "Rust compiler"
lfmf_category = "rust"

[[b00t.usage]]
description = "Check version"
command = "rustc --version"

[[b00t.usage]]
description = "Build"
command = "cargo build"

[[b00t.usage]]
description = "Test"
command = "cargo test"

[[b00t.references]]
url = "https://github.com/rust-lang/rust"
description = "official repo"
type = "github"

[[b00t.references]]
url = "https://doc.rust-lang.org/book/"
description = "the book"
type = "docs"

[[b00t.references]]
url = "https://doc.rust-lang.org/std/"
description = "std lib"
type = "docs"
```

## Backward Compatibility

Both formats work! The deserializer automatically detects which format is used:

```rust
#[serde(deserialize_with = "b00t_c0re_lib::deserialize_references")]
pub references: Option<Vec<Reference>>,

#[serde(deserialize_with = "b00t_c0re_lib::deserialize_usage")]
pub usage: Option<Vec<UsageExample>>,
```

Old datums continue to work without modification.

## Implementation Details

### Type Detection Rules (b00t-c0re-lib/src/datum_types.rs:98-118)

```rust
fn detect_type(url: &str) -> ReferenceType {
    let lower = url.to_lowercase();
    if lower.contains("github.com") || lower.contains("gitlab.com") {
        ReferenceType::GitHub
    } else if lower.contains("stackoverflow.com") || lower.contains("stackexchange.com") {
        ReferenceType::StackOverflow
    } else if lower.contains(".blog") || lower.contains("medium.com") || lower.contains("/blog/") {
        ReferenceType::Blog
    } else if lower.contains("tutorial") || lower.contains("/learn/") {
        ReferenceType::Tutorial
    } else if lower.contains("/docs")
        || lower.contains("/doc/")
        || lower.contains("readthedocs")
        || lower.contains(".systems")
        || lower.contains("doc.")
        || lower.contains("docs.") {
        ReferenceType::Docs
    } else {
        ReferenceType::Community
    }
}
```

### Fragment Processing (b00t-c0re-lib/src/datum_types.rs:76-78)

```rust
let (url, fragment) = if let Some((u, f)) = url_with_fragment.split_once('#') {
    (u.to_string(), f.trim().replace('-', " "))
} else {
    (url_with_fragment.to_string(), String::new())
};
```

## Testing

All tests passing:
- `cargo test datum_types::tests` - 7 tests for parsing logic
- `cargo test test_datum_with_concise_format` - Integration test

## Migration Guide

### Converting Existing Datums

**Before:**
```toml
[[b00t.references]]
url = "https://github.com/user/repo"
description = "My description"
type = "github"
```

**After:**
```toml
references = ["https://github.com/user/repo#my-description"]
```

**Quick conversion:**
1. Extract URL
2. Add `#` + description (convert spaces to hyphens)
3. Combine into single-line array format

### When to Use Which Format

**Use concise** when:
- Simple references (URL + description)
- Simple usage (command + description)
- Token efficiency matters (always!)

**Use verbose** when:
- Need `output` field in UsageExample
- Complex structured data
- Programmatic generation

## Files Modified

- `b00t-c0re-lib/src/datum_types.rs` - Core parsing logic
- `b00t-c0re-lib/src/lib.rs` - Export deserializers
- `b00t-cli/src/lib.rs` - Use custom deserializers in BootDatum
- `b00t-cli/src/datum_utils.rs` - Integration tests
- `_b00t_/rust.cli.toml` - Example concise datum

## Benefits

1. **70% token reduction** vs verbose format
2. **DRY** - No duplicate type information
3. **Readable** - One line per item, easy to scan
4. **Context-saving** - Less to load, faster parsing
5. **Backward compatible** - Old format still works
6. **Auto-detection** - Smart type inference from URL patterns
