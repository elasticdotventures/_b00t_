#!/usr/bin/env bats

# claude-code-slash-commands.bats — proof-of-concept bridge from a real
# Claude Code custom slash command into b00t (#787).
#
# Claude Code discovers project-level slash commands as markdown files under
# .claude/commands/<name>.md (frontmatter + prompt body); typing "/<name>" in
# the chat runs the file's prompt. Before this change b00t had no such file,
# so the "/b00t slash command" referenced by _b00t_/scripts/slash-b00t.sh's
# own header comment was unreachable from Claude Code — the recipe existed,
# but nothing wired it to Claude Code's actual discovery mechanism.
#
# This test locks in the minimal PoC: a real .claude/commands/b00t.md file
# that shells out to the already-existing, already-tested
# `just b00t::slash-b00t` recipe.

load 'test_helper/bats-support/load'
load 'test_helper/bats-assert/load'

REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
COMMAND_FILE="$REPO_ROOT/.claude/commands/b00t.md"

@test "claude code command file .claude/commands/b00t.md exists" {
    [ -f "$COMMAND_FILE" ]
}

@test "command file has YAML frontmatter with a description" {
    run head -n 10 "$COMMAND_FILE"
    assert_success
    assert_line --index 0 "---"
    assert_output --partial "description:"
}

@test "command file body invokes the real just b00t::slash-b00t recipe" {
    run cat "$COMMAND_FILE"
    assert_success
    assert_output --partial "just b00t::slash-b00t"
}

@test "the recipe the command file shells out to actually exists in justfile" {
    run just --list b00t
    assert_success
    assert_output --partial "slash-b00t"
}

@test "the recipe the command file shells out to runs successfully end to end" {
    cd "$REPO_ROOT"
    run just b00t::slash-b00t
    assert_success
    assert_output --partial "b00t Status"
}
