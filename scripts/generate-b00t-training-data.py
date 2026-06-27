#!/usr/bin/env python3
"""b00t training data generator — extracts instruction pairs from source code.

Produces a JSONL file with instruction/response pairs suitable for fine-tuning.
Sources: AGENTS.md, Rust doc comments, datum .tomllm files, skills, justfile.
Output: ~/.b00t/training/b00t-corpus.jsonl
"""

import json, os, re, sys, glob
from pathlib import Path

B00T_ROOT = Path(os.environ.get("B00T_ROOT", os.path.expanduser("~/.b00t")))
OUTPUT = B00T_ROOT / "training" / "b00t-corpus.jsonl"

def extract_doc_comments(text: str, filepath: str) -> list[dict]:
    """Extract Rust /// and //! doc comments, pairing with surrounding context."""
    pairs = []
    lines = text.split("\n")
    doc_lines = []
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("/// ") or stripped.startswith("//! "):
            doc_lines.append(stripped[4:].strip())
        elif stripped.startswith("///") or stripped.startswith("//!"):
            doc_lines.append(stripped[3:].strip())
        elif doc_lines:
            doc = " ".join(doc_lines)
            if len(doc) > 20:
                fn_name = ""
                for j in range(i, min(i + 5, len(lines))):
                    m = re.match(r'pub\s+(?:async\s+)?fn\s+(\w+)', lines[j])
                    if m:
                        fn_name = m.group(1)
                        break
                context = fn_name or filepath.split("/")[-1].replace(".rs", "")
                pairs.append({
                    "instruction": f"Describe the '{context}' function in {filepath.split('/')[-1]}",
                    "response": doc,
                    "source": filepath,
                })
            doc_lines = []
    return pairs


def extract_markdown_sections(text: str, filepath: str) -> list[dict]:
    """Extract ## sections from markdown files as instruction pairs."""
    pairs = []
    sections = re.split(r'\n## ', text)
    for section in sections[1:]:
        lines = section.strip().split("\n")
        title = lines[0].strip()
        body = "\n".join(lines[1:]).strip()
        if len(body) > 30:
            pairs.append({
                "instruction": f"What does the b00t documentation say about '{title}'?",
                "response": body[:500],
                "source": filepath,
            })
    return pairs


def extract_toml_datums(text: str, filepath: str) -> list[dict]:
    """Extract b00t datums — name + hint + learn content."""
    pairs = []
    name_m = re.search(r'name\s*=\s*"([^"]+)"', text)
    hint_m = re.search(r'hint\s*=\s*"([^"]+)"', text)
    learn_m = re.search(r'content\s*=\s*"""\s*(.+?)\s*"""', text, re.DOTALL)
    if name_m and hint_m:
        response = hint_m.group(1)
        if learn_m:
            response += "\n\n" + learn_m.group(1)[:300]
        pairs.append({
            "instruction": f"What is the b00t datum '{name_m.group(1)}'?",
            "response": response,
            "source": filepath,
        })
    return pairs


def main():
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    all_pairs = []

    # AGENTS.md
    agents_path = B00T_ROOT / "AGENTS.md"
    if agents_path.exists():
        text = agents_path.read_text()
        all_pairs.extend(extract_markdown_sections(text, "AGENTS.md"))
        all_pairs.append({
            "instruction": "What is the b00t hive agent operating protocol?",
            "response": text[:2000],
            "source": "AGENTS.md",
        })

    # justfile
    jf = B00T_ROOT / "justfile"
    if jf.exists():
        text = jf.read_text()
        all_pairs.append({
            "instruction": "What recipes are available in the b00t justfile?",
            "response": "\n".join([l for l in text.split("\n") if l.strip() and not l.strip().startswith("#")])[:1500],
            "source": "justfile",
        })

    # Rust doc comments
    for rs_file in B00T_ROOT.glob("b00t-*/src/**/*.rs"):
        text = rs_file.read_text()
        all_pairs.extend(extract_doc_comments(text, str(rs_file.relative_to(B00T_ROOT))))

    # Datum files
    for toml_file in list(B00T_ROOT.glob("_b00t_/**/*.toml"))[:50]:
        text = toml_file.read_text()
        all_pairs.extend(extract_toml_datums(text, str(toml_file.relative_to(B00T_ROOT))))

    # Skills
    for skill in B00T_ROOT.glob(".opencode/skills/*/SKILL.md"):
        text = skill.read_text()
        all_pairs.extend(extract_markdown_sections(text, str(skill.relative_to(B00T_ROOT))))

    # Format as ChatML
    with open(OUTPUT, "w") as f:
        for pair in all_pairs:
            record = {
                "messages": [
                    {"role": "user", "content": pair["instruction"]},
                    {"role": "assistant", "content": pair["response"]},
                ],
                "source": pair["source"],
            }
            f.write(json.dumps(record) + "\n")

    print(f"✅ {len(all_pairs)} training pairs written to {OUTPUT}")


if __name__ == "__main__":
    main()
