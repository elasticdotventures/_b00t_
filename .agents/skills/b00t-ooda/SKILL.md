---
name: b00t-ooda
description: Run an autonomous OODA (Observe-Orient-Decide-Act) loop via ralph to execute task queues with a chosen AI executor. Use when asked to run autonomously, start an agent loop, execute pending tasks without supervision, or run OODA cycles. Activates on "run ooda", "autonomous loop", "start ralph", "execute tasks with pi/opencode/claude".
license: MIT
---

Launch a ralph OODA loop to execute tasks autonomously:

```bash
b00t ooda run                           # claude executor, 5 iterations
b00t ooda run --agent=pi --max-iter=10  # local ch0nky via pi-coding-agent
b00t ooda run --agent=opencode          # opencode executor
b00t ooda run --task=42                 # target specific task ID
b00t ooda run --dry-run                 # preview command without executing
b00t ooda status                        # task queue summary
b00t ooda phase                         # current OodaPhase from last run
```

Executor tiers:
- `claude` — frontier (claude-sonnet); best reasoning, highest cost
- `opencode` — ch0nky (qwen36-local when inference running); code-focused
- `pi` — ch0nky (llama-cpp); local, non-interactive, fastest for simple tasks
- `amp` — Amplitude agent; parallel tool use
- `codex` — OpenAI Codex; sandboxed execution

OODA task dispatch (title prefix → recipe):
- `review-soul: <T>` → `just review-soul topic=<T>`
- `research-soul: <T>` → `just research-soul topic=<T>`
- user story text → standard story implementation cycle

The loop terminates when the executor emits `<promise>COMPLETE</promise>`
or all pending tasks are done.
