# RAGLight Integration for b00t MCP Server

## Overview

The b00t MCP server now includes a comprehensive RAGLight integration that provides:

- **Universal Document Loading**: URL, Git repos, PDFs, text files with auto-detection
- **Topic-Based Organization**: Maps to existing b00t datums (rust, python, docker, etc.)
- **Async Processing**: Non-blocking document indexing with progress tracking
- **Generic MCP Proxy**: Dynamic tool execution without compile-time dependencies

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Claude Code   │ ──→│   b00t-mcp      │ ──→│   RAGLight     │
│   (via MCP)     │    │   Generic Proxy  │    │   Python       │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                ▲
                                │
                       ┌────────▼─────────┐
                       │   Document       │
                       │   Loaders        │
                       │ ┌──────────────┐ │
                       │ │ URL Spider   │ │
                       │ │ Git Cloner   │ │
                       │ │ PDF Parser   │ │
                       │ │ Text Reader  │ │
                       │ └──────────────┘ │
                       └──────────────────┘
```

## Features

### 🧠 **Intelligent Document Loading**
- **Auto-Detection**: Automatically determines loader type from source
- **Multiple Sources**: URLs, Git repositories, PDFs, markdown, text files
- **Topic Mapping**: Documents are indexed into b00t datum topics

### 🔄 **Generic MCP Proxy**
- **Runtime Tool Registration**: Add new MCP tools without recompilation
- **Dynamic Discovery**: Auto-discover tools from MCP servers
- **Health Monitoring**: Track server health and connection status

### ⚡ **Async Processing**
- **Non-Blocking**: Document indexing happens in background
- **Progress Tracking**: Monitor indexing job status and progress
- **Concurrent Jobs**: Multiple documents can be processed simultaneously

## Usage Examples

### 1. Adding Documents to RAG

```bash
# Via b00t CLI (when proxy is integrated)
b00t rag add-document \
  --source "https://doc.rust-lang.org/book/" \
  --topic "rust" \
  --loader-type "url"

# Git repository
b00t rag add-document \
  --source "https://github.com/tokio-rs/tokio" \
  --topic "rust" \
  --loader-type "git"

# PDF document
b00t rag add-document \
  --source "/path/to/manual.pdf" \
  --topic "docker" \
  --loader-type "pdf"
```

### 2. Querying RAG System

```bash
# Query specific topic
b00t rag query \
  --topic "rust" \
  --query "How to handle async errors in Tokio?" \
  --max-results 5

# Query Docker knowledge
b00t rag query \
  --topic "docker" \
  --query "Best practices for multi-stage builds"
```

### 3. MCP Tool Management

```bash
# List available tools
b00t proxy list-tools

# Health check all servers
b00t proxy health-check

# Register new tool
b00t proxy register-tool --tool-definition '{
  "name": "custom-tool",
  "description": "Custom MCP tool",
  "input_schema": {...},
  "server_config": {...}
}'

# Discover tools from server
b00t proxy discover-tools \
  --command "python3" \
  --args "-m" "custom_mcp_server"
```

## Available Topics

The system automatically discovers b00t datums as topics:

- **Core Languages**: `rust`, `python`, `typescript`, `bash`
- **Tools**: `git`, `docker`, `kubernetes`, `just`
- **Protocols**: `mcp`, `acp`
- **Custom**: Any TOML file or directory in `~/.dotfiles/_b00t_/`

## Integration Points

### 1. Direct Python Integration
```python
from raglight import RagLight, RagLightConfig

config = RagLightConfig(
    provider="openai",
    model="gpt-4o-mini",
    k=10
)

rag = RagLight(config)
await rag.load_url("https://example.com", topic="rust")
result = await rag.query("How to use async?", topic="rust")
```

### 2. MCP Server Integration
The b00t MCP server exposes RAGLight functionality as MCP tools:

- `rag-add-document`: Index documents into topics
- `rag-query`: Query knowledge by topic
- `rag-job-status`: Check indexing progress
- `rag-list-topics`: Show available topics
- `proxy-execute`: Execute any registered tool
- `proxy-health-check`: Monitor server health

### 3. Generic Proxy Pattern
```rust
use b00t_mcp::{GenericMcpProxy, McpToolRequest};

let mut proxy = GenericMcpProxy::new();

// Register RAGLight tools
let raglight_tools = create_raglight_tools();
proxy.register_tools_from_config(raglight_tools)?;

// Execute tool dynamically
let request = McpToolRequest {
    tool: "rag-query".to_string(),
    params: serde_json::json!({
        "topic": "rust",
        "query": "Error handling patterns"
    }),
    request_id: None,
};

let response = proxy.execute_tool(request).await?;
```

## Installation & Setup

### Prerequisites
1. **Python 3.8+** with RAGLight installed:
   ```bash
   pip install raglight
   ```

2. **b00t MCP Server** with RAGLight integration:
   ```bash
   cargo build --bin b00t-mcp
   ```

### Configuration
The system uses default configuration that can be customized:

```rust
use b00t_mcp::RagLightConfig;

let config = RagLightConfig {
    venv_path: Some(PathBuf::from("/path/to/venv")),
    raglight_path: PathBuf::from("/path/to/raglight"),
    vector_db_path: PathBuf::from("~/.b00t/raglight/vector_db"),
    max_concurrent_jobs: 3,
    embedding_model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
    llm_config: LlmConfig {
        provider: "openai".to_string(),
        model: "gpt-4o-mini".to_string(),
        api_key: None, // Uses environment variables
        config: HashMap::new(),
    },
};
```

## Error Handling

The system includes comprehensive error handling:

- **Network failures**: Retry with exponential backoff
- **Invalid documents**: Skip with detailed error messages
- **Runtime conflicts**: Async runtime isolation
- **Resource limits**: Job queuing and throttling

## Performance Considerations

- **Concurrent Processing**: Multiple documents indexed simultaneously
- **Streaming**: Large documents processed in chunks
- **Caching**: Vector embeddings cached for faster queries
- **Memory Management**: Efficient cleanup of temporary resources

## Future Enhancements

1. **Real-time Updates**: Watch git repos for changes
2. **Smart Chunking**: Context-aware document splitting
3. **Multi-modal**: Support for images and code analysis
4. **Federated Search**: Cross-topic knowledge discovery
5. **Export Tools**: Generate summaries and reports

This integration transforms the b00t ecosystem into a comprehensive knowledge management system that can intelligently process and query any document source while maintaining the flexibility and performance of the existing architecture.