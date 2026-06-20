---
name: firecrawl
description: |
  Web scraping and crawling for AI agents via Firecrawl MCP. Scrape URLs to markdown,
  crawl websites, search the web, and extract structured data. Supports cloud API and
  self-hosted deployments including CRW (Rust alternative).
version: 1.0.0
tags: [mcp, web, scraping, firecrawl, crawl, markdown, data-extraction]
applies_to: [web scraping, URL to markdown, website crawling, structured data extraction, AI research]
output_types: [.md, .json]
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, mcp_firecrawl_*
---

## What This Skill Does

Enables AI agents to interact with web content through the Firecrawl MCP server. Core capabilities:

- **Scrape**: Convert any URL to clean markdown, HTML, or structured JSON
- **Crawl**: Multi-page extraction with BFS traversal
- **Search**: Web search with optional content scraping
- **Map**: Discover all URLs on a website
- **Extract**: LLM-powered structured data extraction
- **Agent**: Autonomous multi-source research

## When It Activates

Activate this skill when you see phrases like:

- "scrape this website" or "get content from this URL"
- "crawl the documentation" or "extract all pages from..."
- "search the web for..." with content extraction
- "find all URLs on this site"
- "extract structured data from this page"
- "what does this website say about..."
- "gather information from multiple pages"

## Installation

### Option A: Cloud (Fastest)

```bash
# Get API key from firecrawl.dev/app/api-keys
# Add to MCP config:
b00t mcp add firecrawl -- npx -y firecrawl-mcp
# Set env: FIRECRAWL_API_KEY=fc-YOUR_KEY
```

### Option B: Self-Hosted (CRW - Recommended)

```bash
# Single binary, 6MB RAM, no server needed
curl -fsSL https://raw.githubusercontent.com/us/crw/main/install.sh | CRW_BINARY=crw sh

# Add to MCP config:
b00t mcp add crw -- npx crw-mcp
# No API key required in embedded mode
```

## Available Tools

### Core Scraping

| Tool | Best For | Returns |
|------|----------|---------|
| `firecrawl_scrape` | Single URL extraction | markdown/JSON/HTML |
| `firecrawl_batch_scrape` | Multiple known URLs | markdown[] |
| `firecrawl_map` | Discover URLs on site | URL[] |

### Web Discovery

| Tool | Best For | Returns |
|------|----------|---------|
| `firecrawl_search` | Find info across web | results[] |
| `firecrawl_crawl` | Multi-page extraction | markdown[] |

### Advanced

| Tool | Best For | Returns |
|------|----------|---------|
| `firecrawl_extract` | Structured data extraction | JSON (schema-defined) |
| `firecrawl_agent` | Autonomous research | JSON (async) |
| `firecrawl_interact` | Click/navigate pages | execution result |

## Usage Patterns

### Quick Scrape (Markdown)

```json
{
  "name": "firecrawl_scrape",
  "arguments": {
    "url": "https://docs.example.com/api",
    "formats": ["markdown"],
    "onlyMainContent": true
  }
}
```

### Structured Extraction (JSON)

```json
{
  "name": "firecrawl_scrape",
  "arguments": {
    "url": "https://example.com/product",
    "formats": [{
      "type": "json",
      "prompt": "Extract product details",
      "schema": {
        "type": "object",
        "properties": {
          "name": {"type": "string"},
          "price": {"type": "number"},
          "inStock": {"type": "boolean"}
        }
      }
    }]
  }
}
```

### Web Search with Content

```json
{
  "name": "firecrawl_search",
  "arguments": {
    "query": "best practices for async Rust 2025",
    "limit": 5,
    "scrapeOptions": {
      "formats": ["markdown"],
      "onlyMainContent": true
    }
  }
}
```

### Site Crawling

```json
{
  "name": "firecrawl_crawl",
  "arguments": {
    "url": "https://docs.example.com/*",
    "maxDepth": 2,
    "limit": 50
  }
}
```

## Decision Guide

```
Know exact URL? → scrape (single) or batch_scrape (multiple)
Need to find URLs? → search (web) or map (site discovery)
Need all pages? → crawl (with limits!)
Want specific data? → scrape with JSON format + schema
Complex research? → agent (async, poll for results)
```

## Self-Hosted Options

### CRW (Recommended)

| Metric | CRW | Firecrawl Self-Host |
|--------|-----|---------------------|
| RAM | 6 MB | 4 GB+ |
| Containers | 0 | 5+ |
| Cold Start | 85ms | 30-60s |
| Setup | Single binary | Docker Compose |

```bash
# CRW embedded mode - no server, no config
npx crw-mcp

# CRW cloud mode - adds web search
CRW_API_URL=https://fastcrw.com/api CRW_API_KEY=xxx npx crw-mcp
```

### Firecrawl Self-Hosted

```bash
git clone https://github.com/firecrawl/firecrawl
cd firecrawl
docker compose up -d

# MCP config:
FIRECRAWL_API_URL=http://localhost:3002 npx -y firecrawl-mcp
```

## Important Notes

### Format Selection

- **JSON format** (preferred): Use with schema to extract only needed data
- **Markdown format**: Only when full content is required (articles, summaries)

### Token Management

- Use `onlyMainContent: true` to skip navigation/footer
- Set `limit` on crawls to avoid context overflow
- Use JSON extraction instead of full markdown when possible

### Rate Limits

- Built-in exponential backoff (configurable)
- Batch operations are async - poll for status

## Troubleshooting

**"API key required"** - Set FIRECRAWL_API_KEY env var or use CRW in embedded mode

**"Rate limited"** - Increase FIRECRAWL_RETRY_MAX_ATTEMPTS, wait and retry

**"Context too large"** - Use JSON format with schema, reduce crawl depth

**"Self-hosted connection refused"** - Verify Docker containers are running, check FIRECRAWL_API_URL

## Related Skills

- **context7**: Live library documentation
- **browser-use**: Interactive browser automation
- **playwright**: UI testing and scraping

## References

- Firecrawl MCP: https://github.com/firecrawl/firecrawl-mcp-server
- Firecrawl API: https://github.com/firecrawl/firecrawl
- CRW Alternative: https://github.com/us/crw
- API Docs: https://docs.firecrawl.dev
