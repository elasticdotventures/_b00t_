# 🐝 browser-use-b00t-plugin

Chrome extension + CDP bridge for agent-driven browser automation.
Part of the [b00t hive](https://github.com/elasticdotventures/_b00t_) ecosystem.

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         WSL2 (b00t-rpa)                          │
│                                                                  │
│  b00t-rpa start ──ws──▶ 172.30.64.1:9223 (Python relay)         │
│  b00t-rpa plugin      ▶ Chrome --load-extension                  │
│                                                                  │
│  ┌─────────────────────┐    ┌──────────────┐                    │
│  │  ratatui TUI         │    │  ontology    │                     │
│  │  (cmd palette,       │    │  export      │                     │
│  │   script curation)   │    │  --mermaid   │                     │
│  └─────────────────────┘    └──────────────┘                    │
└──────────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────────┐
│                     Windows Host (Chrome)                         │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  b00t Browser Plugin (loaded via --load-extension)           │  │
│  │                                                              │  │
│  │  ┌──────────────┐   ┌──────────────┐   ┌────────────────┐   │  │
│  │  │ Content Script │   │ Side Panel    │   │ chrome.debugger│   │  │
│  │  │ DOM Enrichment │   │ Command       │   │ CDP Bridge     │   │  │
│  │  │ [data-b00t-*]  │   │ Palette +     │   │ Page/Runtime   │   │  │
│  │  │ MutationObsrvr │   │ Script Curation│  │ Screenshot     │   │  │
│  │  └───────┬───────┘   └──────┬───────┘   └───────┬────────┘   │  │
│  │          │                  │                    │            │  │
│  │          └──────────────────┴────────────────────┘            │  │
│  │                          │                                   │  │
│  │                          ▼                                   │  │
│  │              Page DOM → [data-b00t-*] enriched                │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                  │
│  Chrome 149 --remote-debugging-port=9222                          │
│        --remote-allow-origins=*                                   │
│        --load-extension=C:\b00t\browser-plugin                    │
└──────────────────────────────────────────────────────────────────┘
```

## Capabilities

### 1. DOM Enrichment (`content.js`)
Every interactive element on the page gets `[data-b00t-*]` attributes:

| Attribute     | Example                          | Purpose                                |
|---------------|----------------------------------|----------------------------------------|
| `data-b00t-type` | `button`, `input`, `link`, `form` | Element category for querySelector     |
| `data-b00t-role` | `submit`, `text`, `navigation`   | Semantic role within type              |
| `data-b00t-label` | `"Search"`, `"Submit"`          | Stable label (aria-label > placeholder > text) |
| `data-b00t-id` | `"email-input"`                  | Element ID if present                  |
| `data-b00t-name` | `"email"`                        | Form field name if present             |

**Why this matters:** CSS selectors like `.search-box__input--v2` break on every redesign.
`[data-b00t-type="input"][data-b00t-label="Search"]` is **stable** — it survives CSS refactors,
A/B tests, and class name changes.

### 2. CDP Bridge (`chrome.debugger` API)
The plugin attaches to the active tab via Chrome's debugger API and provides:
- `Runtime.evaluate` — execute arbitrary JS in page context
- `Page.captureScreenshot` — full-page + visible screenshots
- `Page.getLayoutMetrics` — page dimensions for full-page capture
- `Emulation.setDeviceMetricsOverride` — resize viewport for screenshots

### 3. Command Palette (Side Panel)
Fzf-like interface in the Chrome side panel:

| Command             | Example                              |
|---------------------|--------------------------------------|
| `navigate`          | `navigate https://example.com`       |
| `click`             | `click button Submit`                |
| `type`              | `type Email hello@b00t.ai`          |
| `select_option`     | `select_option Country AU`           |
| `submit`            | `submit`                             |
| `list_enriched`     | `list_enriched button` — shows all enriched buttons |
| `screenshot_full`   | Captures full-page PNG               |
| `get_tables`        | Extracts all table data as JSON      |
| `evaluate`          | `evaluate document.title`            |

### 4. Screen Capture
- **Visible tab**: `chrome.tabs.captureVisibleTab` (instant, visible area only)
- **Full page**: CDP `Page.captureScreenshot` with viewport resize (full document)

## Installation

```bash
# From WSL — copies extension to Windows, restarts Chrome with --load-extension
b00t-rpa plugin

# Manual: Open Chrome → chrome://extensions
# → Enable Developer Mode → Load unpacked → select this directory
```

## Usage

### From the Chrome Side Panel
1. Click the 🐝 b00t icon in the extensions toolbar
2. The side panel opens with the command palette
3. Type to filter commands, Enter to add to script
4. Ctrl+Enter to execute the curated script
5. Screenshot button captures the current page

### From WSL (headless RPA)
```bash
# Navigate + interact
b00t-rpa --url https://google.com --eval "document.querySelector('textarea').value='b00t'; document.querySelector('form').submit()"

# Full RPA script via TUI
b00t-rpa start

# List enriched elements
b00t-rpa --eval "JSON.stringify(window.__b00t?.findB00t('button') || [])"
```

## DOM Enrichment API (from Console / CDP)

```javascript
// Count enriched elements
window.__b00t.enrichedCount()

// Find elements by type
window.__b00t.findB00t('button')
// → [{type:'button', label:'Submit', selector:'[data-b00t-type="button"][data-b00t-label="Submit"]', ...}]

// Find by type + role
window.__b00t.findB00t('input', 'text')
// → [{type:'input', role:'text', label:'Email', ...}]

// Re-enrich after dynamic content loads
window.__b00t.enrichAll()
```

## File Structure

```
browser-plugin/
├── manifest.json       # Chrome extension v3 manifest
├── content.js          # DOM Enrichment Engine (injected into every page)
├── selector-forge.js   # propose→verify→settle selector resolution (see below)
├── selector-forge.test.js # node --test unit tests for selector-forge.js
├── b00t-client.js      # CDP bridge + RPA command definitions
├── panel.html          # Side panel UI (dark terminal theme)
├── panel.js            # Panel UI logic (search, filter, script curation)
├── icon.png            # Generated green dot icon
└── README.md           # This file
```

### 5. Selector Resolution (`selector-forge.js`)
Deterministic "propose → verify → settle" selector resolution, adapted from
[Intuned/selector-forge](https://github.com/Intuned/selector-forge)'s trust
boundary (see `_b00t_/datums/VENDOR-SELECTOR-FORGE.tomllmd`): candidates are
ranked by a stability heuristic (`#id` > `[data-b00t-*]` > `[name]` > class
> nth-child) and each is re-verified against the live DOM — the browser
decides, not a ranking score — before one is returned. No AI/network call;
this is the local, offline re-verification loop the datum's integration
point #2 describes, reused as a building block b00t's own automation
pipeline can call. Exposed as the `resolve_selector <type> <label>` command
in the side panel, routed via `chrome.tabs.sendMessage` to a bridge in
`content.js` (CDP `Runtime.evaluate` only reaches the MAIN world, and this
logic lives in the isolated content-script world alongside `document`).
Unit-tested with Node's built-in test runner:
```bash
node --test selector-forge.test.js
```

## Roadmap

- [x] DOM enrichment with stable [data-b00t-*] selectors
- [x] CDP bridge via chrome.debugger API
- [x] Side panel with command palette + script curation
- [x] Visible + full-page screenshot capture
- [x] MutationObserver for SPA compatibility
- [x] selector-forge propose→verify→settle resolution loop (issue #770 — local/offline; AI-backed ranking and MCP exposure still open)
- [ ] monty WASM MObject integration for animation
- [ ] Blender-MCP bridge for 3D visualization
- [ ] Record/replay: capture user interactions as b00t scripts
- [ ] Cross-tab orchestration (multi-page RPA workflows)
