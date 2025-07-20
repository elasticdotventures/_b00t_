claude mcp add-json --scope user sequential-thinking "$(jq -c . ./sequential-thinking.mcp.json)"

https://docs.anthropic.com/en/docs/claude-code/mcp


 code --add-mcp $(jq -c --arg name "$(basename sequential-thinki
ng.mcp.json .mcp.json)" '. + {name: $name}' sequential-thinking.m
cp.json)
Added MCP servers: sequential-thinking

