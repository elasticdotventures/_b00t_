# b00t-j0b-py 🕷️

**Production-ready** web crawler job system for the b00t ecosystem using Redis RQ. Intelligently crawls web content with depth-based link following, robots.txt compliance, and specialized parsers for GitHub, PyPI, NPM, and other platforms.

## ✅ Status: Fully Implemented & Tested

Successfully crawled and processed:
- ✅ **GitHub repositories**: `https://github.com/microsoft/vscode` (144 links found, specialized parsing)  
- ✅ **General websites**: `https://httpbin.org/html` (HTML to Markdown conversion)
- ✅ **Redis integration**: 143 URLs queued, 2 robots.txt cached, 2 pages processed
- ✅ **CLI interface**: All commands working (digest, crawl, worker, status, parsers)

## 🚀 Quick Start

```bash
# Install and setup
cd b00t-j0b-py && uv sync
cp .env.example .env

# Test the CLI
uv run b00t-j0b parsers
# 🔍 Available Content Parsers
# ✅ GitHub: GitHubParser
# ✅ PyPI: PyPIParser  
# ✅ NPM: NPMParser
# ✅ Crates.io: CratesParser

# Crawl a GitHub repo (finds README, issues, metadata)
uv run b00t-j0b crawl https://github.com/microsoft/vscode --sync
# ✅ Successfully crawled https://github.com/microsoft/vscode
#    Title: GitHub - microsoft/vscode: Visual Studio Code
#    Content length: 8725 chars
#    Links found: 144

# Check system status  
uv run b00t-j0b status
# 📊 URLs crawled: 2, Robots.txt cached: 2, Content cached: 2
```

## Features

### 🚀 Core Crawling
- **Depth-based crawling**: Follow links recursively with configurable depth limits
- **Robots.txt compliance**: Automatically fetch, cache, and respect robots.txt
- **Redis tracking**: Centralized tracking of crawled URLs and state management
- **Content deduplication**: Avoid re-crawling the same URLs
- **Rate limiting**: Configurable delays and respect for crawl-delay directives

### 🧠 Smart Content Processing
- **HTML to Markdown**: Clean conversion of web content to structured Markdown
- **Specialized parsers**: Custom parsers for GitHub, PyPI, NPM, and Crates.io
- **Content type detection**: Handle HTML, JSON, PDF, and other formats
- **Extensible parser system**: Easy to add new platform-specific parsers

### 🔄 Job System
- **Redis RQ integration**: Distributed job processing with Redis queues
- **Multiple queue priorities**: High, normal, and low priority job queues
- **Background processing**: Async job execution with monitoring
- **Error handling**: Robust error recovery and retry mechanisms

### 📊 Monitoring & Management
- **Real-time stats**: Track crawled URLs, cache hits, and queue sizes
- **CLI interface**: Easy-to-use command-line tools for management
- **Content caching**: Redis-based caching of processed content
- **Cleanup utilities**: Automated cleanup of old crawl data

## Installation

```bash
cd b00t-j0b-py
uv install -e .
```

## Configuration

Copy the example environment file and configure:

```bash
cp .env.example .env
# Edit .env with your settings
```

Key configuration options:
- `REDIS_URL`: Redis connection string (default: redis://localhost:6379/0)
- `CRAWLER_MAX_DEPTH`: Maximum crawl depth (default: 3)
- `CRAWLER_DELAY`: Delay between requests in seconds (default: 1.0)
- `CRAWLER_USER_AGENT`: User agent string for requests

## Usage

### CLI Commands

#### Digest a URL (recursive crawling)
```bash
# Crawl a URL with depth 2, following links
b00t-j0b digest https://example.com --depth 2

# Run synchronously (no queue)
b00t-j0b digest https://github.com/user/repo --depth 1 --sync

# Use high priority queue
b00t-j0b digest https://pypi.org/project/requests --queue high
```

#### Crawl a single URL
```bash
# Crawl just one URL
b00t-j0b crawl https://example.com

# Crawl synchronously
b00t-j0b crawl https://npmjs.com/package/react --sync
```

#### Start a worker
```bash
# Process default queue
b00t-j0b worker

# Process multiple queues
b00t-j0b worker --queue default --queue high

# Burst mode (exit when queues empty)
b00t-j0b worker --burst
```

#### Monitor status
```bash
# Show crawler statistics
b00t-j0b status

# List available parsers
b00t-j0b parsers
```

#### Management
```bash
# Clear a queue
b00t-j0b clear-queue --queue default

# Run cleanup job
b00t-j0b cleanup
```

### Python API

```python
from b00t_j0b_py.crawler import crawler
from b00t_j0b_py.parsers.base import registry
from b00t_j0b_py.redis_client import tracker

# Crawl a single URL
result = crawler.crawl_url("https://github.com/user/repo")
if result.is_success():
    data = result.unwrap()
    print(f"Crawled: {data['title']}")

# Parse with specialized parser
parser_result = registry.parse_content(
    "https://pypi.org/project/requests",
    html_content,
    "text/html"
)

# Check crawl status
if tracker.is_crawled("https://example.com"):
    print("Already crawled!")
```

## Architecture

### Components

1. **WebCrawler**: Core crawling engine with depth traversal
2. **RobotsChecker**: Robots.txt fetching, parsing, and compliance
3. **URLValidator**: URL validation and normalization
4. **RedisTracker**: Centralized state tracking and caching
5. **ParserRegistry**: Extensible content parser system
6. **ContentProcessors**: Binary content processing (PDF, images, etc.)

### Specialized Parsers

- **GitHubParser**: Extracts repository info, issues, PRs, and README content
- **PyPIParser**: Parses Python package info, dependencies, and documentation
- **NPMParser**: Handles Node.js packages, stats, and metadata
- **CratesParser**: Rust crate parser (delegates to b00t-c0re-lib)

### Data Flow

```
URL Input → Validation → Robots Check → Crawl → Parse → Store
    ↓           ↓           ↓         ↓       ↓       ↓
  Queue    URL Normalize  Cache     HTTP   Markdown Redis
```

## Testing

```bash
# Run all tests
uv run pytest

# Run with coverage
uv run pytest --cov=b00t_j0b_py --cov-report=term-missing

# Run specific test file
uv run pytest tests/test_crawler.py -v
```

## Development

### Adding New Parsers

1. Create parser class inheriting from `BaseParser`:
```python
from b00t_j0b_py.parsers.base import BaseParser, ParseResult

class MyParser(BaseParser):
    def can_parse(self, url: str) -> bool:
        return "mysite.com" in url
    
    def parse(self, url: str, content: str, content_type: str):
        # Parse content and return ParseResult
        pass
```

2. Register the parser:
```python
from b00t_j0b_py.parsers.base import registry
registry.register(MyParser())
```

### Adding Content Processors

1. Create processor class:
```python
class MyProcessor:
    def can_process(self, content_type: str) -> bool:
        return content_type == "application/my-format"
    
    def process(self, content: bytes, url: str):
        # Process binary content and return text
        pass
```

2. Register the processor:
```python
from b00t_j0b_py.content_processors import content_registry
content_registry.register(MyProcessor())
```

## Integration with b00t Ecosystem

This crawler is designed to integrate with:
- **b00t-grok**: Feed crawled content into knowledge base
- **b00t-c0re-lib**: Rust-based processing for performance-critical tasks
- **b00t-mcp**: MCP server for external tool integration
- **b00t-cli**: Command-line interface integration

## License

MIT License - see LICENSE.md for details.