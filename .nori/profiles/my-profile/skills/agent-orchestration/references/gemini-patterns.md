# Gemini Delegation Patterns

Reference for delegating multimodal & research tasks to Google Gemini within b00t framework.

## Optimal Use Cases

**Gemini excels at:**
- Multimodal input (images, video, audio, PDF)
- Real-time web research with grounding
- Document analysis (PDFs, presentations)
- Long-context understanding (1M+ tokens)
- Multilingual translation & analysis
- Code execution in context

**Gemini struggles with:**
- Complex refactoring (use Claude)
- High-volume code gen (use Codex)
- Deep reasoning chains (use Claude)

## Access via geminicli

### Method 1: Text Research

```bash
# geminicli installed in _b00t_
geminicli research \
  "OAuth2 security best practices 2025" \
  --citations \
  --output oauth-research.md
```

### Method 2: Multimodal Analysis

```bash
# Analyze image
geminicli analyze screenshot.png \
  --prompt "Extract UI components and layout structure"

# Analyze PDF
geminicli analyze design-doc.pdf \
  --prompt "Summarize architecture decisions"

# Analyze video
geminicli analyze demo.mp4 \
  --prompt "Extract user interaction patterns"
```

### Method 3: Code Execution

```bash
# Gemini can run code to validate
geminicli execute \
  --code "analyze-performance.py" \
  --data "metrics.json"
```

## Integration Patterns

### Pattern: Research → Design → Implement

```typescript
// 1. Gemini researches current patterns
const research = await gemini.research({
  query: "Kubernetes autoscaling patterns 2025",
  focus: ["cost optimization", "performance"],
  citations: true
})

// 2. Claude designs architecture
const design = await claude.designArchitecture({
  requirements: research.findings,
  constraints: research.bestPractices
})

// 3. Codex generates config
const manifests = await codex.generateK8sManifests({
  design: design.architecture
})

// 4. Gemini validates against research
const validation = await gemini.validate({
  implementation: manifests,
  bestPractices: research.bestPractices
})
```

### Pattern: Multimodal Understanding

```typescript
// Analyze screenshot → Generate code
const ui = await gemini.analyzeImage({
  image: "design-mockup.png",
  prompt: "Extract components, colors, spacing"
})

const componentTree = await claude.structureUI({
  analysis: ui.components
})

const code = await codex.generateReactComponents({
  structure: componentTree,
  styling: ui.styling
})
```

## Prompt Engineering

### Research Prompts

```bash
# ✅ GOOD: Specific, constrained, time-bound
geminicli research \
  "WebAssembly security vulnerabilities discovered in 2024-2025" \
  --require-citations \
  --sources "cvedetails.com, github.com, arxiv.org"

# ❌ BAD: Vague, no constraints
geminicli research "wasm stuff"
```

### Multimodal Prompts

```bash
# ✅ GOOD: Clear extraction goals
geminicli analyze architecture-diagram.png \
  --prompt "Extract: 1) Service names 2) Communication protocols 3) Data stores 4) External dependencies"

# ❌ BAD: Open-ended
geminicli analyze architecture-diagram.png \
  --prompt "What's in this image?"
```

## Long Context Advantage

**Gemini 1.5 Pro**: 1M token context

**Use cases:**
```bash
# Analyze entire codebase in single pass
geminicli analyze-codebase \
  --files "src/**/*.rs" \
  --prompt "Find all authentication touch-points"

# Process long documents
geminicli summarize \
  --file "500-page-spec.pdf" \
  --format "executive-summary + decision-points"
```

## Validation Workflows

### Pattern: Gemini as Validator

```typescript
// After implementation
const implementation = readFiles("src/oauth/**")

const validation = await gemini.validate({
  implementation: implementation,
  prompt: `Compare this OAuth implementation against:
  1. RFC 6749 (OAuth 2.0 spec)
  2. 2025 OWASP recommendations
  3. Common CVE patterns

  Report: Compliance gaps + security risks`
})
```

### Pattern: Gemini as Fact-Checker

```typescript
// After Claude designs architecture
const design = claude.generateDesignDoc()

const factCheck = await gemini.research({
  claims: extractClaims(design),
  verify: true,
  citations: true
})

if (factCheck.errors.length > 0) {
  // Claude revises based on correct info
  design = await claude.revise({
    original: design,
    corrections: factCheck.corrections
  })
}
```

## Token Efficiency

**Gemini pricing** (1.5 Pro):
- Input (≤128K): $0.00125 / 1K tokens
- Input (>128K): $0.005 / 1K tokens
- Output: $0.005 / 1K tokens

**Strategy: Front-load research**
```bash
# Heavy research upfront (Gemini)
research = gemini.research(...)  # 50K tokens input

# Efficient generation (Codex + Claude)
design = claude.design(research.summary)  # 5K tokens
code = codex.generate(design)  # 10K tokens

# Amortize research cost across multiple tasks
```

## Compounding Workflows

### Workflow: Multimodal Documentation

```justfile
# Generate docs from screenshots + code
document-ui COMPONENT:
    # 1. Gemini analyzes screenshots
    geminicli analyze ui/{{COMPONENT}}/*.png \
        --output docs/{{COMPONENT}}/analysis.md

    # 2. Claude structures documentation
    claude-task document-component \
        --analysis docs/{{COMPONENT}}/analysis.md \
        --code src/{{COMPONENT}}

    # 3. Codify for future components
    b00t lfmf datum abstract multimodal-docs
```

### Workflow: Research-Driven Development

```justfile
# Research → Design → Implement → Validate
research-driven-feature FEATURE:
    #!/usr/bin/env bash
    # 1. Gemini: Research current patterns
    geminicli research "{{FEATURE}} best practices 2025" \
        --citations > research/{{FEATURE}}.md

    # 2. Claude: Design based on research
    claude-task design-feature \
        --feature {{FEATURE}} \
        --research research/{{FEATURE}}.md

    # 3. Codex: Generate implementation
    codex-generate --spec design/{{FEATURE}}.md

    # 4. Gemini: Validate against research
    geminicli validate \
        --implementation src/{{FEATURE}} \
        --standards research/{{FEATURE}}.md

    # 5. Codify successful pattern
    just codify-pattern {{FEATURE}}
```

## Error Handling

### Hallucination Detection

```typescript
// Gemini can hallucinate citations
const research = await gemini.research(query)

// Validate citations exist
const validCitations = await validateCitations(research.sources)

if (validCitations < 0.8) {
  // Re-run with stricter constraints
  research = await gemini.research({
    ...query,
    requireVerifiedSources: true
  })
}
```

### Multimodal Failures

```typescript
// Image analysis might fail
try {
  const analysis = await gemini.analyzeImage(img)
} catch (UnsupportedFormatError) {
  // Convert format
  img = await convertImage(img, 'png')
  analysis = await gemini.analyzeImage(img)
} catch (LowQualityError) {
  // Enhance image
  img = await enhanceImage(img)
  analysis = await gemini.analyzeImage(img)
}
```

## B00t Integration

### Datum Capture

```bash
# After successful multimodal workflow
b00t lfmf datum abstract gemini-pattern <<EOF
Pattern: UI mockup → Component code

Workflow:
1. Designer provides Figma screenshot
2. Gemini extracts: components, styling, layout
3. Claude structures: component hierarchy
4. Codex generates: React components + CSS
5. Gemini validates: against design system

Success: 90% match to designer intent
Time: 30min (vs 4hr manual)

Codified: just mockup-to-code <screenshot>
EOF
```

### Justfile Integration

```justfile
# B00t Gemini workflows
research TOPIC:
    geminicli research "{{TOPIC}} best practices 2025" \
        --citations \
        --output _b00t_/research/{{TOPIC}}.md

analyze-pdf FILE:
    geminicli analyze "{{FILE}}" \
        --prompt "Extract: decisions, requirements, constraints" \
        --output analysis/$(basename {{FILE}} .pdf).md

validate-against-research IMPL RESEARCH:
    geminicli validate \
        --implementation {{IMPL}} \
        --standards {{RESEARCH}} \
        --output validation-report.md
```

---

*Gemini sees more. Claude thinks deeper. Codex codes faster. Use each for their strength.* 🔮
