---
name: ralph-prd
description: "Generate a Ralph backlog for a new feature. Use when planning a feature, starting a new project, or when asked to create tasks. Triggers on: create tasks, write prd for, plan this feature, requirements for, spec out, generate TODO-next.md, use the ralph-prd skill."
user-invocable: true
---

# Ralph Backlog Generator

Create detailed task lists for `TODO-next.md` that are clear, actionable, and suitable for autonomous Ralph execution.

---

## The Job

1. Receive a feature description from the user
2. Ask 3-5 essential clarifying questions (with lettered options)
3. Generate a structured markdown checklist backlog based on answers
4. Save to `TODO-next.md`
5. Optionally save a markdown PRD to `tasks/prd-[feature-name].md` for documentation

**Important:** Do NOT start implementing. Just create the backlog file.

---

## Step 1: Clarifying Questions

Ask only critical questions where the initial prompt is ambiguous. Focus on:

- **Problem/Goal:** What problem does this solve?
- **Core Functionality:** What are the key actions?
- **Scope/Boundaries:** What should it NOT do?
- **Success Criteria:** How do we know it's done?

### Format Questions Like This:

```
1. What is the primary goal of this feature?
   A. Improve user onboarding experience
   B. Increase user retention
   C. Reduce support burden
   D. Other: [please specify]

2. Who is the target user?
   A. New users only
   B. Existing users only
   C. All users
   D. Admin users only

3. What is the scope?
   A. Minimal viable version
   B. Full-featured implementation
   C. Just the backend/API
   D. Just the UI
```

This lets users respond with "1A, 2C, 3B" for quick iteration.

---

## Step 2: Backlog Output

After gathering answers, generate `TODO-next.md` with this structure:

```markdown
# Next
- [ ] [Short imperative title] -- [acceptance criteria summary]
- [ ] [Second task] -- [acceptance criteria summary]
```

### Task Size Rule
Each task MUST be completable in one Ralph iteration (one context window). If you cannot describe the task in 2-3 sentences, split it into smaller tasks.

### Dependency Tracking
- **dependsOn**: Array of task IDs that this task logically builds on (for documentation)
- **blockedBy**: Array of task IDs that MUST complete before this one can start (enforced by Ralph)

---

## Step 3: Optional Markdown PRD

Generate the PRD with these sections:

### 1. Introduction/Overview
Brief description of the feature and the problem it solves.

### 2. Goals
Specific, measurable objectives (bullet list).

### 3. User Stories
Each story needs:
- **Title:** Short descriptive name
- **Description:** "As a [user], I want [feature] so that [benefit]"
- **Acceptance Criteria:** Verifiable checklist of what "done" means

Each story should be small enough to implement in one focused session.

**Format:**
```markdown
### US-001: [Title]
**Description:** As a [user], I want [feature] so that [benefit].

**Acceptance Criteria:**
- [ ] Specific verifiable criterion
- [ ] Another criterion
- [ ] Typecheck/lint passes
- [ ] **[UI stories only]** Verify in browser using dev-browser skill
```

**Important:** 
- Acceptance criteria must be verifiable, not vague. "Works correctly" is bad. "Button shows confirmation dialog before deleting" is good.
- **For any story with UI changes:** Always include "Verify in browser using dev-browser skill" as acceptance criteria. This ensures visual verification of frontend work.

### 4. Functional Requirements
Numbered list of specific functionalities:
- "FR-1: The system must allow users to..."
- "FR-2: When a user clicks X, the system must..."

Be explicit and unambiguous.

### 5. Non-Goals (Out of Scope)
What this feature will NOT include. Critical for managing scope.

### 6. Design Considerations (Optional)
- UI/UX requirements
- Link to mockups if available
- Relevant existing components to reuse

### 7. Technical Considerations (Optional)
- Known constraints or dependencies
- Integration points with existing systems
- Performance requirements

### 8. Success Metrics
How will success be measured?
- "Reduce time to complete X by 50%"
- "Increase conversion rate by 10%"

### 9. Open Questions
Remaining questions or areas needing clarification.

---

## Writing for Junior Developers

The PRD reader may be a junior developer or AI agent. Therefore:

- Be explicit and unambiguous
- Avoid jargon or explain it
- Provide enough detail to understand purpose and core logic
- Number requirements for easy reference
- Use concrete examples where helpful

---

## Output Files

### Required:
- **Backlog**: `TODO-next.md` (markdown checklist)

### Optional (for documentation):
- **PRD markdown**: `tasks/prd-[feature-name].md` (human-readable)

---

## Example Output

### TODO-next.md (Required)

```markdown
# Next
- [ ] Add priority field to database -- migration runs; Typecheck passes
- [ ] Display priority badge on task cards -- badge colors visible; Typecheck passes; Verify in browser using dev-browser skill
- [ ] Add priority selector to task edit -- selection saves; Typecheck passes; Verify in browser using dev-browser skill
- [ ] Filter tasks by priority -- filter persists in URL params; Typecheck passes; Verify in browser using dev-browser skill
```

---

### prd-task-priority.md (Optional Documentation)

```markdown
# PRD: Task Priority System

[... Keep existing PRD example for reference ...]
```

---

## Checklist

Before saving `TODO-next.md`:

- [ ] Asked clarifying questions with lettered options
- [ ] Incorporated user's answers
- [ ] Each task is small enough (completable in one iteration)
- [ ] Tasks ordered by dependency (schema → backend → UI)
- [ ] Priority numbers match dependency order
- [ ] Every task has verifiable acceptance criteria
- [ ] Every task has "Typecheck passes" as final criterion
- [ ] UI tasks have "Verify in browser using dev-browser skill"
- [ ] Dependencies are documented inline or in the PRD when needed
- [ ] Saved to `TODO-next.md`
