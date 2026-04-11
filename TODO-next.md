# b00t Gap-Fill Backlog (OpenHarness analysis)
# Ref: elasticdotventures/_b00t_#343
# Branch: feat/adversarial-gemma4-epiphany
# Tool: pi (gemma4 ch0nky on :8001)

## HIGH PRIORITY

### H1: Automated regression suite for b00t-cli
**Acceptance criteria:**
- MUST add `tests/integration/` directory under `b00t-cli/` with at least 3 integration tests
- Tests MUST cover: `b00t hive status`, `b00t whoami`, `b00t-cli up --help`
- MUST be runnable via `cargo test -p b00t-cli` without external services
- SHOULD use `assert_cmd` or equivalent for CLI process testing
- Tests MUST pass on current main branch baseline

### H2: Task-state checkpoint restore wiring
**Acceptance criteria:**
- MUST verify b00t.sh restore_task_state reads .b00t/ralph/task_state.json correctly
- MUST add a test: write mock task_state.json, run restore, verify tasks.json populated
- MUST document checkpoint format in _b00t_/ralph.cli.toml as epiphany
- SHOULD verify checkpoint written every loop iteration

## MEDIUM PRIORITY

### M1: pi --mode rpc systemd service wiring
**Acceptance criteria:**
- MUST verify pi --mode rpc flag exists (run: pi --help | grep rpc)
- IF exists: add b00t hive activate pi-agent smoke test to just validate-mcp
- IF missing: document gap in _b00t_/pi.agent.toml as epiphany
- MUST NOT block on missing functionality

### M2: Provider agnosticism documentation
**Acceptance criteria:**
- MUST add _b00t_/liter-llm-gateway.tomllm datum documenting :1234 gateway profiles
- MUST include: how to switch gemma4 direct (:8001) vs gateway (:1234)
- MUST include known working model aliases (ch0nky, sm0l)

## CONSTRAINTS
- NEVER use cloud inference (all work local gemma4 only)
- Keep diffs tight; run cargo test after each change
- EXIT_SIGNAL=true only when ALL high-priority items have passing tests
- Append friction report at END of session
