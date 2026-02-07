# Codex Delegation Patterns

Reference for delegating code generation tasks to OpenAI Codex within b00t framework.

## Optimal Use Cases

**Codex excels at:**
- High-volume code generation
- Boilerplate & scaffolding
- API client generation
- Test fixture creation
- Legacy code translation (Python 2→3, JS→TS)
- Pattern-based code completion

**Codex struggles with:**
- Complex architectural decisions (use Claude)
- Multimodal tasks (use Gemini)
- Context-heavy refactoring (use Claude)
- Novel algorithm design (use Claude)

## Access Methods

### Method 1: OpenAI API (Direct)

```bash
# Via openai-codex.🤖 in b00t
curl https://api.openai.com/v1/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -d '{
    "model": "code-davinci-002",
    "prompt": "# Python function to validate email\ndef validate_email(email: str) -> bool:",
    "max_tokens": 150,
    "temperature": 0
  }'
```

### Method 2: GitHub Copilot CLI

```bash
# If gh copilot CLI installed
gh copilot suggest "create REST API endpoint for user management"
gh copilot explain "what does this regex do: ^[a-zA-Z0-9+_.-]+@[a-zA-Z0-9.-]+$"
```

### Method 3: B00t Wrapper (Future)

```bash
# Planned b00t integration
b00t codex generate \
  --template=rest-api \
  --spec=openapi.yaml \
  --output=src/api/
```

## Delegation Strategies

### Strategy 1: Template-Based Generation

```typescript
// Define template structure
const template = `
// {{LANGUAGE}} {{COMPONENT_TYPE}} for {{FEATURE}}
{{IMPORTS}}

{{INTERFACE_DEFINITIONS}}

{{IMPLEMENTATION}}

{{EXPORTS}}
`

// Let Codex fill in sections
const prompt = `
Generate TypeScript REST API controller for user management.

Required endpoints:
- POST /users (create)
- GET /users/:id (read)
- PUT /users/:id (update)
- DELETE /users/:id (delete)

Use Express.js framework, follow RESTful conventions.
`
```

**When to use**: Structured, pattern-based code generation

### Strategy 2: Example-Driven Generation

```python
# Provide example, ask for variations
prompt = """
# Example: User model
class User(BaseModel):
    id: str
    email: str
    created_at: datetime

# Now generate: Product model with fields:
# - id, name, price, category, stock_count, created_at
"""
```

**When to use**: Similar structures, different domains

### Strategy 3: Iterative Refinement

```bash
# Pass 1: Generate basic structure
codex_generate("Create user authentication service")

# Pass 2: Add specific features
codex_refine("Add JWT token generation with 24hr expiry")

# Pass 3: Add error handling
codex_refine("Add try-catch for database errors")
```

**When to use**: Complex generation requiring multiple passes

## Integration with Claude

### Pattern: Generate → Review → Integrate

```typescript
// 1. Codex generates code
const generatedCode = await codexGenerate({
  prompt: "Generate OAuth2 token refresh logic",
  language: "typescript"
})

// 2. Claude reviews for issues
const review = await claudeReview({
  code: generatedCode,
  focus: ["security", "error-handling", "edge-cases"]
})

// 3. Codex fixes issues (if simple)
const fixedCode = await codexFix({
  code: generatedCode,
  issues: review.simpleIssues
})

// 4. Claude handles complex fixes
const finalCode = await claudeFix({
  code: fixedCode,
  issues: review.complexIssues
})
```

### Pattern: Parallel Generation

```typescript
// Generate multiple variations in parallel
const [implA, implB, implC] = await Promise.all([
  codexGenerate({ prompt: basePrompt, temperature: 0 }),    // Conservative
  codexGenerate({ prompt: basePrompt, temperature: 0.5 }),  // Balanced
  codexGenerate({ prompt: basePrompt, temperature: 1.0 })   // Creative
])

// Claude selects best approach
const best = await claudeEvaluate({
  implementations: [implA, implB, implC],
  criteria: ["simplicity", "performance", "maintainability"]
})
```

## Prompt Engineering for Codex

### Effective Prompts

```python
# ✅ GOOD: Specific, constrained
prompt = """
Create a Python function that:
1. Accepts list of integers
2. Filters even numbers
3. Returns sum of squares
4. Includes type hints
5. Includes docstring with examples

def sum_of_even_squares(numbers: List[int]) -> int:
"""

# ❌ BAD: Vague, open-ended
prompt = "Make a function that processes numbers"
```

### Context Priming

```javascript
// Provide existing code context
const context = `
// Existing codebase style:
import { Request, Response } from 'express';

export const getUser = async (req: Request, res: Response) => {
  try {
    const user = await UserService.findById(req.params.id);
    return res.json(user);
  } catch (error) {
    return res.status(500).json({ error: error.message });
  }
};
`

const prompt = `
${context}

// Following the same pattern, implement:
export const updateUser = async (req: Request, res: Response) => {
`
```

## Code Quality Validation

### Post-Generation Checks

```bash
#!/bin/bash
# Validate Codex-generated code

# 1. Syntax check
if ! rustc --check generated.rs; then
    echo "❌ Syntax errors detected"
    exit 1
fi

# 2. Run linter
if ! cargo clippy -- -D warnings; then
    echo "⚠️ Linting issues found"
fi

# 3. Run tests
if ! cargo test; then
    echo "❌ Tests failed"
    exit 1
fi

# 4. Security scan
if ! cargo audit; then
    echo "🚩 Security vulnerabilities"
fi

echo "✅ Generated code validated"
```

### Integration with TDD

```rust
// 1. Write test first (human or Claude)
#[test]
fn test_email_validation() {
    assert!(validate_email("user@example.com"));
    assert!(!validate_email("invalid-email"));
    assert!(!validate_email(""));
}

// 2. Codex generates implementation
// Prompt: "Implement validate_email function to pass above tests"

// 3. Iterate until tests pass
// 4. Claude reviews for edge cases
// 5. Add additional tests if needed
```

## Token Efficiency

**Codex token costs** (code-davinci-002):
- Input: $0.02 / 1K tokens
- Output: $0.02 / 1K tokens

**Cost optimization:**
```typescript
// ❌ Wasteful: Regenerate entire file
prompt = `Regenerate complete 500-line file with this one change...`
cost = ~1000 tokens × $0.02 = $0.02

// ✅ Efficient: Generate only changed section
prompt = `Generate just the updateUser function following this pattern...`
cost = ~200 tokens × $0.02 = $0.004
```

**5x cost reduction** through targeted generation.

## Bulk Generation Workflows

### Workflow: API Client Generation

```bash
# From OpenAPI spec → Full API client
just generate-api-client openapi.yaml

# Behind the scenes:
# 1. Parse OpenAPI spec (Claude)
# 2. Generate models (Codex)
# 3. Generate endpoints (Codex)
# 4. Generate tests (Codex)
# 5. Review & integrate (Claude)
# 6. Validate (cargo test)
```

### Workflow: Migration Scripts

```bash
# Python 2.7 → 3.12 migration
just migrate-python27-to-312 src/

# Behind the scenes:
# 1. Analyze dependencies (Claude)
# 2. Generate 2to3 fixes (Codex)
# 3. Update type hints (Codex)
# 4. Fix async patterns (Claude)
# 5. Update tests (Codex)
# 6. Validate (pytest)
```

## Error Handling

### Common Codex Errors

**Syntax errors:**
```typescript
// Codex generated invalid syntax
const result = Codex.generate(prompt)

if (!validateSyntax(result)) {
  // Retry with explicit syntax constraints
  result = Codex.generate({
    ...prompt,
    constraints: "Valid TypeScript, no syntax errors"
  })
}
```

**Hallucinated APIs:**
```python
# Codex invents non-existent library
import fake_library  # ❌ Doesn't exist

# Solution: Validate imports
valid_imports = check_imports(generated_code)
if not valid_imports:
    prompt += "\nOnly use: requests, flask, sqlalchemy"
    regenerate()
```

**Incomplete generation:**
```rust
// Codex stops mid-function
fn incomplete_function(x: i32) -> i32 {
    let result = x * 2;
    // ... <generation cut off>

// Solution: Increase max_tokens or use iterative approach
```

## Compounding Patterns

### Pattern Library

```bash
# Capture successful generation patterns
b00t lfmf datum abstract codex-pattern <<EOF
Pattern: REST CRUD endpoint generation

Prompt structure:
1. Specify framework
2. List operations (CRUD)
3. Define data model
4. Specify error handling

Success rate: 95%
Manual fixes needed: <5%

Example: just generate-crud-api users
EOF
```

### Reusable Templates

```justfile
# justfile: Codify Codex workflows
generate-crud-api MODEL:
    #!/usr/bin/env bash
    # Generate complete CRUD API using Codex
    codex-generate \
        --template=crud \
        --model={{MODEL}} \
        --framework=express \
        --db=postgres

    # Claude reviews
    claude-task review-api --model={{MODEL}}

    # Run tests
    npm test api/{{MODEL}}
```

---

*Codex for volume. Claude for wisdom. Together they compound.* 🤖
