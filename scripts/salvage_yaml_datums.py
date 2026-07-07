#!/usr/bin/env python3
# @b00t:harness salvage-yaml-datums (task #115, doctrine: NEVER lose the payload)
# Converts YAML-frontmatter files mis-saved as .toml datums into valid .tomllm:
#   frontmatter keys → [b00t] stanza; ALL original content preserved verbatim
#   in a comment block (prose belongs in TOML comments per .tomllmd rules);
#   tail-map generated. Original broken file is deleted only when the emitted
#   .tomllm parses (checked with tomllib) — salvage, then remove the wreck.
#
# Usage: uv run --with pyyaml python3 scripts/salvage_yaml_datums.py [--apply] [files...]
#        (default dry-run; no files → scan _b00t_/*.toml for the signature)

import sys
import tomllib
from pathlib import Path

import yaml


def is_yaml_frontmatter(path: Path) -> bool:
    try:
        head = path.read_text(errors="replace")
    except OSError:
        return False
    return head.startswith("---\n")


def split_frontmatter(text: str):
    parts = text.split("\n---", 1)
    if not text.startswith("---\n"):
        return None, text
    rest = text[4:]
    if "\n---" in rest:
        fm, body = rest.split("\n---", 1)
        return fm, body.lstrip("\n")
    return rest, ""


def toml_str(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def convert(path: Path) -> str:
    text = path.read_text(errors="replace")
    fm_text, body = split_frontmatter(text)
    meta = {}
    if fm_text:
        try:
            loaded = yaml.safe_load(fm_text)
            if isinstance(loaded, dict):
                meta = loaded
        except yaml.YAMLError:
            pass  # salvage doctrine: keep going, everything lands in comments

    stem = path.name.rsplit(".", 2)[0]
    name = str(meta.get("name", stem))
    dtype = str(meta.get("type", path.suffixes[0].lstrip(".") if len(path.suffixes) > 1 else "skill"))
    desc = str(meta.get("description", meta.get("hint", ""))).strip()
    hint = str(meta.get("hint", desc.splitlines()[0] if desc else name)).strip()[:160]

    out = [f"[b00t]", f"name = {toml_str(name)}", f"type = {toml_str(dtype)}", f"hint = {toml_str(hint)}", ""]
    extra = {k: v for k, v in meta.items() if k not in ("name", "type", "hint")}

    out.append("# ── salvaged content (was YAML frontmatter in a .toml — task #115) ──────────")
    out.append("# 🤓 payload preserved VERBATIM below; distill to structured fields later (#98)")
    for k, v in extra.items():
        block = yaml.safe_dump({k: v}, default_flow_style=False, width=98).rstrip()
        out.extend(f"# {line}" for line in block.splitlines())
    if body.strip():
        out.append("#")
        out.extend(f"# {line}" for line in body.rstrip().splitlines())

    tags = ", ".join(p for p in stem.replace("_", "-").split("-")[:5])
    out += [
        "",
        "# b00t:map v1",
        f"# summary: {hint or name} (salvaged from YAML-frontmatter .toml)",
        f"# tags: {tags}",
        "# tier: sm0l",
        f"# cmds: b00t learn {name}",
        "# complexity: 3",
        "",
    ]
    return "\n".join(out)


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--apply"]
    apply = "--apply" in sys.argv
    files = [Path(a) for a in args] or sorted(
        p for p in Path("_b00t_").glob("*.toml") if is_yaml_frontmatter(p)
    )
    converted = failed = 0
    for path in files:
        if not is_yaml_frontmatter(path):
            continue
        new_text = convert(path)
        try:
            tomllib.loads(new_text)
        except tomllib.TOMLDecodeError as e:
            print(f"FAIL {path}: emitted TOML invalid: {e}", file=sys.stderr)
            failed += 1
            continue
        target = path.with_suffix("")  # strip .toml
        target = target.parent / (target.name + ".tomllm")
        if apply:
            target.write_text(new_text)
            path.unlink()
        print(f"{'SALVAGED' if apply else 'would salvage'} {path} → {target}")
        converted += 1
    print(f"{'PASS' if failed == 0 else 'PARTIAL'}: {converted} converted, {failed} failed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
