---
name: next-task
description: End-to-end task pipeline. Discovers the next task, plans, implements, reviews with certainty-graded quality gates, then ships. Resumable via git state.
disable-model-invocation: false
---

# /next-task Workflow Orchestration

Gated pipeline: DISCOVER → PLAN → IMPLEMENT → REVIEW → SHIP.
State persists in git (branch + commit messages). Each phase can resume independently.

## Phases

### Phase 1: DISCOVER

1. Check taskmaster-ai for next prioritized task. # output: task_id, task_description
2. If no taskmaster-ai, check git issues: `gh issue list --label next-task --limit 1` # output: issue_id
3. If none, ask Operator to specify. # output: task_clarified
4. Confirm task scope with Operator before proceeding. ← **HUMAN GATE**
5. Create isolated branch: `git checkout -b task/<id>-<slug>` # output: branch_name

### Phase 2: PLAN

1. Map affected files and dependencies using repo structure. # output: affected_files[]
2. Identify risks (breaking changes, security, performance). # output: risk_assessment
3. Write implementation plan (steps, test strategy, rollback). # output: plan_md
4. Apply certainty-grade to each plan step. # output: graded_plan
5. Present plan to Operator. ← **HUMAN GATE**

### Phase 3: IMPLEMENT

1. Write failing tests first (TDD). # output: test_files[]
2. Implement minimum code to pass tests. # output: changed_files[]
3. Run code-quality skill on all changed files. # output: quality_report
4. AUTO-FIX all HIGH certainty findings. # output: auto_fix_log
5. Surface MEDIUM findings to Operator. ← **HUMAN GATE (if MEDIUM findings exist)**
6. Commit: `git commit -m "feat: <description>"` # output: commit_sha

### Phase 4: REVIEW

1. Run full test suite. # output: test_results (PASS|FAIL)
2. If FAIL: diagnose, fix, re-run. Loop max 3x before escalating to Operator.
3. Run code-quality on full diff. # output: quality_report
4. Check for documentation drift (code changed without doc update). # output: drift_findings
5. All LOW certainty findings → Operator review. ← **HUMAN GATE**
6. Confirm ready to ship. ← **HUMAN GATE**

### Phase 5: SHIP

1. Push branch: `git push -u origin <branch>` # output: remote_branch
2. Create PR: `gh pr create --title "<task>" --body "<plan_summary>"` # output: pr_url
3. Monitor CI: poll `gh run list --branch <branch>` every 30s. # output: ci_status
4. If CI fails: check logs `gh run view --log-failed`, diagnose, fix, push. Max 2 retries.
5. If CI passes: request merge or auto-merge if authorized. # output: merge_sha
6. Cleanup: delete branch after merge. # output: DONE

## State & Resumability

State encoded in git:
- Current phase → branch name prefix (`task/<id>-<slug>`)
- Plan → first commit message body
- Quality report → `.b00t/quality-report.md` (gitignored, ephemeral)
- CI status → `gh run list` output

Resume interrupted task:
```
/next-task --resume <branch_name>
```
Detects current phase from branch + commit state, continues from last gate.

## Flags

```
/next-task                    # Auto-discover next task
/next-task --task <id>        # Specific task by id
/next-task --resume <branch>  # Resume interrupted pipeline
/next-task --phase <phase>    # Start from specific phase
/next-task --skip-gates       # ⚠️ Skip human gates (CI/CD use only)
```

## Gate Summary

| Gate | Trigger | Required |
|------|---------|---------|
| Task confirmation | After DISCOVER | Always |
| Plan approval | After PLAN | Always |
| MEDIUM quality findings | During IMPLEMENT | If MEDIUM findings exist |
| LOW quality findings | During REVIEW | If LOW findings exist |
| Ship confirmation | Before SHIP | Always (unless --skip-gates) |

## Integration

- Uses `certainty-grade` skill for all findings
- Uses `code-quality` skill at IMPLEMENT and REVIEW phases
- Uses `b00t learn` for task-relevant topics before IMPLEMENT
- Uses taskmaster-ai MCP when available
- Falls back to gh issues if taskmaster-ai unavailable
