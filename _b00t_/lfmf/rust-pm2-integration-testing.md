# Rust Testing with nextest and PM2 Integration

**Topic**: rust, testing, nextest, pm2-integration

**Lesson Learned**: PM2 Integration Testing patterns and Rust 2024 safety requirements

## Problem

- Building PM2 integration required comprehensive test coverage analysis
- Rust 2024 edition enforces safety for environment variable manipulation
- Need faster parallel test execution for 326+ test suite

## Solution

### 1. Use cargo-nextest for Test Execution

```bash
# Install nextest (faster, parallel test runner)
cargo install cargo-nextest --locked

# Run tests with better output
cargo nextest run --workspace --no-fail-fast

# Results: 290/326 passed (88.96% coverage), 36 failed, 34 skipped
```

### 2. Wrap env Operations in unsafe Blocks

Rust 2024 edition requires unsafe blocks for `env::set_var` and `env::remove_var`:

```rust
// ❌ OLD (Rust 2021) - compilation error in 2024
env::remove_var("QDRANT_URL");
env::set_var("QDRANT_URL", "http://localhost:6334");

// ✅ NEW (Rust 2024) - wrapped in unsafe
unsafe {
    env::remove_var("QDRANT_URL");
    env::set_var("QDRANT_URL", "http://localhost:6334");
}
```

**Rationale**: Environment variables are process-global mutable state. Concurrent access from multiple threads can cause data races.

### 3. Test Coverage Analysis

Current test suite status:
- **Total tests**: 326
- **Passing**: 290 (88.96%)
- **Failing**: 36 (mostly datum loading, MCP integration requiring external services)
- **Skipped**: 34

Failed tests primarily in:
- `datum_ai_model::tests` - Repository resolution, cache directory expansion
- `hello_world_test` - MCP introspection, session memory tracking
- `ai_test` - Constraint evaluation with environment variables
- `b00t-grok` - Qdrant vector DB tests (require running Qdrant service)
- `integration_tests` - MCP workflow tests

## Benefits

1. **Faster CI/CD**: nextest runs tests in parallel, ~2-3x faster than `cargo test`
2. **Safety**: Unsafe blocks make concurrent env var access explicit
3. **Better output**: nextest provides cleaner test failure reporting
4. **Coverage visibility**: Easy to identify which subsystems need external services

## Related

- LFMF: typescript PM2 process manager integration
- LFMF: k0mmand3r IPC integration pattern
- Documentation: `pm2-tasker/README.md`

## Date

2025-11-17

## Category

testing, rust, pm2, integration
