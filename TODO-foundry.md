# TODO: Azure AI Foundry gaps in `~/.b00t`

## Summary

`foundry-samples/` contains runnable Azure AI Foundry and Agent Service examples. In `~/.b00t`, Azure/Foundry support already exists at the datum/configuration level, but several implementation and workflow gaps remain.

## Existing overlap

- Azure AI Foundry CLI datum exists: `_b00t_/azure-ai-foundry.cli.toml`
- Azure AI Foundry MCP datum exists: `_b00t_/azure-ai-foundry.mcp.toml`
- Foundry Local AI runtime datum exists: `_b00t_/foundry-local.ai.toml`
- General Azure infra/docs already exist in `azure.🤖.云☁️/`, `docs/README-azure.md`, and `B00T-CAPABILITY-MAP.md`

## Gaps

### 1. No concrete runnable Azure Foundry agent examples in b00t

`~/.b00t` has Foundry datums and install instructions, but not equivalent runnable examples like:

- hosted MCP agents
- hosted workflow agents
- text-search/RAG agents
- LangGraph-hosted agents

Reference examples in `foundry-samples/`:

- `foundry-samples/samples/microsoft/python/getting-started-agents/agent-framework/agent_with_hosted_mcp/main.py`
- `foundry-samples/samples/microsoft/python/getting-started-agents/agent-framework/agents_in_workflow/main.py`
- `foundry-samples/samples/microsoft/hosted-agents/python/agent_with_hosted_mcp/main.py`

### 2. No `agent.yaml` / `azd ai agent` deployment flow

`foundry-samples/` includes deployable agent manifests and Azure Developer CLI deployment guidance. No equivalent first-class flow was found in `~/.b00t`.

Reference examples:

- `foundry-samples/samples/microsoft/python/getting-started-agents/agent-framework/agent_with_hosted_mcp/agent.yaml`
- `foundry-samples/samples/microsoft/hosted-agents/python/agent_with_hosted_mcp/agent.yaml`

### 3. No Azure AI AgentServer SDK usage in b00t code

No b00t implementation was found using patterns such as:

- `from azure.ai.agentserver.agentframework import from_agent_framework`
- `HostedMCPTool(...)`
- `from azure.ai.agentserver.langgraph import from_langgraph`

This means b00t currently describes Foundry integration more than it implements it.

### 4. No Azure AI Agent Service IaC templates comparable to `foundry-samples`

`foundry-samples/` includes ARM and Terraform templates for:

- basic agent setup
- standard agent setup
- BYO virtual network / private networking
- customer-managed keys
- Azure AI Search / Cosmos DB / Storage-backed agent state

No equivalent dedicated Foundry Agent Service IaC set was identified in `~/.b00t`.

Reference directories:

- `foundry-samples/samples/microsoft/infrastructure-setup/`
- `foundry-samples/samples/microsoft/infrastructure-setup-terraform/`

### 5. Missing sample integrations for common Foundry tools/services

`foundry-samples/` includes examples for:

- Azure AI Search
- Bing grounding
- Azure Functions
- Logic Apps
- OpenAPI tools
- enterprise/file search
- third-party tool integrations

Equivalent b00t sample implementations were not identified.

### 6. Docs/config exceed operational recipes

Current b00t Foundry support is mainly:

- install/update datums
- MCP/CLI metadata
- learning content
- high-level orchestration positioning

Missing are:

- `just` recipes to bootstrap and run Foundry examples
- validated end-to-end tutorial paths
- tested sample projects inside the repo
- operational runbooks for local dev -> Azure deploy

## Suggested next actions

1. Port one minimal Python Hosted MCP sample into a b00t-owned example.
2. Add one `agent.yaml` + `azd ai agent` deployment path.
3. Add one Foundry IaC template path for basic agent deployment.
4. Add `just` recipes for local run, deploy, and smoke test.
5. Add one documented Azure AI Search or OpenAPI integration example.

## High-value candidate ports

- Hosted MCP docs agent
- agents-in-workflow sample
- basic infrastructure setup
- Terraform private-network setup
