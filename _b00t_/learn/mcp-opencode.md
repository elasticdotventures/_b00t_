---
hand-rolled-mcp-servers: For opencode compatibility, every tool MUST include a description field, inputSchema MUST have type:object at top level (oneOf alone is rejected), and strip $schema from inputSchema. Official rmcp SDK servers avoid these issues by construction.

---
b00t-mcp-schema-validation: [[b00t.gate.check]] is rejected by b00t parser — use [[b00t.gate]] with command/hint/env fields. [[b00t.env]] is also rejected — gate env vars via [[b00t.gate]]. Old [mcp] format (without [b00t] wrapper) is not recognized.
