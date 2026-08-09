---
description: Show b00t hive status — git, zellij, governance gates, cake balance, open issues
allowed-tools: Bash(just b00t::slash-b00t:*)
---

# /b00t

🥾 Bridges this repo's already-existing `just b00t::slash-b00t` recipe
(`_b00t_/scripts/slash-b00t.sh`) into Claude Code's native slash-command
surface, so operators get hive status without leaving chat (see issue #787).

Run the recipe and report its output verbatim — do not summarize or omit
sections (git, zellij, governance gates, cake balance, open issues, exercise
reminder):

!`just b00t::slash-b00t`
