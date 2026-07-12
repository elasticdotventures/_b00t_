#!/usr/bin/env python3.14
"""Generate b00t-aligned training dataset from datums, learn files, and justfile recipes.

Output: JSONL in Alpaca/chatml format for unsloth QLoRA fine-tuning.
Target: 5000+ rows covering b00t idioms, k0mmand3r syntax, datum patterns, hive ops.
"""

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Any

# ─── Corpus sources ────────────────────────────────────────────────────────────

DOTFILES = Path.home() / ".dotfiles"

SOURCES: dict[str, list[Path]] = {
    "datums": sorted((DOTFILES / "_b00t_/datums").glob("*.tomllmd")),
    "learn": sorted((DOTFILES / "_b00t_/learn").glob("*.md")),
    "hive_profiles": sorted((DOTFILES / "_b00t_").glob("*.hive.toml")),
    "justfile": [DOTFILES / "justfile"],
    "agents": sorted((DOTFILES / "AGENTS").glob("*.md")),
}

# ─── Template patterns ─────────────────────────────────────────────────────────

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


def generate_dataset(output_path: str, format: str = "alpaca", max_rows: int = 5000) -> int:
    """Generate the training dataset from all corpus sources."""
    rows: list[dict[str, str]] = []

    for source_name, paths in SOURCES.items():
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
            after = len(rows)
            if after > before:
                print(f"  ✓ {path.name}: {after - before} rows", file=sys.stderr)

    print(f"\nTotal: {len(rows)} rows generated", file=sys.stderr)

    # Deduplicate
    seen = set()
    deduped: list[dict[str, str]] = []
    for row in rows:
        key = (row["instruction"], row.get("response", ""))
        if key not in seen:
            seen.add(key)
            deduped.append(row)

    print(f"After dedup: {len(deduped)} rows", file=sys.stderr)

    # Truncate or pad to max_rows
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
    args = parser.parse_args()

    count = generate_dataset(args.output, args.format, args.max_rows)
    print(f"\n✅ Generated {count} training rows → {args.output}")


if __name__ == "__main__":
    main()
