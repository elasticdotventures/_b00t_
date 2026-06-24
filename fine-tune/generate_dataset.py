#!/usr/bin/env python3
"""Generate b00t-aligned training dataset from datums, learn files, and justfile recipes.

Output: JSONL in Alpaca/chatml format for unsloth QLoRA fine-tuning.
Target: 5000+ rows covering b00t idioms, k0mmand3r syntax, datum patterns, hive ops.

Corpus layers:
  Layer 1 (syntactic)  — datums, learn, hive profiles, AGENTS, justfile
  Layer 2 (generative) — teach model to WRITE b00t syntax from descriptions
  Layer 3 (library)    — NeumannStore knowledge corpus via b00t grok ask
"""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

# ─── Corpus root resolution ────────────────────────────────────────────────────
# Prefer explicit --b00t-root arg; fall back to repo root (two dirs up from this
# file), then ~/.dotfiles, then ~/.b00t — first that contains _b00t_/ wins.

def _find_b00t_root() -> Path:
    candidates = [
        Path(__file__).resolve().parent.parent,   # repo root (fine-tune/../)
        Path.home() / ".dotfiles",
        Path.home() / ".b00t",
    ]
    for c in candidates:
        if (c / "_b00t_").is_dir():
            return c
    return candidates[0]

B00T_ROOT = _find_b00t_root()

def _sources(root: Path) -> dict[str, list[Path]]:
    return {
        "datums":       sorted((root / "_b00t_/datums").glob("*.tomllmd")),
        "learn":        sorted((root / "_b00t_/learn").glob("*.md")),
        "hive_profiles":sorted((root / "_b00t_").glob("*.hive.toml")),
        "justfile":     [p for p in [root / "justfile"] if p.exists()],
        "agents":       sorted((root / "AGENTS").glob("*.md")),
        "skills":       sorted((root / "plugins").glob("*/skills/*/SKILL.md"))
                      + sorted((root / "_b00t_/skills").glob("*/SKILL.md")),
    }

# ─── Template patterns ─────────────────────────────────────────────────────────
# Used only for reference — actual rows are dicts with instruction/input/response.

ALPACA_TEMPLATE = """\
### Instruction:
{instruction}

### Input:
{input}

### Response:
{response}"""

CHATML_TEMPLATE = """\
<|im_start|>user
{instruction}
<|im_end|>
<|im_start|>assistant
{response}
<|im_end|>"""


def extract_title(content: str, fallback: str) -> str:
    """Extract first heading or title from content."""
    for line in content.splitlines():
        line = line.strip()
        if line.startswith("# ") or line.startswith("## "):
            return line.lstrip("#").strip()
    return fallback


def parse_datum_instruction(path: Path, content: str) -> list[dict[str, str]]:
    """Parse a .tomllmd datum into instruction/response pairs."""
    rows: list[dict[str, str]] = []
    name = path.stem

    # Extract the executive summary (comment section)
    exec_summary = ""
    for line in content.splitlines():
        if "# ───" in line and "Executive" in line:
            continue
        if line.startswith("#") and not line.startswith("# b00t:map"):
            exec_summary += line.lstrip("#").strip() + "\n"
        elif line.startswith("[") and exec_summary:
            break

    if exec_summary.strip():
        rows.append({
            "instruction": f"What is the {name} datum?",
            "input": "",
            "response": exec_summary.strip(),
        })

    # Parse usage sections
    in_usage = False
    usage_lines: list[str] = []
    for line in content.splitlines():
        if "[[b00t.usage]]" in line:
            in_usage = True
            usage_lines = []
        elif in_usage:
            if line.startswith("["):
                in_usage = False
                if usage_lines:
                    desc = ""
                    cmd = ""
                    for ul in usage_lines:
                        if "description" in ul:
                            desc = ul.split("=", 1)[-1].strip().strip('"')
                        if "command" in ul:
                            cmd = ul.split("=", 1)[-1].strip().strip('"')
                    if desc and cmd:
                        rows.append({
                            "instruction": f"How do I {desc.lower()}?",
                            "input": "",
                            "response": f"Run: `{cmd}`",
                        })
                usage_lines = []
            else:
                usage_lines.append(line)

    return rows


def parse_learn_instruction(path: Path, content: str) -> list[dict[str, str]]:
    """Parse a learn markdown file into instruction/response pairs."""
    rows: list[dict[str, str]] = []
    title = extract_title(content, path.stem)

    # Extract code blocks as examples
    code_blocks = re.findall(r"```(?:bash|toml|rhai|python|rust)\n(.*?)```", content, re.DOTALL)
    for block in code_blocks[:3]:  # max 3 per file
        rows.append({
            "instruction": f"Show me an example of {title}",
            "input": "",
            "response": block.strip(),
        })

    return rows


def parse_justfile(content: str) -> list[dict[str, str]]:
    """Parse justfile recipes into instruction/response pairs."""
    rows: list[dict[str, str]] = []
    recipe_pattern = re.compile(r"^([a-zA-Z][a-zA-Z0-9_-]+):.*?\n(?:^    .*\n?)*", re.MULTILINE)
    for match in recipe_pattern.finditer(content):
        block = match.group(0)
        name = match.group(1)
        lines = block.splitlines()
        if len(lines) < 2:
            continue
        # First line after the recipe name is the description if it starts with #
        desc = ""
        body = "\n".join(l for l in lines[1:] if l.strip())
        for l in lines[1:]:
            l = l.strip()
            if l.startswith("#"):
                desc = l.lstrip("#").strip()
            elif l:
                break
        instruction = desc or f"Run the {name} recipe"
        rows.append({
            "instruction": instruction,
            "input": "",
            "response": f"```bash\n{body.strip()}\n```",
        })
    return rows


def parse_agent_instruction(path: Path, content: str) -> list[dict[str, str]]:
    """Parse an AGENTS role file into instruction/response pairs."""
    rows: list[dict[str, str]] = []
    role_name = path.stem.replace("--role=", "").replace(".md", "")

    # Extract mission and responsibilities
    in_mission = False
    mission_lines: list[str] = []
    in_resp = False
    resp_lines: list[str] = []

    for line in content.splitlines():
        if line.startswith("## Mission"):
            in_mission = True
            mission_lines = []
            continue
        if line.startswith("## Core"):
            in_mission = False
            in_resp = True
            resp_lines = []
            continue
        if line.startswith("## "):
            in_mission = False
            in_resp = False
        if in_mission and line.strip():
            mission_lines.append(line.strip())
        if in_resp and line.strip():
            resp_lines.append(line.strip())

    if mission_lines:
        rows.append({
            "instruction": f"What is the mission of the {role_name} role?",
            "input": "",
            "response": " ".join(mission_lines),
        })
    if resp_lines:
        resp_text = "\n".join(resp_lines)
        rows.append({
            "instruction": f"What are the responsibilities of the {role_name} role?",
            "input": "",
            "response": resp_text,
        })

    return rows


# ─── Layer 2: Generative tasks ────────────────────────────────────────────────
# Teach the model to PRODUCE b00t syntax, not just answer questions about it.
# These templates create (description → artifact) pairs.

_DATUM_SKELETON = '''\
[b00t.schema]
version   = "1"
type      = "{datum_type}"
type_tags = ["{datum_type}", "FILL_IN_TAGS"]

[datum]
name        = "FILL_IN_NAME"
description = "FILL_IN_DESCRIPTION"

# b00t:map v1
# summary: FILL_IN_SUMMARY
# tags: FILL_IN_TAGS
# tier: sm0l|ch0nky|frontier
# cmds: FILL_IN_COMMANDS
# complexity: 1-10
'''

_BMAP_EXAMPLE = '''\
# b00t:map v1
# summary: {summary}
# tags: {tags}
# tier: {tier}
# cmds: {cmds}
# complexity: {complexity}
'''

_SKILL_FRONTMATTER = '''\
---
name: {name}
description: {description}
tier: {tier}
depends_on: [{depends_on}]
unlocks: [{unlocks}]
---
'''

GENERATIVE_TASKS: list[dict[str, str]] = [
    # ── Datum authoring ───────────────────────────────────────────────────────
    {
        "instruction": "Write a b00t skill datum skeleton for a new skill called 'semantic-search'.",
        "input": "",
        "response": _DATUM_SKELETON.format(datum_type="skill"),
    },
    {
        "instruction": "Write a b00t MCP datum skeleton for an MCP server that provides weather data.",
        "input": "",
        "response": _DATUM_SKELETON.format(datum_type="mcp"),
    },
    {
        "instruction": "What are the required fields in a b00t .tomllmd datum?",
        "input": "",
        "response": (
            "Every .tomllmd datum must have:\n"
            "  [b00t.schema] with version, type, type_tags\n"
            "  A b00t:map tail-map in the last ≤10 lines:\n"
            "    # b00t:map v1\n"
            "    # summary: one-line description\n"
            "    # tags: comma, separated\n"
            "    # tier: sm0l|ch0nky|frontier\n"
            "    # cmds: example command\n"
            "    # complexity: 1-10"
        ),
    },
    # ── b00t:map tail-map generation ─────────────────────────────────────────
    {
        "instruction": "Write the b00t:map tail-map for a datum that describes a Qdrant vector store MCP.",
        "input": "",
        "response": _BMAP_EXAMPLE.format(
            summary="Qdrant vector store MCP — semantic search over embedded knowledge",
            tags="mcp, qdrant, vector-store, semantic-search, rag",
            tier="ch0nky",
            cmds="b00t mcp install qdrant claudecode",
            complexity=4,
        ),
    },
    {
        "instruction": "Write the b00t:map tail-map for a frontier-tier PRD about BFO ontology integration.",
        "input": "",
        "response": _BMAP_EXAMPLE.format(
            summary="PRD — BFO/OWL2 ontology integration for b00t datum type system",
            tags="prd, bfo, owl2, ontology, datum-types, frontier",
            tier="frontier",
            cmds="b00t ontology sparql --subject b00t:Datum --predicate all",
            complexity=9,
        ),
    },
    # ── SKILL.md frontmatter authoring ───────────────────────────────────────
    {
        "instruction": "Write a SKILL.md YAML frontmatter for a skill called 'code-review' that unlocks b00t_grok_digest.",
        "input": "",
        "response": _SKILL_FRONTMATTER.format(
            name="code-review",
            description="Systematic code review using certainty-graded findings",
            tier="ch0nky",
            depends_on="six-sigma",
            unlocks="b00t_grok_digest, b00t_grok_ask",
        ),
    },
    # ── DatumType classification ──────────────────────────────────────────────
    {
        "instruction": "What b00t DatumType should I use for a file that describes how to run Qdrant in Docker?",
        "input": "",
        "response": "DatumType::Docker (.docker.toml). It describes a containerized service. Use DatumType::Stack if it composes multiple services.",
    },
    {
        "instruction": "What b00t DatumType should I use for a Python CLI tool like 'crawl4ai'?",
        "input": "",
        "response": "DatumType::Cli (.cli.toml). CLI datums describe detect/install/update/validate for command-line tools.",
    },
    {
        "instruction": "What is the difference between a b00t Skill and a Role datum?",
        "input": "",
        "response": (
            "Skill (DatumType::Skill, .skill.toml): a BFO:RealizableEntity / UFO:Moment::Mode. "
            "Realized when an agent executes it. Has unlocks: [] field granting MCP tool access. "
            "Loaded via `b00t learn <skill>` — earns AuthZ.\n\n"
            "Role (DatumType::Role, .role.toml): a BFO:RealizableEntity / UFO:Endurant::Role. "
            "Played by an Agent in a relational context. Loaded via `b00t whoami --role=<role>`. "
            "Aggregates required skills but does not itself unlock tools."
        ),
    },
    # ── HiveProfile and tier routing ─────────────────────────────────────────
    {
        "instruction": "Which cognitive tier should handle a DDC code lookup?",
        "input": "",
        "response": "sm0l. Table lookups are deterministic retrieval tasks — exact code + label from local JSONL cache. sm0l is sufficient and cheapest. Route to ch0nky only if classification (which DDC class fits this novel concept?) is needed.",
    },
    {
        "instruction": "How do I activate a hive profile for RTX 3090 inference?",
        "input": "",
        "response": "b00t hive activate inference-qwen36-27b  # or the relevant .hive.toml profile name\nCheck available profiles: b00t hive list\nDry-run resource gate: b00t hive plan=inference-qwen36-27b",
    },
    # ── OODA and auto-research ────────────────────────────────────────────────
    {
        "instruction": "What is the b00t OODA loop and how does enable_autoresearch work?",
        "input": "",
        "response": (
            "b00t's OODA loop (b00t-c0re-lib::ooda) has phases: Idle→Observing→Orienting→Deciding→Acting→Reviewing→Complete.\n"
            "OodaConfig.enable_autoresearch=true triggers a sub-cycle during Orient when an observation is complex (>100 chars). "
            "The sub-cycle gathers additional context before proceeding to Decide. "
            "Used in the library auto-learn loop to generate research frontier items after each PDF ingestion."
        ),
    },
    # ── Rust type system ─────────────────────────────────────────────────────
    {
        "instruction": "How do I add a new DatumType variant to b00t?",
        "input": "",
        "response": (
            "1. Add the variant to datum_type_table! macro in b00t-cli/src/lib.rs\n"
            "   Format: (VariantName, \"prefix\", \".suffix\", \"description\")\n"
            "2. datum_nodes() auto-derives from all_variants() + type_prefix() — zero maintenance.\n"
            "3. Run `cargo test -p b00t-cli` — datum_nodes_covers_all_variants will catch missing coverage.\n"
            "4. Create a corresponding *.suffix.toml datum in _b00t_/ to validate the new type."
        ),
    },
    {
        "instruction": "What does #[derive(HolonEmit)] do in b00t?",
        "input": "",
        "response": (
            "#[derive(HolonEmit)] is a proc-macro from b00t-reflect crate. "
            "Applied to an enum, it generates emit_nodes() -> Vec<b00t_reflect_types::HolonNode> "
            "where each variant becomes a HolonNode {id, label, kind, z_layer, semantic_type}. "
            "VizDomain in holon-viz uses it so the type graph is a compile-time artifact — "
            "no hand-maintained gen.rs. DatumType::datum_nodes() uses the same pattern via "
            "all_variants() + type_prefix()."
        ),
    },
    # ── Cake / reward ────────────────────────────────────────────────────────
    {
        "instruction": "What does 🍰 mean in b00t?",
        "input": "",
        "response": "🍰 is the b00t alignment reward. Agents earn cake by hitting milestones (cognitive benchmarks), completing tasks correctly, and following Core Laws (DRY, TDD-first, Postel's Law). It's recorded via `b00t lfmf` as a tribal lesson. Misalignment breaks the BMI link.",
    },
]

def parse_skill_frontmatter(path: Path, content: str) -> list[dict[str, str]]:
    """Parse a SKILL.md YAML frontmatter into instruction/response pairs."""
    rows: list[dict[str, str]] = []
    # Extract YAML block between --- delimiters
    m = re.match(r"^---\n(.*?)\n---", content, re.DOTALL)
    if not m:
        return rows
    fm = m.group(1)
    name_m = re.search(r"^name:\s*(.+)$", fm, re.MULTILINE)
    desc_m = re.search(r"^description:\s*(.+)$", fm, re.MULTILINE)
    unlocks_m = re.search(r"^unlocks:\s*\[(.+)\]", fm, re.MULTILINE)
    if not (name_m and desc_m):
        return rows
    name = name_m.group(1).strip()
    desc = desc_m.group(1).strip()
    unlocks = unlocks_m.group(1).strip() if unlocks_m else ""
    rows.append({
        "instruction": f"What does the b00t skill '{name}' do?",
        "input": "",
        "response": desc,
    })
    if unlocks:
        rows.append({
            "instruction": f"What MCP tools does b00t learn {name} unlock?",
            "input": "",
            "response": f"Loading '{name}' via `b00t learn {name}` unlocks: {unlocks}",
        })
    return rows


# ─── Layer 3: Library corpus via NeumannStore ─────────────────────────────────
# Queries b00t grok ask to pull indexed knowledge from the library NeumannStore.
# Falls back silently if b00t-cli is not installed or grok backend is offline.

_LIBRARY_QUERIES = [
    # Domain-general retrieval tasks that ground the model in real knowledge
    "What are the core principles of double-entry bookkeeping?",
    "What is the Australian R&D Tax Incentive and who is eligible?",
    "What are the main categories of R&D activities for tax purposes?",
    "How does BFO distinguish between continuants and occurrents?",
    "What is the difference between a SKOS ConceptScheme and a Collection?",
    "What are the key provisions of ITAA 1997 Division 355?",
    "How is income tax assessed for companies in Australia?",
    "What is the UFO distinction between endurants and perdurants?",
    "What are the core axioms of first-order predicate logic?",
    "How does the Dewey Decimal System classify computer science?",
    "What is the difference between deductive and inductive reasoning?",
    "What are the key principles of strategic planning in enterprise contexts?",
    "How does ISO 4217 standardize currency codes?",
    "What is OWL2 DL and how does it differ from OWL2 RL?",
    "What is the Satisfies<Constraint> pattern in UFO type systems?",
]

def query_library_corpus(queries: list[str], namespace: str = "library") -> list[dict[str, str]]:
    """Query NeumannStore via b00t grok ask, return instruction/response pairs."""
    rows: list[dict[str, str]] = []
    for query in queries:
        try:
            result = subprocess.run(
                ["b00t-cli", "grok", "ask", "--namespace", namespace, query],
                capture_output=True, text=True, timeout=15,
            )
            if result.returncode == 0 and result.stdout.strip():
                answer = result.stdout.strip()
                if len(answer) > 50:  # skip empty/error responses
                    rows.append({
                        "instruction": query,
                        "input": "",
                        "response": answer,
                    })
        except (subprocess.TimeoutExpired, FileNotFoundError):
            pass  # b00t-cli not available or grok backend offline — skip silently
    return rows


def generate_dataset(
    output_path: str,
    format: str = "alpaca",
    max_rows: int = 5000,
    b00t_root: Path | None = None,
    library_namespace: str = "library",
    skip_library: bool = False,
) -> int:
    """Generate the training dataset from all corpus sources."""
    root = b00t_root or B00T_ROOT
    sources = _sources(root)
    rows: list[dict[str, str]] = []

    # Layer 1: syntactic corpus
    for source_name, paths in sources.items():
        for path in paths:
            if not path.exists():
                continue
            try:
                content = path.read_text(encoding="utf-8", errors="replace")
            except Exception as e:
                print(f"  ⚠️  Error reading {path}: {e}", file=sys.stderr)
                continue

            before = len(rows)
            if source_name == "datums":
                rows.extend(parse_datum_instruction(path, content))
            elif source_name == "learn":
                rows.extend(parse_learn_instruction(path, content))
            elif source_name == "justfile":
                rows.extend(parse_justfile(content))
            elif source_name == "agents":
                rows.extend(parse_agent_instruction(path, content))
            elif source_name == "skills":
                rows.extend(parse_skill_frontmatter(path, content))
            after = len(rows)
            if after > before:
                print(f"  ✓ {path.name}: {after - before} rows", file=sys.stderr)

    # Layer 2: generative tasks (teach model to produce b00t syntax)
    layer2_before = len(rows)
    rows.extend(GENERATIVE_TASKS)
    print(f"  ✓ generative-tasks: {len(rows) - layer2_before} rows", file=sys.stderr)

    # Layer 3: library corpus (NeumannStore — skipped if b00t-cli unavailable)
    if not skip_library:
        print("\n[Layer 3] Querying library NeumannStore…", file=sys.stderr)
        layer3_rows = query_library_corpus(_LIBRARY_QUERIES, namespace=library_namespace)
        rows.extend(layer3_rows)
        if layer3_rows:
            print(f"  ✓ library-corpus ({library_namespace}): {len(layer3_rows)} rows", file=sys.stderr)
        else:
            print("  ⚠️  library-corpus: 0 rows (b00t-cli unavailable or namespace empty)", file=sys.stderr)

    print(f"\nTotal: {len(rows)} rows generated", file=sys.stderr)

    # Deduplicate
    seen: set[tuple[str, str]] = set()
    deduped: list[dict[str, str]] = []
    for row in rows:
        key = (row["instruction"], row.get("response", ""))
        if key not in seen:
            seen.add(key)
            deduped.append(row)

    print(f"After dedup: {len(deduped)} rows", file=sys.stderr)

    # Truncate to max_rows
    if len(deduped) > max_rows:
        deduped = deduped[:max_rows]
        print(f"Truncated to {max_rows} rows", file=sys.stderr)

    # Write output
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    with open(output_path, "w") as f:
        for row in deduped:
            if format == "chatml":
                instruction = row["instruction"]
                input_text = row.get("input", "")
                response = row.get("response", "")
                user_content = f"{instruction}\n\n{input_text}" if input_text else instruction
                f.write(json.dumps({
                    "messages": [
                        {"role": "user", "content": user_content},
                        {"role": "assistant", "content": response},
                    ]
                }) + "\n")
            else:
                f.write(json.dumps(row) + "\n")

    return len(deduped)


def main():
    parser = argparse.ArgumentParser(description="Generate b00t fine-tuning dataset")
    parser.add_argument("--output", default="fine-tune/train.jsonl", help="Output path")
    parser.add_argument("--format", choices=["alpaca", "chatml"], default="alpaca", help="Output format")
    parser.add_argument("--max-rows", type=int, default=5000, help="Max training rows")
    parser.add_argument("--b00t-root", type=Path, default=None, help="b00t repo root (auto-detected if omitted)")
    parser.add_argument("--library-namespace", default="library", help="NeumannStore namespace for library corpus")
    parser.add_argument("--skip-library", action="store_true", help="Skip Layer 3 NeumannStore queries")
    args = parser.parse_args()

    count = generate_dataset(
        args.output,
        args.format,
        args.max_rows,
        b00t_root=args.b00t_root,
        library_namespace=args.library_namespace,
        skip_library=args.skip_library,
    )
    print(f"\n✅ Generated {count} training rows → {args.output}")


if __name__ == "__main__":
    main()
