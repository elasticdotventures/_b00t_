---
name: b00t
description: Use when user asks for /b00t to send b00t learn content to a nominated agent.
disable-model-invocation: false
---

# /b00t Skill (Claude Plugin)

## Inputs
- --agent <id>
- --learn <csv>
- --msg <string?>

## Steps
1) Parse flags from the request. # output: agent_id, learn_topics[], msg
2) For each topic, run: b00t learn <topic> # output: rendered_learn_text
3) Build a JSON payload with agent, topics, rendered content, msg, caller. # output: payload_json
4) Try MCP first: mcp__b00t-mcp__b00t_agent_message <agent_id> "b00t/learn" <payload_json> # output: MCP ack or error
5) If MCP unavailable, fallback to CLI: b00t-cli agent message <agent_id> --subject "b00t/learn" --content '<payload_json>' # output: CLI ack or error
6) If CLI unavailable, fallback to ACP (experimental): publish to b00t:agent:<agent_id> with payload # output: ACP publish status
7) If a required tool is missing, request: b00t install tool <name> with justification and WAIT. # output: install request queued

## Output
- Confirm dispatch and transport used (MCP | CLI | ACP).
