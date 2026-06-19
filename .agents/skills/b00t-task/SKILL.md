---
name: b00t-task
description: Manage the b00t task queue — add, list, pick next, and mark tasks done. Use when asked to track work, add a bug/feature/research task, check what's next, or report task completion. Activates on "add task", "next task", "mark done", "task queue", "what should I work on".
license: MIT
---

b00t task management (native, replaces taskmaster-ai):

```bash
b00t task list                  # show all tasks with status
b00t task next                  # next pending task (priority ordered)
b00t task add "<title>"         # add task (title prefixes matter)
b00t task done <id>             # mark task complete
```

Title prefix conventions — determines how OODA executors handle the task:
- `bug: <desc>` — bug fix; executor writes test first, then fix
- `feat: <desc>` — new feature; TDD cycle
- `research-soul: <topic>` — triggers `just research-soul topic=<topic>`
- `review-soul: <topic>` — triggers `just review-soul topic=<topic>` (pi review)
- `gh#<N>: <desc>` — GitHub issue reference

Always branch before starting a task:
```bash
git checkout -b task/<id>-<slug> main
b00t task done <id>   # when tests pass and commit is clean
```

Tasks live in `.b00t/tasks.json` — committed to repo for hive-wide visibility.
