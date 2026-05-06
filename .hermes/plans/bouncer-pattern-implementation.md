# Bouncer Pattern Implementation Plan

## Overview
Implement a bouncer pattern gatekeeper system for b00t hive agents.
The bouncer pattern enforces input/output validation, constraint enforcement,
and audit trail logging for all agent operations.

## Architecture
```
caller → bouncer (input gates) → implementation → bouncer (output gates) → caller
```

### Bouncer Components
1. **Input Gates**: sanitize, credential-check, permission-check, rate-limit
2. **Output Gates**: contract-validation, security-scan, quality-check
3. **Audit Trail**: JSONL log of all gate decisions
4. **Validation Layer**: TOML-based gate configuration

## Implementation Tasks

### Task 1: Bouncer Core Library (Rust)
**Agent**: alpha (specialist, Rust/testing)
**Cognitive Tier**: ch0nky
**Priority**: P0

- Create `b00t-bouncer` crate in workspace
- Implement `Bouncer` struct with gate pipeline
- Define gate traits: `InputGate`, `OutputGate`, `AuditLogger`
- Add TOML configuration for gate rules
- Write unit tests for each gate type

**Tests**:
- `test_input_gates` — sanitize, credential-check, permission-check, rate-limit
- `test_output_gates` — contract-validation, security-scan, quality-check
- `test_gate_pipeline` — full pipeline with pass/fail scenarios
- `test_audit_logging` — JSONL output format validation

### Task 2: Bouncer CLI Integration
**Agent**: beta (specialist, Docker/deploy)
**Cognitive Tier**: ch0nky
**Priority**: P0

- Integrate bouncer into `b00t-cli` binary
- Add `b00t bouncer` subcommand
- Implement `b00t bouncer validate <input>` command
- Implement `b00t bouncer audit` command
- Add gate configuration to `b00t` config

**Tests**:
- `test_bouncer_cli_validate` — CLI input validation
- `test_bouncer_cli_audit` — CLI audit log output
- `test_bouncer_config` — TOML config parsing

### Task 3: Agent Datum Bouncer Integration
**Agent**: executive (captain, strategic)
**Cognitive Tier**: frontier
**Priority**: P1

- Update `++abstract.agent.tomllm` with bouncer fields
- Add `[[b00t.agent.bouncer]]` section to all agent datums
- Implement bouncer initialization in agent startup
- Add bouncer audit logging to agent operations

**Tests**:
- `test_agent_bouncer_init` — agent starts with bouncer enabled
- `test_agent_bouncer_gates` — gates enforce constraints
- `test_agent_bouncer_audit` — audit trail captured

### Task 4: Bouncer Validation Tests
**Agent**: scoring (stateless scoring)
**Cognitive Tier**: sm0l
**Priority**: P1

- Write integration tests for bouncer pattern
- Test input/output validation with real agent datums
- Test audit trail format and completeness
- Test gate failure scenarios

**Tests**:
- `test_integration_input_validation` — real input data
- `test_integration_output_validation` — real output data
- `test_integration_audit_trail` — JSONL format validation
- `test_integration_gate_failure` — failure handling

## Agent Delegation Plan

| Task | Agent | Tier | Parallel? |
|---|---|---|---|
| Task 1: Core Library | alpha | ch0nky | Yes |
| Task 2: CLI Integration | beta | ch0nky | Yes |
| Task 3: Agent Integration | executive | frontier | No (depends on 1,2) |
| Task 4: Validation Tests | scoring | sm0l | No (depends on 1,2,3) |

## Bouncer Gate Configuration

```toml
[b00t.bouncer]
enabled = true
audit_log = ".b00t/bouncer-audit.jsonl"

[b00t.bouncer.input_gates]
sanitize = { enabled = true, rules = ["no-shell-injection", "no-credential-exposure"] }
credential-check = { enabled = true, check_env = true }
permission-check = { enabled = true, check_role = true }
rate-limit = { enabled = true, max_concurrent = 10 }

[b00t.bouncer.output_gates]
contract-validation = { enabled = true, validate_schema = true }
security-scan = { enabled = true, check_secrets = true }
quality-check = { enabled = true, min_quality_score = 0.8 }
```

## Quality Gates

1. **All tests MUST pass** — no untested code committed
2. **Bouncer gates MUST be enabled** — no bypass allowed
3. **Audit trail MUST be complete** — every gate decision logged
4. **Documentation MUST be updated** — all changes documented

## Commit Strategy

- Each task completed → commit with bouncer validation
- All tasks → integration test → final commit
- Bouncer approves all commits → no manual review needed

## Success Criteria

- ✅ `cargo test` passes for all crates
- ✅ `b00t bouncer validate` works correctly
- ✅ All agent datums have bouncer configuration
- ✅ Audit trail is complete and valid JSONL
- ✅ No gate bypasses possible
- ✅ Documentation updated

<!-- b00t:map v1
summary: Bouncer pattern implementation plan — core library, CLI integration, agent datum integration, validation tests
tags: bouncer, pattern, implementation, testing, validation, audit, gates
tier: frontier
cmds: b00t bouncer validate, b00t bouncer audit, cargo test
complexity: 8
-->\n
