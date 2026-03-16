# 🥾 b00t Repository Instructions for Copilot

## Repository Overview

This repository contains **b00t** - a universal agentic development framework. b00t is a context-aware hive operating system that bridges AI agents with real-world tooling through intelligent abstraction and unified tool discovery.

## Core Principles

### Code Philosophy
- **DRY (Don't Repeat Yourself)**: Never duplicate functionality that exists in open-source libraries
- **NRtW (Never Reinvent the Wheel)**: Always search for and leverage existing 3rd-party libraries before writing new code
- **KISS (Keep It Simple)**: Use idiomatic patterns and systems thinking
- **TDD/BDD**: Write tests first, then implement functionality

### Tech Stack
- 🦀 Rust (stable 1.82+) for core tooling
- 🐍 Python (3.12+) for scripting and integrations
- 🦄 TypeScript/JavaScript for web and MCP servers
- 🐧 Linux CLI tools with modern replacements (fd, rg, etc.)

## Required Practices

### Code Changes
- NEVER make changes directly to dev/main branch - ALWAYS use feature branches (`git checkout -b`)
- NEVER remove code without explicit instruction or user consent
- NEVER delete tests without triple justification using TRIZ rule of 3
- ALWAYS verify interfaces using Context7 and CrateDoc MCP tools before making changes
- ALWAYS use latest library versions and fortnightly releases

### Testing
- ALWAYS add unit/interface tests (TDD) before writing code
- ALWAYS write integration tests using BDD framework
- NEVER claim a problem is solved until tests verify it
- Data sets for tests stored in JSON files, never embedded in test code

### Documentation
- ALWAYS memoize key features in `justfile` using `casey/just` command runner
- Use `just -l` or `just-mcp` to list available commands
- Add 🤓 melvin comments for non-idiomatic tribal knowledge
- Each session adds at most ONE new 🤓 comment

### Tools & Commands
- Use `fdfind` instead of `find` (ignores .gitignore, faster)
- Use `rg` (ripgrep) for code search
- Disable pagers: `git --no-pager`, `less -F`, or pipe to `cat`
- Use MCP tools over bash when available (MCP is less costly)
- Use `b00t learn <skill>` to load context-specific knowledge

## b00t-Specific Patterns

### Learning System
```bash
b00t learn rust      # Load Rust development context
b00t learn docker    # Container orchestration knowledge
b00t learn bash      # Shell scripting patterns
```

### Tribal Knowledge
```bash
b00t lfmf <topic> "<lesson>"    # Record lessons learned
b00t advice <topic> "<query>"   # Get contextual debugging advice
```

### Installation & Tools
```bash
b00t cli install <tool>    # Install with dependency resolution
b00t cli check <tool>      # Check if tool is installed
b00t mcp install <server>  # Install MCP server
```

## Workflow

### Starting Work
1. Use `sequential-thinking` MCP to create a plan
2. Track progress with lightweight repo-native mechanisms (git issues, PR state, or `b00t` job checkpoints)
3. Verify assumptions from first principles
4. Check for existing libraries/solutions before coding
5. **If acting as executive orchestrator**: delegate sub-tasks to specialized agents (see Sub-Agent Delegation below)

### Sub-Agent Delegation (Executive Orchestrator Pattern)

Executive contexts are high-cost, high-intelligence models with limited token budgets. Delegate aggressively.

#### Cognitive Tier Routing
Route tasks to the cheapest tier that can handle them:

| Tier | Models | Tasks | Response contract |
|---|---|---|---|
| `sm0l` | qwen2.5-3B, claude-haiku | lint, classify, grep, format | `PASS` or `FAIL: <5-line excerpt>` |
| `ch0nky` | local qwen3-coder | implement, refactor, debug | diff + test result |
| `frontier` | claude-opus/sonnet | architecture, security, novel design | structured decision |

#### Delegating with b00t
```bash
# Load sub-agent skills before dispatching
b00t learn rust        # Load Rust context into sub-agent
b00t learn docker      # Container orchestration for sub-agent
# See orchestrator role docs for full orchestrator patterns

# Parallel task execution via b00t jobs
b00t job run parallel-tasks.job.toml   # DAG-mode parallel execution
b00t job run build-and-test.job.toml   # CI sub-task

# Launch specialist agents with skills
# See _b00t_/alpha.agent.toml, beta.agent.toml, executive.agent.toml
```

#### Sub-Agent Response Protocol
Instruct non-`sm0l` sub-agents to reply **laconically & fastidiously** to save executive context:
- ✅ `DONE: <1-line summary> | commit: <hash>`
- ❌ `FAIL: <error in ≤5 lines>`
- ⚠️ `BLOCKED: <dependency> missing`

For `sm0l`-tier agents, follow the routing-table contract instead:
- ✅ `PASS: <1-line summary>`
- ❌ `FAIL: <error in ≤5 lines>`

Never forward full sub-agent output to executive context — demand compressed summaries.

### During Development
1. Write tests first (TDD)
2. Implement minimal changes
3. Use existing libraries and patterns
4. Add 🤓 comments for non-obvious decisions
5. Update justfile for new features

### Before Completion
1. Run relevant tests
2. Verify all changes work as expected
3. Check git status and review changes
4. Ensure .gitignore excludes build artifacts

## Language-Specific Guidance

### Rust
- Use stable toolchain (1.82+)
- Follow idiomatic Rust patterns
- Use `Result<>` for error handling
- Leverage cargo workspaces for multi-crate projects

### Python
- Use Python 3.12+ features
- Type hints for all function definitions
- Prefer comprehensions over map/filter where readable
- Use modern package managers (uv, poetry)

### TypeScript
- Use strict type checking
- Prefer async/await over callbacks
- Use modern ESM imports
- Follow existing patterns in codebase

## Security & Privacy

- NEVER commit secrets into source code
- NEVER introduce new security vulnerabilities
- NEVER share sensitive data with 3rd party systems
- ALWAYS check dependencies for known vulnerabilities

## Communication Style

- Be laconic and precise (follow RFC 2119 word precision)
- Use technical language (presume technical literacy)
- Avoid platitudes and apologies
- Use emoji to save tokens: ✅ ❌ ⚠️ 🚩
- State "I don't know" when uncertain rather than speculating

## Additional Resources

See [AGENTS.md](mdc:/AGENTS.md) for comprehensive agent alignment protocols and b00t gospel.
See [dev_workflow.md](mdc:/.github/instructions/dev_workflow.md) for legacy Taskmaster task management reference (use b00t-native job/agent patterns for new work).
See [MULTI_AGENT_GOSPEL.md](mdc:/MULTI_AGENT_GOSPEL.md) for multi-agent coordination protocols (crew, IPC, cake economics).
