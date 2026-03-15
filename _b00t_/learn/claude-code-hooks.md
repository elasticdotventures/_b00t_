# Claude Code Hooks — b00t Reference

> Source: https://code.claude.com/docs/en/hooks

## Hook Types

| type | mechanism | blocks? |
|------|-----------|---------|
| `command` | shell script, JSON on stdin, exit code controls | yes (exit 2) |
| `http` | POST to endpoint, event JSON as body | yes (2xx + JSON decision) |
| `prompt` | Claude eval → `{"ok": bool, "reason": "..."}` | yes |
| `agent` | subagent with tools (Read/Grep/Glob) | yes |

## Lifecycle Events

### Blocking (exit 2 / `decision: "block"` stops the action)
| Event | Fires when | Matcher support |
|-------|-----------|----------------|
| `UserPromptSubmit` | user hits enter | no |
| `PreToolUse` | before tool runs | tool name regex |
| `PermissionRequest` | permission dialog | tool name regex |
| `Stop` | claude finishes responding | no |
| `SubagentStop` | subagent finishes | agent type regex |
| `TeammateIdle` | agent going idle | no |
| `TaskCompleted` | task marked done | no |
| `ConfigChange` | config file changes | source name |
| `WorktreeCreate` | worktree being created | no |
| `Elicitation` | MCP server requests user input | MCP server name |
| `ElicitationResult` | user responds to MCP prompt | MCP server name |

### Non-blocking (stderr shown, action already happened)
| Event | Fires when | Matcher support |
|-------|-----------|----------------|
| `SessionStart` | session begins/resumes | source: startup/resume/clear/compact |
| `SessionEnd` | session terminates | exit reason |
| `PostToolUse` | after tool succeeds | tool name regex |
| `PostToolUseFailure` | after tool fails | tool name regex |
| `Notification` | notification sent | notification type |
| `SubagentStart` | subagent spawned | agent type regex |
| `InstructionsLoaded` | CLAUDE.md loaded | no |
| `PreCompact` | before context compaction | manual/auto |
| `PostCompact` | after context compaction | manual/auto |

## Config Locations (priority order, lower wins)

```
~/.claude/settings.json              # user scope (all projects)
.claude/settings.json                # project scope (shareable)
.claude/settings.local.json          # project scope (gitignored)
plugin hooks/hooks.json              # plugin scope
skill/agent frontmatter              # component lifetime only
```

## Config Structure

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash|Edit|Write",
        "hooks": [
          { "type": "command", "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh", "timeout": 30 }
        ]
      }
    ]
  }
}
```

## Exit Code Protocol (command hooks)

- `exit 0` + stdout JSON → success, JSON parsed for decisions
- `exit 2` → blocking: stderr fed to Claude; tool blocked, prompt rejected
- `exit 1` (other) → non-blocking: stderr shown in verbose mode only

## JSON Output Schema (stdout, exit 0)

```json
{
  "continue": true,
  "stopReason": "reason shown when continue=false",
  "suppressOutput": false,
  "systemMessage": "warning shown to user"
}
```

## Per-Event Decision Fields

### PreToolUse — allow/deny/ask + input mutation
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow|deny|ask",
    "permissionDecisionReason": "...",
    "updatedInput": { "command": "sanitized_cmd" },
    "additionalContext": "injected context for Claude"
  }
}
```

### UserPromptSubmit / PostToolUse / Stop / ConfigChange — block
```json
{ "decision": "block", "reason": "explanation" }
```

### WorktreeCreate — print absolute path to stdout
```bash
echo "/absolute/path/to/new/worktree"
```

## Common stdin fields (all events)
```json
{
  "session_id": "...",
  "transcript_path": "/path/to/transcript.jsonl",
  "cwd": "/project/root",
  "permission_mode": "default|plan|acceptEdits|dontAsk|bypassPermissions",
  "hook_event_name": "PreToolUse",
  "agent_id": "subagent-id (only inside subagent)",
  "agent_type": "Explore (only inside subagent)"
}
```

## SubagentStop extra fields
```json
{
  "agent_id": "def456",
  "agent_type": "Explore",
  "agent_transcript_path": "~/.claude/projects/.../subagents/agent-def456.jsonl",
  "last_assistant_message": "Analysis complete..."
}
```

## Native Tool Names (PreToolUse matcher)
`Bash`, `Write`, `Edit`, `Read`, `Glob`, `Grep`, `WebFetch`, `WebSearch`, `Agent`

MCP tools: `mcp__<server>__<tool>` — match with `mcp__memory__.*` etc.

## SessionStart — env var injection
```bash
if [ -n "$CLAUDE_ENV_FILE" ]; then
  echo 'export B00T_ROLE=executive' >> "$CLAUDE_ENV_FILE"
fi
```

## Hooks in skill/agent frontmatter
```yaml
---
name: security-reviewer
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: ".claude/hooks/security-check.sh"
          once: false   # once: true = run once per session only
---
```

## Hook composition / deduplication
- All matching hooks at same scope run **in parallel**
- Command hooks deduplicated by command string
- HTTP hooks deduplicated by URL
- Identical handlers run only once even if matched by multiple matchers

## Key env vars in hook scripts
- `$CLAUDE_PROJECT_DIR` — project root
- `${CLAUDE_PLUGIN_ROOT}` — plugin root
- `$CLAUDE_ENV_FILE` — (SessionStart only) append exports here
- `$CLAUDE_CODE_REMOTE` — "true" in web/remote environments

## b00t design notes
- `dispatch.sh` should read `agent_type` from stdin to select role hooks
- `SessionStart` with `CLAUDE_ENV_FILE` is the right place to inject role env
- `SubagentStart` + `SubagentStop` are the lifecycle anchors for agent sessions
- Skill frontmatter hooks are scoped — clean separation from global hooks
- Hook deduplication means b00t dispatcher can safely register for all events
- `once: true` in frontmatter hooks = useful for one-time session setup
