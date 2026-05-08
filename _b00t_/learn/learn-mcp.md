# Microsoft Learn MCP Server

Official MCP server giving AI agents direct access to real-time, trusted Microsoft documentation and code samples. Free, no auth required, no API keys.

**Repo**: https://github.com/MicrosoftDocs/mcp (NOT microsoft/learn-mcp)
**Endpoint**: https://learn.microsoft.com/api/mcp (Streamable HTTP)
**License**: CC-BY-4.0
**Language**: TypeScript
**Stars**: 1614+

## Overview

The Microsoft Learn MCP Server eliminates AI hallucinations about Microsoft technologies by providing direct, secure access to official Microsoft docs. Unlike generic web search, it only accesses first-party Microsoft documentation — no insecure blogs or malicious sites.

### Key Benefits
- **Eliminate Hallucinations** - Stop AI from inventing non-existent Azure SDK methods or hallucinating library packages
- **Plug & Play (No Auth)** - No API keys, logins, or sign-ups. One-click install
- **100% Trusted & Safe** - Only accesses official 1st-party Microsoft documentation
- **Completely Free** - High search capacity for heavy coding sessions

## Tools

| Tool | Description | Parameters |
|------|-------------|-----------|
| `microsoft_docs_search` | Semantic search against Microsoft official technical documentation | `query` (string) |
| `microsoft_docs_fetch` | Fetch and convert a Microsoft documentation page to markdown | `url` (string) |
| `microsoft_code_sample_search` | Search official Microsoft/Azure code snippets and examples | `query` (string), `language` (string, optional) |

## Endpoint

```
https://learn.microsoft.com/api/mcp
```

Standard config for MCP clients:
```json
{
  "mcpServers": {
    "microsoft-learn": {
      "type": "http",
      "url": "https://learn.microsoft.com/api/mcp"
    }
  }
}
```

### Experimental: OpenAI-Compatible Endpoint
```
https://learn.microsoft.com/api/mcp/openai-compatible
```
Supports OpenAI Deep Research models.

### Token Budget Control
Append `?maxTokenBudget=N` to limit token count in search responses:
```
https://learn.microsoft.com/api/mcp?maxTokenBudget=2000
```

## Installation Methods

### VS Code (One-Click)
Search "@mcp learn" in Extensions view, or use the one-click install badge from the repo README.

### GitHub Copilot CLI
```
/plugin install microsoftdocs/mcp
```

### Claude Desktop
Add custom connector with URL `https://learn.microsoft.com/api/mcp` using type `http`.

### Claude Code
```
/plugin install microsoft-docs@claude-plugins-official
```
(Includes MCP server + agent skills)

### Codex
```
codex mcp add "microsoft-learn" --url "https://learn.microsoft.com/api/mcp"
```

### Cursor
Use the one-click install badge, or configure with `type: "http"` and `url: "https://learn.microsoft.com/api/mcp"`.

### Visual Studio
Built-in starting from VS 2022 / 2026. No manual config needed.

### Gemini CLI
Add to `.gemini/settings.json`:
```json
{
  "Microsoft Learn MCP Server": {
    "httpUrl": "https://learn.microsoft.com/api/mcp"
  }
}
```

### Generic MCP Clients
```json
{
  "servers": {
    "microsoft-learn": {
      "type": "http",
      "url": "https://learn.microsoft.com/api/mcp"
    }
  }
}
```

## CLI Tool (@microsoft/learn-cli)

The same search capabilities are available as a CLI tool (no MCP client needed):

```bash
# Run instantly (no install)
npx @microsoft/learn-cli search "azure functions timeout"

# Install globally
npm install -g @microsoft/learn-cli
mslearn search "azure functions timeout"

# JSON output for scripting
mslearn search "azure openai" --json | jq '.results[].title'
```

## Agent Skills

The Microsoft Learn MCP server comes with portable agent skills:

| Skill | Purpose | Best For |
|-------|---------|----------|
| `microsoft-docs` | Understanding concepts, tutorials, architecture, limits | "How does X work?", learning, configuration guides |
| `microsoft-code-reference` | API lookups, code samples, verification, error fixing | Implementing code, finding correct methods, troubleshooting |
| `microsoft-skill-creator` | Meta-skill that generates custom agent skills for any Microsoft technology | Creating a skill about a new Azure library or .NET feature |

## Example Prompts

These prompts demonstrate the value of Microsoft Learn MCP:

> "Give me the Azure CLI commands to create an Azure Container App with a managed identity."

> "Is gpt-5.4 available in Azure EU regions?"

> "Are you sure this is the right way to implement IHttpClientFactory in a .NET 8 minimal API?"

> "Show me runnable Python code to do harms eval using the Azure AI Foundry evaluation SDK."

## System Prompt for Better Tool Usage

Even tool-friendly models may need prompting. Add a rule like:

```md
## Querying Microsoft Documentation

You have access to MCP tools called `microsoft_docs_search`,
`microsoft_docs_fetch`, and `microsoft_code_sample_search` - these tools
allow you to search through and fetch Microsoft's latest official documentation
and code samples, and that information might be more detailed or newer than
what's in your training data set.

When handling questions around how to work with native Microsoft technologies,
such as C#, F#, ASP.NET Core, Microsoft.Extensions, NuGet, Entity Framework,
the `dotnet` runtime - please use these tools for research purposes.
```

## b00t Configuration

The b00t datum at `_b00t_/mcp/learn-mcp.mcp.toml` configures:

- **Primary**: HTTP Streamable endpoint at `https://learn.microsoft.com/api/mcp` (no auth, no install)
- **Fallback**: CLI tool via `npx @microsoft/learn-cli` (not MCP protocol, terminal-only)

No credential management needed — this MCP server requires no authentication.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Connection errors | Verify network connection and endpoint URL |
| No results returned | Rephrase query with more specific technical terms |
| Tool not appearing | Restart IDE, verify MCP extension installed |
| HTTP 405 | Endpoint accessed via browser — use MCP client or MCP Inspector |

## Building a Custom Client

If building a programmatic integration:
1. **Discover Tools Dynamically** — Use `tools/list` at runtime, do NOT hardcode tool names
2. **Refresh on Failure** — If a tool call fails (404/400), refresh tool cache via `tools/list`
3. **Handle Live Updates** — Listen for `listChanged` server notifications

## References

- [MicrosoftDocs/mcp on GitHub](https://github.com/MicrosoftDocs/mcp)
- [Microsoft Learn MCP Server product docs](https://learn.microsoft.com/training/support/mcp)
- [Microsoft MCP Servers](https://github.com/microsoft/mcp)
- [@microsoft/learn-cli on npm](https://www.npmjs.com/package/@microsoft/learn-cli)
- [MCP Specification](https://modelcontextprotocol.io)

## LFMF Integration

Record lessons about the Microsoft Learn MCP server:

```bash
# Repo location
b00t lfmf learn-mcp "repo: MicrosoftDocs/mcp (not microsoft/learn-mcp as some docs suggest)"

# No auth needed
b00t lfmf learn-mcp "auth: No API keys, logins, or sign-ups required for Microsoft Learn MCP"

# HTTP transport
b00t lfmf learn-mcp "transport: Streamable HTTP at https://learn.microsoft.com/api/mcp — no local server process needed"

# Tool names
b00t lfmf learn-mcp "tools: microsoft_docs_search (docs), microsoft_docs_fetch (page→md), microsoft_code_sample_search (code snippets)"

# CLI alternative
b00t lfmf learn-mcp "cli: Use npx @microsoft/learn-cli or install globally as mslearn for terminal searches without MCP client"

# Agent plugins
b00t lfmf learn-mcp "plugins: Claude Code /plugin install microsoft-docs@claude-plugins-official, Copilot /plugin install microsoftdocs/mcp"

# VS Code
b00t lfmf learn-mcp "vscode: Search @mcp learn in Extensions view or use one-click install badge"

# VS built-in
b00t lfmf learn-mcp "visual-studio: Built-in from VS 2022/2026, no manual configuration needed"

# Endpoint URL
b00t lfmf learn-mcp "endpoint: https://learn.microsoft.com/api/mcp — remember this URL"
```

Get advice:
```bash
b00t advice learn-mcp "installation"
b00t advice learn-mcp "tools"
b00t advice learn-mcp "configuration"
b00t advice learn-mcp "endpoint"
b00t advice learn-mcp list
```
