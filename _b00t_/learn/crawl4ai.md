# crawl4ai - AI-Powered Web Crawler

Master the art of web scraping for LLM consumption using craw4ai CLI.

## Overview

crawl4ai is an AI-powered web crawler that outputs clean, LLM-ready markdown. Perfect for research, documentation scraping, and knowledge base building.

**Key Features:**
- Clean markdown output (filters nav, footers, boilerplate)
- Browser automation (Playwright) for dynamic content
- Fast mode for static sites (no JS rendering)
- Deep crawling (follow links up to N depth)
- CSS selector extraction for specific content

## Installation

```bash
# Via uv tool (b00t standard - recommended)
uv tool install crawl4ai[all]

# Install playwright browsers
uv tool run playwright install chromium
```

## Basic Usage

### Simple Crawl

```bash
# Crawl a URL
crawl4ai https://docs.python.org/3/

# Output to file
crawl4ai https://example.com --output ~/.b00t/crawls/example.md

# Custom user agent (identify as b00t)
crawl4ai https://example.com --user-agent "b00t-crawler/0.7"
```

### Output Formats

```bash
# Markdown (default) - LLM-friendly
crawl4ai https://example.com --format markdown

# HTML - raw HTML
crawl4ai https://example.com --format html

# Text - plain text only
crawl4ai https://example.com --format text

# JSON - structured with metadata
crawl4ai https://example.com --format json
```

## Advanced Features

### Deep Crawling (Follow Links)

```bash
# Crawl documentation site with depth 2
crawl4ai https://docs.langchain.com --depth 2 --max-pages 20

# Follow only specific link patterns
crawl4ai https://docs.site.com --depth 3 --link-pattern "*/docs/*"
```

### Dynamic Content (JavaScript)

```bash
# Wait for content to load
crawl4ai https://spa-site.com --wait-for "css:.loaded"

# Execute JavaScript before crawling
crawl4ai https://example.com --js-code "window.scrollTo(0, document.body.scrollHeight)"

# Wait for network to be idle
crawl4ai https://example.com --wait-for "networkidle"
```

### Content Extraction

```bash
# Extract specific CSS selector
crawl4ai https://example.com --css-selector "article.main"

# Multiple selectors
crawl4ai https://example.com --css-selector ".article-content, .sidebar"

# Extract with XPath
crawl4ai https://example.com --xpath "//article[@class='main']"
```

### Fast Mode (Static Sites)

```bash
# No browser rendering (faster)
crawl4ai https://static-site.com --mode fast

# Use fast mode for documentation sites
crawl4ai https://python.langchain.com/docs/ --mode fast --depth 2
```

## Integration with b00t

### Crawl and Learn Pattern

```bash
# 1. Crawl documentation
crawl4ai https://docs.langchain.com/oss/python/releases/langchain-v1 \
  --format markdown \
  --output ~/.b00t/crawls/langchain-v1.md \
  --user-agent "b00t-crawler"

# 2. Ingest into grok knowledge base
b00t-cli grok learn ~/.b00t/crawls/langchain-v1.md
```

### Research Workflow

```bash
# Create organized crawl directory
mkdir -p ~/.b00t/crawls/$(date +%Y-%m-%d)

# Crawl multiple related pages
crawl4ai https://docs.langchain.com --depth 2 \
  --output ~/.b00t/crawls/$(date +%Y-%m-%d)/langchain.md

# Process all crawled content
for file in ~/.b00t/crawls/$(date +%Y-%m-%d)/*.md; do
  b00t-cli grok learn "$file"
done
```

### Via justfile

```bash
# Add to justfile
crawl-and-learn url:
    #!/bin/bash
    FILENAME=$(echo "{{url}}" | sed 's|https://||' | tr '/' '_').md
    crawl4ai "{{url}}" --format markdown --output ~/.b00t/crawls/"$FILENAME"
    b00t-cli grok learn ~/.b00t/crawls/"$FILENAME"

# Usage
just crawl-and-learn https://docs.langchain.com/oss/python/releases/langchain-v1
```

## Best Practices

### 1. Use Fast Mode for Documentation

Documentation sites are usually static:
```bash
crawl4ai https://python.langchain.com/docs/ --mode fast
```

### 2. Set User Agent to Identify as b00t

```bash
export CRAWL4AI_USER_AGENT="b00t-crawler/0.7 (https://github.com/elasticdotventures/_b00t_)"
crawl4ai https://example.com
```

### 3. Organize Crawls by Date/Project

```bash
~/.b00t/crawls/
├── 2025-01-17/
│   ├── langchain-v1.md
│   ├── mcp-protocol.md
│   └── rust-async.md
└── projects/
    ├── pm2-integration/
    └── langchain-integration/
```

### 4. Use CSS Selectors for Specific Content

```bash
# Extract only main article content
crawl4ai https://blog.example.com/post \
  --css-selector "article.post-content" \
  --output post.md
```

### 5. Respect Rate Limits

```bash
# Add delay between requests
crawl4ai https://example.com --depth 3 --delay 2  # 2 seconds between pages
```

## Common Use Cases

### Research New Technology

```bash
# LangChain v1.0 research
crawl4ai https://docs.langchain.com/oss/python/releases/langchain-v1 \
  --mode fast \
  --output ~/.b00t/crawls/langchain-v1-overview.md

# Deep dive on specific features
crawl4ai https://langchain-ai.github.io/langgraph/ \
  --depth 2 \
  --max-pages 15 \
  --output ~/.b00t/crawls/langgraph-docs.md
```

### GitHub Repository Analysis

```bash
# Crawl README and docs
crawl4ai https://github.com/cryxnet/deepmcpagent \
  --css-selector "#readme" \
  --output ~/.b00t/crawls/deepmcpagent-readme.md
```

### API Documentation Scraping

```bash
# Crawl API reference
crawl4ai https://api.example.com/docs \
  --depth 1 \
  --link-pattern "*/reference/*" \
  --output ~/.b00t/crawls/api-reference.md
```

## Troubleshooting

### "Playwright not installed"

```bash
uv tool run playwright install chromium
```

### "Timeout waiting for page"

Increase timeout:
```bash
crawl4ai https://slow-site.com --timeout 60000  # 60 seconds
```

### "Content not rendering"

Try waiting for specific element:
```bash
crawl4ai https://spa-site.com --wait-for "css:.content-loaded"
```

### "Too many pages"

Limit with `--max-pages`:
```bash
crawl4ai https://docs.site.com --depth 3 --max-pages 50
```

## Environment Variables

```bash
# User agent
export CRAWL4AI_USER_AGENT="b00t-crawler/0.7"

# Output directory
export CRAWL4AI_OUTPUT_DIR="~/.b00t/crawls"

# Browser path (optional)
export CRAWL4AI_BROWSER_PATH="/path/to/chromium"

# Verbose logging
export CRAWL4AI_VERBOSE="true"
```

## Comparison: crawl4ai vs wget/curl

| Feature | crawl4ai | wget/curl |
|---------|----------|-----------|
| **JavaScript** | ✅ Full support | ❌ No support |
| **Clean output** | ✅ Markdown | ❌ Raw HTML |
| **Deep crawl** | ✅ Follow links | Limited |
| **Content extraction** | ✅ CSS/XPath | ❌ Manual |
| **LLM-ready** | ✅ Yes | ❌ No |
| **Speed** | Slower (browser) | Faster |

**When to use:**
- **crawl4ai**: Documentation, SPAs, research
- **wget/curl**: Simple static files, APIs

## Integration with LangChain

Once LangChain agent is operational, crawl4ai becomes an MCP tool:

```python
# LangChain agent uses crawl4ai automatically
agent = create_agent(
    model="claude-sonnet-4",
    tools=["crawl4ai-mcp", "grok"],
    system_prompt="You are a researcher"
)

# Agent can invoke: /crawl https://docs.langchain.com
result = await agent.run("Research LangChain v1.0 features")
```

## References

- GitHub: https://github.com/unclecode/crawl4ai (55k+ stars)
- Docs: https://docs.crawl4ai.com/
- MCP Server: `crawl4ai-mcp.mcp.toml`
- b00t Integration: `b00t-grok-py/CRAWL4AI_INTEGRATION.md`
