# B00t Task Tracking Design

## Problem Statement

TaskMaster-AI has grown complex with unwanted dependencies. Need lightweight task tracking aligned with b00t datum ontology and snarktank/ralph simplicity.

## Existing Capabilities

### b00t Job System (Already Exists!)

**Location**: `b00t-cli/src/commands/job.rs`

**Features**:
- DAG workflow execution (sequential, parallel, dag modes)
- Git-based checkpoints after each step
- Step state persistence (`.b00t/jobs/`)
- Resume from checkpoint support
- Task types: bash, agent, k0mmander, datum, mcp, dagu
- Rollback on failure
- Environment variables
- Timeout handling

**Datum Format**: `.job.toml`
```toml
[b00t]
name = "build-release"
type = "job"

[b00t.job]
description = "Build and release workflow"
tags = ["build", "release"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "auto"
checkpoint_after_each_step = true

[[b00t.job.steps]]
name = "tests"
description = "Run test suite"
depends_on = []
checkpoint = "tests-passed"

[b00t.job.steps.task]
type = "bash"
command = "cargo test --workspace"
```

**CLI Commands**:
```bash
b00t job list
b00t job plan <name>
b00t job run <name> [--from-step X] [--resume]
b00t job status [--all]
b00t job checkpoints
```

## Claude Skills Pattern

**Key Insights** (from Simon Willison + Claude blog):

1. **Progressive Disclosure**: Metadata (~100 tokens) → Instructions (<5k tokens) → Files/scripts (on-demand)
2. **Composability**: Folder-based organization, mix & match
3. **Simplicity**: Markdown + scripts, no complex protocols
4. **Token Efficiency**: Better than MCP's heavy tool definitions

**Structure**:
```
skills/
├── prd-to-job/
│   ├── skill.md          # Instructions
│   ├── example.prd.txt   # Sample input
│   └── template.job.toml # Output template
```

## snarktank/ralph Simplicity

**What Worked**:
- Simple JSON task format (prd.json)
- Git commits for persistence
- `progress.txt` for learnings between iterations
- Minimal dependencies: bash, jq, git

**What to Keep**:
- File-based state (not databases)
- Git as source of truth
- Progress tracking for agent learning
- Right-sized task decomposition

## Proposed Solution

### Replace TaskMaster-AI with B00t Job System

**Why?**
- ✅ Already implemented in b00t-cli
- ✅ Datum-based (fits b00t ontology)
- ✅ Git checkpoints (snarktank/ralph pattern)
- ✅ No external dependencies
- ✅ DAG support for complex workflows
- ✅ Resume from checkpoint (ralph pattern)

### Add Skill Datum Type

**New datum type**: `.skill.toml`

```toml
[b00t]
name = "prd-to-job"
type = "skill"
hint = "Generate job workflows from PRD documents"

[b00t.skill]
description = "Converts Product Requirements Documents to executable job workflows"
instructions_file = "prd-to-job.md"
examples = ["example.prd.txt", "example-output.job.toml"]
tags = ["workflow", "planning"]

[b00t.skill.metadata]
# Progressive disclosure - loaded first (~100 tokens)
applies_to = ["workflow planning", "task generation", "prd conversion"]
output_types = [".job.toml"]
dependencies = []
```

### Implement /prd Skill

**File**: `_b00t_/skills/prd-to-job.md`

```markdown
# PRD to Job Workflow Converter

## Purpose
Generate executable `job.toml` workflows from Product Requirements Documents.

## Input Format
Plain text or markdown PRD with:
- User stories ("As a X, I need Y, so that Z")
- Acceptance criteria
- Dependencies (optional)

## Output Format
B00t job definition (`.job.toml`) with:
- Sequential/DAG mode based on dependencies
- Steps derived from user stories
- Checkpoints after each story completion
- Bash/agent tasks for implementation

## Process
1. Parse PRD into user stories
2. Extract dependencies from story descriptions
3. Generate `job.toml` with appropriate mode:
   - No dependencies → sequential mode
   - Dependencies specified → DAG mode
4. Add checkpoints for each story
5. Include acceptance criteria in step descriptions

## Example Transformation

**Input** (prd.txt):
```
User Story 1: Setup database schema
- Create users table
- Add indexes

User Story 2: Implement auth (depends on Story 1)
- JWT token generation
- Login endpoint
```

**Output** (auth-feature.job.toml):
```toml
[b00t.job.config]
mode = "dag"

[[b00t.job.steps]]
name = "setup-database"
description = "Setup database schema"
checkpoint = "db-ready"
[b00t.job.steps.task]
type = "bash"
command = "just db:migrate"

[[b00t.job.steps]]
name = "implement-auth"
depends_on = ["setup-database"]
checkpoint = "auth-complete"
[b00t.job.steps.task]
type = "agent"
agent_type = "codex"
prompt = "Implement JWT auth with login endpoint"
```
```

### Update Ralph Integration

**Replace**: ralph → taskmaster delegation
**With**: ralph → b00t job execution

```rust
// b00t-cli/src/commands/agent.rs
async fn handle_ralph(...) {
    // Instead of: uv run ralph --tool codex
    // Use: b00t job run hive-validate --resume

    let job_file = ensure_job_exists(&root, task)?;

    cmd!(
        "b00t-cli",
        "job",
        "run",
        job_file,
        "--resume"
    )
    .dir(&root)
    .run()?;
}
```

**Hive Validation Job**: `_b00t_/hive-validate.job.toml`
```toml
[b00t]
name = "hive-validate"
type = "job"

[b00t.job]
description = "Validate b00t hive system health"

[[b00t.job.steps]]
name = "check-submodules"
checkpoint = "submodules-ok"
[b00t.job.steps.task]
type = "bash"
command = """
git submodule update --init --recursive
git submodule status | grep -v '^-' || exit 1
"""

[[b00t.job.steps]]
name = "check-rust-toolchain"
depends_on = ["check-submodules"]
checkpoint = "rust-ok"
[b00t.job.steps.task]
type = "bash"
command = "rustc --version && cargo --version"

[[b00t.job.steps]]
name = "validate-datums"
depends_on = ["check-rust-toolchain"]
checkpoint = "datums-ok"
[b00t.job.steps.task]
type = "agent"
agent_type = "codex"
prompt = "Validate all TOML files in _b00t_/ have required fields and valid syntax"
```

## Benefits

1. **No External Dependencies**: Use b00t's existing job system
2. **Datum Ontology Aligned**: `.job.toml` already part of b00t
3. **Git-Based State**: Checkpoints persist via git (snarktank pattern)
4. **Resume Support**: Pick up where you left off
5. **Claude Skills Compatible**: Progressive disclosure via skill datums
6. **Token Efficient**: Skills load on-demand, not upfront
7. **Composable**: Mix job workflows + skills + MCP tools

## Implementation Steps

1. ✅ B00t job system exists
2. Create `.skill.toml` datum type
3. Implement skill loader in b00t-cli
4. Create `/prd` skill (`_b00t_/skills/prd-to-job/`)
5. Update ralph integration to use jobs
6. Create hive-validate.job.toml
7. Test workflow: PRD → job.toml → execution → checkpoints

## Migration Path

**From**: taskmaster-ai (b00t-wiggums dependency)
```
PRD → taskmaster init → tasks.json → ralph loop
```

**To**: b00t jobs + skills
```
PRD → /prd skill → job.toml → b00t job run → git checkpoints
```

**Compatibility**: Ralph can still run, but delegates to b00t job system instead of taskmaster-ai.

---

🥾 Generated via b00t gospel alignment
