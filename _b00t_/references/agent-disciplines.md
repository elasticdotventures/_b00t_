# Agent Disciplines

These are NOT guardable by code — they are execution disciplines the agent
must internalize. Unlike command guards that fire on pattern match, these
are principles the agent absorbs during planning and execution.

## Discipline 7: Precise Response

**Principle:** Every response must be precise — specifically address the
question, include concrete values/code/paths, and never default to vague
generalities.

### Why

Agents working in the b00t ecosystem operate on real files, real commands,
and real system state. A vague response costs the orchestrator $0.04/query
and provides zero value. A precise response saves 3-5 follow-up rounds.

### Concrete Rules

1. **Always include paths.** When you mention a file, give its absolute path.
   When you mention a command, show it as a runnable code block.

2. **Never say "appropriate" or "standard".** These are context-pressure
   markers. If you mean `clap::Parser` and `anyhow::Result`, say those names.
   If you mean `retry with exponential backoff`, say that explicitly.

3. **When answering "which" or "what", give the actual value.** Not "the
   guard module" but "b00t-cli/src/hive.rs:692-754 (check_guards function)".
   Not "the relevant function" but "GuardPattern::matches() at
   b00t-cli/src/hive.rs:524-554".

4. **When answering "how", give the exact steps.** Numbered lists with
   commands, expected outputs, and error recovery. Assume the reader will
   execute your steps literally.

5. **When verifying, confirm specific facts.** Not "the test passes" but
   "the test asserts `assert_eq!(result, 42)` on line 17 and passes with
   `cargo test verify_guard_escalation`".

6. **When uncertain, state what you don't know.** Not "let me check" but
   "I don't know whether the `repeat_threshold` field is optional or
   required in the TOML — checking b00t-cli/src/hive.rs:241-248 shows it
   uses `Option<u32>` so it defaults to None."

### Anti-Patterns

| Anti-Pattern | Instead |
|---|---|
| "Update the function" | "Add `repeat_threshold: Option<u32>` to HiveGuard at b00t-cli/src/hive.rs:175-184" |
| "The appropriate tool" | "Use `mcp_b00t_mcp_b00t_checkpoint` with `skip_tests=false`" |
| "Check the file" | "Read b00t-cli/src/hive.rs:500-555 to verify GuardPattern serialization" |
| "Handle the error" | "Wrap with `.context(\"loading hive profile\")?` and let anyhow propagate" |

### Self-Check

Before submitting a response, scan for these precision failures:

- Does any sentence contain "the" followed by a vague noun without a qualifier?
- Did I give a path or just a filename?
- Did I show the exact code/command or describe it?
- Could someone execute my instructions without guessing?
