# b00t MCP Registry

## 🎯 **Dual-Role Architecture**

b00t-mcp acts as BOTH:
1. **MCP Server** - Exposes b00t capabilities via Model Context Protocol
2. **MCP Registry** - Discovers, registers, and manages other MCP servers

This creates a self-hosting ecosystem where b00t can dynamically discover and expose tools from any MCP server.

## 🏗️ **Architecture**

```
┌─────────────────────────────────────────────────────────────┐
│                      b00t-mcp Server                         │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐    ┌──────────────┐    ┌───────────────┐ │
│  │   MCP        │    │   MCP        │    │   Generic     │ │
│  │   Server     │◄───┤   Registry   │◄───┤   Proxy       │ │
│  │   (Stdio)    │    │              │    │               │ │
│  └──────────────┘    └──────────────┘    └───────────────┘ │
│         │                    │                     │         │
│         │                    │                     │         │
│         ▼                    ▼                     ▼         │
│  ┌──────────────────────────────────────────────────────┐  │
│  │          Local MCP Registry Storage                   │  │
│  │          (~/.b00t/mcp_registry.json)                 │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴──────────┐
                    │                    │
                    ▼                    ▼
           ┌────────────────┐   ┌────────────────┐
           │  Official MCP  │   │   Discovered   │
           │   Registry     │   │   Servers      │
           │  (Upstream)    │   │   (Local)      │
           └────────────────┘   └────────────────┘
```

## 🚀 **Core Capabilities**

### 1. **Server Registration**
Register any MCP server with b00t registry:
```bash
# Register via MCP tool
registry-register \
  --id "io.example/my-server" \
  --name "My MCP Server" \
  --command "my-mcp-server" \
  --args "--stdio"
```

### 2. **Dynamic Tool Discovery**
Automatically discover tools from registered servers:
```bash
# Auto-discover tools from server
registry-discover

# Tools are immediately available via generic proxy
proxy-execute --tool "discovered-tool-name" --params '{...}'
```

### 3. **Registry as MCP Resource**
The registry itself is exposed as an MCP resource:
- List all registered servers
- Search by tag or keyword
- Get server health status
- Export/import registry data

### 4. **Official Registry Sync**
Sync with https://github.com/modelcontextprotocol/registry:
```bash
# Sync servers from official registry
registry-sync-official
```

## 📦 **Registry Data Format**

Compatible with official MCP registry format:
```json
{
  "version": "1.0.0",
  "servers": [
    {
      "id": "io.b00t/raglight",
      "name": "RAGLight Server",
      "description": "Document ingestion and RAG queries",
      "version": "0.1.0",
      "tags": ["rag", "ai", "documents"],
      "config": {
        "command": "python3",
        "args": ["-m", "raglight.mcp_server"],
        "transport": "stdio"
      },
      "metadata": {
        "registered_at": "2025-10-13T08:00:00Z",
        "source": "local",
        "health_status": "healthy"
      }
    }
  ]
}
```

## 🔧 **MCP Tools**

### Registry Management
- `registry-register` - Register new MCP server
- `registry-unregister` - Remove server from registry
- `registry-list` - List all registered servers
- `registry-get` - Get specific server details
- `registry-search` - Search servers by keyword/tag

### Discovery & Sync
- `registry-discover` - Auto-discover servers from system
- `registry-sync-official` - Sync with official MCP registry
- `registry-export` - Export registry to JSON
- `registry-import` - Import registry from JSON

### Dynamic Execution
- `proxy-execute` - Execute any tool from registered servers
- `proxy-list-tools` - List all available tools
- `proxy-health-check` - Check health of all servers

## 🎯 **Use Cases**

### 1. **Universal MCP Hub**
b00t acts as a central hub that:
- Discovers all MCP servers on system
- Registers them in local registry
- Exposes all their tools through single interface

### 2. **Tool Marketplace**
- Browse official MCP registry
- Install servers locally
- Auto-discover their capabilities
- Execute tools dynamically

### 3. **Self-Hosting**
b00t-mcp registers itself:
```rust
// On startup
registry.register(create_registration_from_datum(
    "io.b00t/b00t-mcp",
    "b00t MCP Server",
    current_exe(),
    vec!["--stdio"],
))?;
```

### 4. **Cross-Server Tool Composition**
Combine tools from multiple servers:
```bash
# Tool from server A
proxy-execute --tool "server-a/analyze" --params '{"file": "data.csv"}'

# Tool from server B using A's output
proxy-execute --tool "server-b/visualize" --params '{"data": "..."}'
```

## 🔍 **Server Discovery**

b00t automatically discovers MCP servers from:
1. **System Paths**
   - `~/.local/share/mcp/servers`
   - `~/.config/mcp/servers`
   - `/usr/local/share/mcp/servers`

2. **Configuration Files**
   - `.mcp.json` (project-specific)
   - `~/.claude.json` (user-specific)
   - b00t datum configurations

3. **Official Registry**
   - https://github.com/modelcontextprotocol/registry
   - Syncs server metadata and configurations

## 📊 **Health Monitoring**

Track server health status:
```bash
# Check all servers
proxy-health-check

# Response includes per-server status
{
  "healthy_servers": 5,
  "unhealthy_servers": 1,
  "health_status": {
    "io.b00t/b00t-mcp": true,
    "io.example/broken-server": false
  }
}
```

## 🔐 **Security**

### Namespace Ownership
Following official MCP registry rules:
- `io.github.username/*` - Requires GitHub auth
- `io.gitlab.username/*` - Requires GitLab auth
- `me.domain/*` - Requires DNS/HTTP verification
- `io.b00t/*` - b00t-managed servers

### Local vs Remote
- **Local registration**: Full trust, no verification
- **Official sync**: Metadata only, user verifies before execution
- **Discovered**: Flagged for review before trust

## 🎨 **Example Workflow**

```bash
# 1. Sync official registry
registry-sync-official
# → Downloads server metadata from github.com/modelcontextprotocol/registry

# 2. Search for specific functionality
registry-search --keyword "database"
# → Returns: postgres-mcp, mysql-mcp, sqlite-mcp

# 3. Register chosen server locally
registry-register \
  --id "io.modelcontextprotocol/sqlite-mcp" \
  --name "SQLite MCP" \
  --command "npx" \
  --args "-y @modelcontextprotocol/server-sqlite"

# 4. Auto-discover its tools
registry-discover
# → Discovers: sqlite-query, sqlite-execute, sqlite-schema

# 5. Use discovered tools
proxy-execute \
  --tool "sqlite-query" \
  --params '{"query": "SELECT * FROM users"}'
```

## 🌐 **Integration with Official Registry**

b00t registry is compatible with https://github.com/modelcontextprotocol/registry:

| Feature | Official Registry | b00t Registry |
|---------|-------------------|---------------|
| Server metadata | ✅ PostgreSQL | ✅ JSON file |
| Authentication | ✅ GitHub/DNS | ⏳ Future |
| Discovery | ✅ API | ✅ Auto-scan |
| Tool execution | ❌ | ✅ Via proxy |
| Health monitoring | ❌ | ✅ Built-in |
| Local-first | ❌ | ✅ Yes |

## 🔮 **Future Enhancements**

1. **Registry Sync Server** - Contribute discovered servers back to official registry
2. **P2P Registry** - Share registries across b00t instances
3. **Smart Discovery** - ML-based server capability detection
4. **Tool Composition** - Chain tools from multiple servers
5. **Performance Monitoring** - Track tool execution metrics

## 🤓 **Implementation Notes**

The registry uses a **dual-layer architecture**:
- **Storage Layer**: JSON-based persistent registry
- **Runtime Layer**: Generic MCP proxy for dynamic execution

This allows:
- Offline operation (local registry)
- Online sync (official registry)
- Dynamic tool discovery (runtime proxy)
- Self-hosting (b00t registers itself)

Perfect example of b00t's philosophy: **DRY (Don't Repeat Yourself)** - leverage existing MCP ecosystem rather than building everything from scratch.