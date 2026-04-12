---
name: ralph
description: "Convert PRDs to a Ralph backlog for the autonomous agent system. Use when you have an existing PRD and need to convert it to Ralph's current backlog format. Triggers on: convert this prd, turn this into ralph format, create TODO-next.md from this, ralph backlog."
user-invocable: true
---

# Ralph Backlog Converter

Converts existing PRDs to the `TODO-next.md` format that Ralph uses for autonomous execution. Legacy TaskMaster JSON MAY be emitted only when explicitly requested for compatibility.

---

## The Job

Take a PRD (markdown file or text) and convert it to `TODO-next.md` in the project root.

---

## Output Format

```markdown
# Next
- [ ] [Story title] -- [key acceptance criteria]
- [ ] [Next story title] -- [key acceptance criteria]
```

### Backlog Rules:

- Use short imperative task titles
- Put the most important acceptance signal after `--`
- Keep each line independently executable in one Ralph iteration
- Prefer natural ordering over explicit IDs unless legacy JSON output is requested

---

## Story Size: The Number One Rule

**Each story must be completable in ONE Ralph iteration (one context window).**

Ralph spawns a fresh Amp instance per iteration with no memory of previous work. If a story is too big, the LLM runs out of context before finishing and produces broken code.

### Right-sized stories:
- Add a database column and migration
- Add a UI component to an existing page
- Update a server action with new logic
- Add a filter dropdown to a list

### Too big (split these):
- "Build the entire dashboard" - Split into: schema, queries, UI components, filters
- "Add authentication" - Split into: schema, middleware, login UI, session handling
- "Refactor the API" - Split into one story per endpoint or pattern

**Rule of thumb:** If you cannot describe the change in 2-3 sentences, it is too big.

---

## Story Ordering: Dependencies First

Tasks execute in priority order. Earlier tasks must not depend on later ones.

**Correct order:**
1. Schema/database changes (migrations)
2. Server actions / backend logic
3. UI components that use the backend
4. Dashboard/summary views that aggregate data

**Wrong order:**
1. UI component (depends on schema that does not exist yet)
2. Schema change

---

## Dependency Tracking

If you need to capture dependencies explicitly, mention them inline in the task text or in the optional PRD.

**Example:**
```markdown
- [ ] Add status toggle to UI after schema + badge work lands -- Typecheck passes; Verify in browser using dev-browser skill
```

**When to use blockedBy:**
- Task B cannot compile/run without Task A completing first
- Task B would fail all tests without Task A's changes
- Task B modifies code that Task A creates

**When to use dependsOn only:**
- Task B logically builds on Task A but could theoretically be attempted
- Helps Ralph understand context but doesn't prevent execution
- Documents architectural relationships

---

## Acceptance Criteria: Must Be Verifiable

Each criterion must be something Ralph can CHECK, not something vague.

### Good criteria (verifiable):
- "Add `status` column to tasks table with default 'pending'"
- "Filter dropdown has options: All, Active, Completed"
- "Clicking delete shows confirmation dialog"
- "Typecheck passes"
- "Tests pass"

### Bad criteria (vague):
- "Works correctly"
- "User can do X easily"
- "Good UX"
- "Handles edge cases"

### Always include as final criterion:
```
"Typecheck passes"
```

For stories with testable logic, also include:
```
"Tests pass"
```

### For stories that change UI, also include:
```
"Verify in browser using dev-browser skill"
```

Frontend stories are NOT complete until visually verified. Ralph will use the dev-browser skill to navigate to the page, interact with the UI, and confirm changes work.

---

## Conversion Rules

1. **Each user story becomes one checklist item**
2. **Order matters**: earlier tasks SHOULD unblock later tasks
3. **Prefer concise titles** with the acceptance summary after `--`
4. **Always add**: `Typecheck passes` to every task summary
5. **Add** `Verify in browser using dev-browser skill` for UI work

---

## Splitting Large PRDs

If a PRD has big features, split them:

**Original:**
> "Add user notification system"

**Split into:**
1. US-001: Add notifications table to database
2. US-002: Create notification service for sending notifications
3. US-003: Add notification bell icon to header
4. US-004: Create notification dropdown panel
5. US-005: Add mark-as-read functionality
6. US-006: Add notification preferences page

Each is one focused change that can be completed and verified independently.

---

## Example

**Input PRD:**
```markdown
# Task Status Feature

Add ability to mark tasks with different statuses.

## Requirements
- Toggle between pending/in-progress/done on task list
- Filter list by status
- Show status badge on each task
- Persist status in database
```

**Output TODO-next.md:**
```markdown
# Next
- [ ] Add status field to tasks table -- migration runs; Typecheck passes
- [ ] Display status badge on task cards -- badge colors visible; Typecheck passes; Verify in browser using dev-browser skill
- [ ] Add status toggle to task list rows -- save works; tests pass; Verify in browser using dev-browser skill
- [ ] Filter tasks by status -- filter persists in URL params; Typecheck passes; Verify in browser using dev-browser skill
```

---

## Archiving Previous Runs

**Before writing a new backlog, check if there is an existing one from a different feature:**

1. Read the current `TODO-next.md` if it exists
2. Check if it is clearly for a different feature
3. If different AND `progress.txt` has content beyond the header:
   - Create archive folder: `archive/YYYY-MM-DD-feature-name/`
   - Copy current `TODO-next.md` and `progress.txt` to archive
   - Reset `progress.txt` with fresh header

**The ralph.sh script handles this automatically** when you run it, but if you are manually updating `TODO-next.md` between runs, archive first.

---

## Checklist Before Saving

Before writing `TODO-next.md`, verify:

- [ ] **Previous run archived** when replacing a backlog from a different feature
- [ ] Each task is completable in one iteration (small enough)
- [ ] Tasks are ordered by dependency (schema to backend to UI)
- [ ] Every task has "Typecheck passes" as criterion
- [ ] UI tasks have "Verify in browser using dev-browser skill" as criterion
- [ ] Acceptance criteria are verifiable (not vague)
- [ ] No task depends on a later task in the checklist
- [ ] Dependencies are noted inline or in the PRD when needed
