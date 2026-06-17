# README Rust b00t Best Practices for Agents

⚠️ THE FOLLOWING ARE CONSIDERED ALIGNMENT FAILURES:
* MUST NEVER downgrade crates without explicit permission or instructions from user operator.
* MUST NEVER modify Cargo.toml directly, always run the `cargo add` cli
* MUST NEVER use xtask patterns for scripts and utilities


. don't write a postgres database interface. don't even store a dsn type -- find existing types exist.
use crates to do the heavy lifting.

* use existing crates, choose crates using the criteria:
fit for purprose, stability, popularity
* using libraries/crates are good for decomposition of your program, they often contain solutions for problems you may not have considered.
* it is ALWAYS better to fork and patch an existing crate than add more code to your primary codebase.

* 🦀💖🦑  (rust loves git)
- ALWAYS install + use cocogitto & husky pre-commit checks & tests
- ALWAYS fix clippy errors.
- there are examples in ~/.dotfiles/_b00t_/

* Error Handling:
	- Use ? Operator for Error Propagation: Leverage the ? operator to propagate errors, ensuring that each error variant implements the From trait for seamless conversions.
	- RECOMMEND snafu crate for Error Management: Implement the snafu crate to define and manage errors. It combines the benefits of thiserror and anyhow, facilitating structured error definitions and context propagation.
	- Define Modular Error Types: Create distinct error enums for each crate or module, ensuring they implement std::error::Error. Use snafu's macros to streamline this process.
	- Implement Display and Debug Traits: For each error type, implement the Display and Debug traits to facilitate informative logging and debugging.
	- Provide Clear Laconic Error Messages: Ensure error messages include: Root Cause: The fundamental
	issue.; Contextual Stack: The sequence of operations leading to the
	error.; User Perspective: A message understandable by end-users.




## Rust/Cargo CI/CD Setup - Codified Lessons Learned

### Pre-push Git Hooks (.git/hooks/pre-push)

```
#!/bin/sh
  # Quality gate enforcement before push
  echo "Running library tests before push..."
  cargo test -p <your-lib-crate>

  if [ $? -ne 0 ]; then
      echo "Library tests failed. Push aborted."
      exit 1
  fi

  echo "Running code formatting..."
  cargo fmt --all

  echo "Running code checks..."
  cargo check --all-targets --all-features

  if [ $? -ne 0 ]; then
      echo "Code check failed. Push aborted."
      exit 1
  fi

  echo "Running clippy lints..."
  cargo clippy --all-targets --all-features -- -D warnings

  if [ $? -ne 0 ]; then
      echo "Clippy lints failed. Push aborted."
      exit 1
  fi

  echo "All checks passed. Proceeding with push."

GitHub Actions CI (.github/workflows/ci.yml)

name: CI
  on:
    push:
      branches: [ main ]
    pull_request:
      branches: [ main ]

  env:
    CARGO_TERM_COLOR: always

  jobs:
    test:
      strategy:
        matrix:
          os: [ubuntu-latest]  # Simplified from multi-platform
          rust: [stable]       # Simplified from [stable, beta]
      runs-on: ${{ matrix.os }}

      steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@${{ matrix.rust }}

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run tests
        run: cargo test

      - name: Build
        run: cargo build --release

  Release Workflow (.github/workflows/release.yml)

  name: Release
  on:
    workflow_run:
      workflows: ["CI"]
      types: [completed]
      branches: [ main ]

  permissions:
    contents: write
    pull-requests: write
    issues: write
    repository-projects: write

  jobs:
    release:
      if: ${{ github.event.workflow_run.conclusion == 'success' && github.event.workflow_run.event == 'push' }}
      runs-on: ubuntu-latest
      steps:
        - name: Checkout code
          uses: actions/checkout@v4
          with:
            fetch-depth: 0
            token: ${{ secrets.GITHUB_TOKEN }}

        - name: Install cocogitto
          uses: cocogitto/cocogitto-action@v3

        - name: Setup git config
          run: |
            git config user.name "github-actions[bot]"
            git config user.email "github-actions[bot]@users.noreply.github.com"

        - name: Check if there are releasable changes
          id: check_changes
          run: |
            if cog check --from-latest-tag; then
              echo "has_changes=true" >> $GITHUB_OUTPUT
            else
              echo "has_changes=false" >> $GITHUB_OUTPUT
            fi
          continue-on-error: true

        - name: Create release
          if: steps.check_changes.outputs.has_changes == 'true'
          env:
            GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          run: |
            cog bump --auto
            NEW_VERSION=$(cargo pkgid | cut -d# -f2 | cut -d: -f2)
            cog changelog --at "v${NEW_VERSION}" > RELEASE_NOTES.md
            gh release create "v${NEW_VERSION}" \
              --title "Release v${NEW_VERSION}" \
              --notes-file RELEASE_NOTES.md
```
## Key Patterns

  - Quality Gates: Tests → Formatting → Type Check → Linting (enforced in order)
  - Workflow Dependencies: Release only runs after successful CI (workflow_run)
  - Permissions: Explicit contents: write required for release workflows
  - Pre-build Strategy: CI tests use pre-built binaries to avoid compilation output interference
  - Quote-aware Parsing: Handle content="value" parameters correctly in Justfiles
  - Template Substitution: Support both {{ param }} and {{param}} formats
  - Conventional Commits: Required for automated cocogitto releases (historical commits may need handling)



## Lowering Logic — Rust Trait System as First-Order Logic

The Rust trait system maps cleanly onto first-order Horn clauses (and their FOHH extension).

**Core mapping:**
```prolog
% trait Clone {} + impl Clone for usize {}
Clone(usize).
% impl<T> Clone for Vec<T> where T: Clone {}
Clone(Vec<?T)) :- Clone(?T).
```

Proof search is backward chaining: to prove `Clone(Vec<Vec<usize>>)`, recurse until base fact `Clone(usize)`.

**Why Horn clauses aren't enough** — generic function type-checking requires FOHH (First-Order Hereditary Harrop):
```prolog
% fn foo<T: Eq<T>>() { bar::<T>() }
fooTypeChecks :-
  forall<T> { if (Eq(T, T)) { barWellFormed(T) } }.
```
Standard Horn clauses forbid `forall`/`if` in goal position; FOHH allows it. See: Gopalan Nadathur "A Proof Procedure for the Logic of Hereditary Harrop Formulas" (Chalk Book bibliography).

**Crate landscape for trait lowering:**
| Crate | Role | Fit |
|---|---|---|
| `chalk` | rustc's actual FOHH trait solver | ✅ exact fit, complex API |
| `datafrog` | Datalog semi-naive eval (used in Polonius/borrow checker) | ✅ Horn-only, very fast |
| `scryer-prolog` | WAM Prolog engine, FOHH-capable | ✅ more stable than foras |
| `foras` | FOL reasoner wrapping scryer-prolog | ⚠️ PoC only — 0 stars, undergrad project |
| `syn` | Rust AST parser | required — source of trait/impl declarations |

**Typical lowering pipeline:**
```
syn AST (trait/impl decls)
  → Horn clause encoding (subject: impl, predicate: where-bounds)
  → datafrog (Datalog/Horn) or scryer-prolog (full FOHH)
  → proof trace / entailment result
```

🤓 `foras` wraps `scryer-prolog` with `Formula::parse()` API but adds thin value; for Rust lowering you write the `syn`→FOL bridge either way. Prefer `chalk` (exact match) or `datafrog` (simpler, Datalog subset) over `foras` for production use.
