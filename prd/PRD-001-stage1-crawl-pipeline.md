# PRD-001: b00t Generic Crawl Pipeline

> **PRD:** PRD-001 / Stage 1 | **Status:** DRAFT | **Parent:** PRD-001
> **Components:** BinaryDocument type, ledg3rr shape detection, parser registry, datum ingress
> **Dependencies:** crawl4ai.cli.toml, l3dg3rr.cli.toml, k0mmand3r dispatcher

---

## 1. Problem Statement

Current crawl tools (crawl4ai, firecrawl) are siloed and format-hardcoded. Fetching a URL requires knowing the tool AND specifying the output format upfront (`--format markdown`). A unified generic crawl that fetches as raw binary, detects shape, and delegates to the correct parser is needed.

---

## 2. Solution: Generic Crawl as 5-Step Chain

### Step 1: Fetch (BinaryDocument)
- **Input:** URL
- **Output:** BinaryDocument { raw_bytes: Vec<u8>, source_url, fetched_at }
- **Backend (fallback order):**
  1. crawl4ai (`b00t cli run crawl4ai --browser <url>`)
  2. firecrawl (once datum created)
  3. curl/wget (raw HTTP fallback)
- **Stub:** If no crawler backend installed, use curl + warn
- **Existing code:** curl.apt.toml, crawl4ai.cli.toml, bash aliases

### Step 2: Shape Detection (ledg3rr)
- **Input:** BinaryDocument.raw_bytes
- **Output:** ClassificationResult { mime_type, shape_hint, confidence, raw_bytes }
- **Algorithm:**
  1. `file --mime` on raw bytes for coarse type
  2. Byte-pattern matching: `<html` → HTML, `{` + `":` → JSON, `%PDF` → PDF, `# ` → markdown/headers
  3. ledg3rr `tomllmd-validation` for structured types (TOML, YAML, XML, RSS)
  4. Confidence scoring: byte_magic 60% + pattern 30% + ledg3rr 10%
- **Stub:** Unrecognized shape → "text/plain" with confidence 0.3
- **Existing code:** l3dg3rr.cli.toml, b00t-mcp derive_mcp_tools

### Step 3: Parse
- **Input:** ClassificationResult
- **Output:** ParsedContent { content_type, text_body, metadata }
- **Dispatch table (k0mmand3r routes based on shape_hint):**

| Shape Hint | Parser | Existing Code |
|---|---|---|
| HTML | crawl4ai/firecrawl markdown | crawl4ai.cli.toml |
| JSON | jq structured filter | jqfzf in ubuntu.🐧/bin |
| TOML/YAML | toml-cli, yq | toml-cli.sh, yq |
| PDF | pymupdf | ocr-and-documents skill |
| RSS/Atom | blogwatcher-cli | blogwatcher skill |
| Markdown | pass-through | n/a |
| text/plain | pass-through | n/a |

- **Stub:** Unknown parser → return raw bytes as text, log warning

### Step 4: Classify (ledg3rr)
- **Input:** ParsedContent
- **Output:** TypedDocument { parsed_content, datum_class, tags, recommendations }
- **ledg3rr shape-matching against known categories:**
  - CLI docs → `b00t learn` candidate
  - API spec → integration datum
  - Config file → system configuration knowledge
  - Log/output → debug reference
- **Recommendations:** Deterministically triggered, e.g.:
  - "cpu load is healthy" vs "unhealthy" based on metric thresholds
  - "This looks like GitHub PR review docs — `b00t learn github-pr-workflow`"
- **Existing code:** l3dg3rr.cli.toml, b00t datum classify

### Step 5: Ingest (b00t grok learn)
- **Input:** TypedDocument
- **Output:** Datum stored in ~/.b00t git object store
- **Process:**
  1. Write content as git blob: `git -C ~/.b00t hash-object -w`
  2. Create .datum.toml file referencing blob hash
  3. Validation: `b00t datum validate` reads blob
- **Stub:** Dry-run mode (outputs to stdout without storing)
- **Existing code:** b00t grok learn, b00t datum, mcp_transports-*.datum.toml pattern

---

## 3. Data Types (Rust)

```rust
pub struct BinaryDocument {
    pub raw_bytes: Vec<u8>,
    pub source_url: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    pub http_status: u16,
}

pub struct ClassificationResult {
    pub mime_type: String,
    pub shape_hint: String,    // "html", "json", "pdf", "markdown", "text"
    pub confidence: f64,       // 0.0 - 1.0
    pub raw_bytes: Vec<u8>,    // forward original
}

pub struct ParsedContent {
    pub content_type: String,
    pub text_body: String,     // parsed text
    pub metadata: HashMap<String, String>,
}

pub struct TypedDocument {
    pub parsed_content: ParsedContent,
    pub datum_class: String,   // "cli-docs", "api-spec", "config", etc.
    pub tags: Vec<String>,
    pub recommendations: Vec<String>,
}
```

---

## 4. Integration Points

| Step | Existing Component | Modification |
|---|---|---|
| Fetch | crawl4ai.cli.toml | Add `type="crawl"` section with binary-output flag |
| Fetch | firecrawl.cli.toml (future) | Same pattern as crawl4ai |
| Shape | l3dg3rr.cli.toml | Add `feature = "shape-detection"` for mime/pattern matching |
| Parse | k0mmand3r dispatcher | Add classification-based routing rules |
| Classify | l3dg3rr.tomllmd-validation | Extend with content categorization rules |
| Ingest | `b00t grok learn` | Accept ParsedContent as input pipe |

---

## 5. Stage Gates

### Gate 1: BinaryDocument Fetch ✅ (Stub OK)
- [ ] BinaryDocument struct defined in b00t-rust or as shell wrapper
- [ ] Fetch works with curl minimum (crawler backends optional)
- [ ] HTTP status codes tracked
- [ ] **Validation:** `b00t crawl --fetch <url> | file` returns correct mime

### Gate 2: Shape Detection (ledg3rr) 🔴
- [ ] Shape hints match for: HTML, JSON, TOML, YAML, PDF, RSS, Markdown, Plain Text
- [ ] Confidence scoring implemented
- [ ] Fallback to text/plain for unknown shapes
- [ ] **Validation:** Feed known files → `b00t crawl --detect` outputs correct shapes

### Gate 3: Parser Dispatch 🔴
- [ ] Parser registry maps shape_hint → tool invocation
- [ ] HTML → curl4ai/firecrawl markdown parsing
- [ ] JSON → jq filter
- [ ] TOML/YAML → toml/yq parse
- [ ] PDF → pymupdf pipe through
- [ ] **Validation:** Crawl URLs of each type → correct parser invoked, clean output

### Gate 4: Classification (ledg3rr) 🔴
- [ ] TypedDocument.datum_class populated from ledg3rr rules
- [ ] Recommendations generated deterministically
- [ ] **Validation:** Parse a CLI docs page → recommends `b00t learn <topic>`

### Gate 5: Ingestion (existing b00t grok) 🔴
- [ ] ParsedContent pipes into `b00t grok learn`
- [ ] Datum stored as git blob
- [ ] .datum.toml created with blob reference
- [ ] **Validation:** `b00t crawl --ingest <url>` → `b00t datum list` shows new datum

---

# b00t:map v1
# summary: PRD-001 Stage 1 — Generic crawl pipeline with BinaryDocument, ledg3rr shape detection, parser dispatch, ingress
# tags: crawl, binary-document, ledg3rr, shape-detection, parser-registry, datum-ingress
# tier: frontier
# cmds: b00t crawl --fetch <url> --debug 2, b00t datum list
# complexity: 8
