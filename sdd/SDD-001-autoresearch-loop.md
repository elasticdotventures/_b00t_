# SDD-001: Karpathy Autoresearch Self-Reinforcing Loop

> **Status:** DRAFT → READY FOR IMPLEMENTATION | **PRD Reference:** None (SDD replaces PRD)
> **Components:** crawl → datum → classify → loop → new research targets
> **Dependencies:** crawl4ai or firecrawl, l3dg3rr, b00t grok assimilate, b00t datum

---

## 1. Problem Statement

Manual research requires humans to (1) find URLs, (2) run crawls, (3) classify content, (4) decide what to learn next. Karpathy's autoresearch pattern automates this: crawl a seed URL → classify → find related URLs → repeat. Each cycle enriches the b00t datum store.

---

## 2. Specification

### 2.1 Core Loop

```
seed_urls = [initial URLs]
for url in seed_urls:
    result = b00t crawl fetch url        # BinaryDocument
    shape = b00t detect result            # ledg3rr shape detection
    parsed = b00t parse result shape      # parser dispatch
    classified = b00t classify parsed     # ledg3rr classification
    new_urls = extract_links(parsed)      # find related URLs
    datum = b00t grok assimilates parsed  # ingest into knowledge store
    seed_urls.extend(new_urls if not visited)
```

### 2.2 Entry Points

```bash
# Single URL research
b00t crawl --url <URL> --depth <N> --loop    # depth 1 = just URL, N+1 = follow links

# Seed list research
b00t crawl --seed-file ~/.b00t/research_seeds.txt --depth 3 --loop

# Topical research (uses existing datums to find gaps)
b00t grok autoresearch --topic "rust mcp servers" --depth 2
```

### 2.3 Self-Reinforcement Loop

The loop is "self-reinforcing" because:
1. Each assimilated datum enriches `b00t grok ask` results
2. Future queries find more relevant links to crawl
3. ledg3rr classification improves with more examples in datum store
4. The research DAG (directed acyclic graph) grows: seed → crawled → classified → new seeds

```
        Seed URL
          │
     ┌────┼────┐
     ▼    ▼    ▼
  Datum1 Datum2 Datum3
     │     │      │
  link   link   link
     │     │      │
     ▼     ▼      ▼
  Datum4 Datum5 Datum6
          ...loops...
```

### 2.4 Termination Conditions

| Condition | Default Value | Override |
|---|---|---|
| Max depth | 3 | `--depth N` |
| Max datums per run | 50 | `--max-datums N` |
| Max tokens consumed | 100K | `--max-tokens N` |
| Domain whitelist | seed URL domain | `--domains <csv>` |
| Domain blacklist | [] | `--no-domains <csv>` |
| Rate limit | 1 request/5s | `--rate-limit <ms>` |

### 2.5 Output Contract

```json
{
  "seed_urls": 3,
  "datums_assimilated": 12,
  "new_urls_discovered": 47,
  "loops_completed": 2,
  "termination_reason": "max_depth_reached",
  "datum_ids": ["<hash1>", "<hash2>"],
  "rotel_trace_id": "<span_id>"
}
```

---

## 3. Implementation Notes

### 3.1 Fallback Chain
- No firecrawl installed → use crawl4ai
- No crawl4ai installed → use curl (HTML only, no JS rendering)
- No crawler at all → `b00t crawl --curl <url>` uses plain HTTP GET

### 3.2 Link Extraction
- HTML: extract `href` attributes from `<a>` tags
- Markdown: extract `[text](url)` patterns
- JSON: extract URL strings matching `https?://` pattern
- RSS: extract `<link>` elements from feed items

### 3.3 Deduplication
- Normalize URLs: lowercase, remove fragments, strip trailing `/`
- Check against `.b00t/visited_urls.txt` (gitignored)
- Skip URLs already in datum store (check by URL in datum metadata)

### 3.4 Rate Limiting
- Respect `robots.txt` crawl-delay
- Default 5s between requests
- Configurable via `~/.b00t/crawl_config.toml`

---

## 4. Integration with Existing Components

| Component | Integration |
|---|---|
| `crawl4ai.cli.toml` | Use `crawl4ai --mode fast` for static, `--browser` for dynamic |
| `firecrawl.cli.toml` | Use when created, fallback to crawl4ai |
| `b00t grok assimilate` | Datum ingestion endpoint |
| `b00t datum validate` | Post-loop integrity check |
| `l3dg3rr.cli.toml` | Shape detection + classification |
| rotel | Trace the full loop (span per iteration) |

---

## 5. Stage Gates

### Gate 1: Basic Crawl + Assimilate ✅
- [ ] `b00t crawl --url <URL> --assimilate` works for single URL
- [ ] Datum stored in git blob, .datum.toml created
- [ ] **Validate:** `b00t datum list` shows new datum after crawl

### Gate 2: Link Extraction + Depth 🔴
- [ ] Links extracted from HTML, markdown, JSON, RSS
- [ ] `--depth 2` follows links from seed URL
- [ ] Deduplication prevents re-crawling same URL
- [ ] **Validate:** Crawl docs site → 10+ datums created from linked pages

### Gate 3: Self-Reinforcing Loop 🔴
- [ ] Loop continues discovery until termination condition
- [ ] Terminates cleanly on max depth/datums/tokens
- [ ] Rotel trace ID included in output
- [ ] **Validate:** Seed with 1 URL → loop converges with N datums

### Gate 4: Topical Autoresearch 🔴
- [ ] `b00t grok autoresearch --topic <topic>` seeds from b00t datum gaps
- [ ] Identifies missing knowledge areas in existing datum store
- [ ] Prioritizes URLs that fill knowledge gaps
- [ ] **Validate:** Gap in "rust mcp servers" → discovers relevant documentation

---

## 6. Debug Support

```bash
# Trace the full loop with rotel
b00t crawl --url <URL> --depth 2 --loop --debug 3

# Dry-run: show what would be crawled without assimilating
b00t crawl --url <URL> --depth 2 --dry-run --debug 2

# Resume interrupted loop from checkpoint
b00t crawl --resume --debug 1
```

---

# b00t:map v1
# summary: SDD-001 — Karpathy autoresearch loop: crawl → classify → ingest → discover new URLs → repeat
# tags: karpathy, autoresearch, crawl, self-reinforcing, loop, datum, assimilation, research
# tier: frontier
# cmds: b00t crawl --url <URL> --depth N --loop, b00t grok autoresearch --topic <topic>
# complexity: 8
