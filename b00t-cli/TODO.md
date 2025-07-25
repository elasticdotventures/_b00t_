add:
b00t mcp json add --dwiw which allows various json strings to be pasted.
it follows the rules, firstly it should attempt to strip any line that starts with a [\s]*\/\/.*$  which is consider a comment before attemptvalidate the json
then it will attempt to match the json to various patterns of known mcp servers
then it will call the vscode via the LSP Socket via JSONRPC to tell it to add an mcp package with the json
there will be an equivalent cli interpreter, that accepts mcp input in various formats and installs the tool


 claude mcp add context7 -- npx -y @upstash/context7-mcp

