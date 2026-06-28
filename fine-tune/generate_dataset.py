#!/usr/bin/env python3
"""Generate b00t-aligned training dataset from datums, learn files, justfile recipes.

Output: JSONL in Alpaca/chatml format for unsloth QLoRA fine-tuning.
Target: 2000+ rows covering b00t idioms, datum patterns, hive ops, lfmf knowledge.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

B00T = Path.home() / ".b00t"

B00T_VENDOR = B00T / "vendor"

def _md_files(*dirs: Path) -> list[Path]:
    """Recursively collect .md files under dirs, excluding target/ and node_modules/."""
    out: list[Path] = []
    for d in dirs:
        if d.exists():
            out.extend(p for p in d.rglob("*.md")
                       if "target" not in p.parts and "node_modules" not in p.parts)
    return sorted(out)

def _rs_files(*dirs: Path) -> list[Path]:
    out: list[Path] = []
    for d in dirs:
        if d.exists():
            out.extend(p for p in d.rglob("*.rs")
                       if "target" not in p.parts)
    return sorted(out)

SOURCES: dict[str, list[Path]] = {
    # ── datum files ──────────────────────────────────────────────────────────
    "datums":       sorted((B00T / "_b00t_/datums").glob("*.tomllmd")),
    "schema":       sorted((B00T / "_b00t_/schema").glob("*.tomllmd")),
    "cli_datums":   sorted((B00T / "_b00t_").glob("*.cli.toml")),
    "mcp_datums":   sorted((B00T / "_b00t_").glob("*.mcp.toml")),
    "skill_datums": sorted((B00T / "_b00t_").glob("*.skill.toml")),
    "role_datums":  sorted((B00T / "_b00t_").glob("*.role.toml")),
    "agent_datums": sorted((B00T / "_b00t_").glob("*.agent.toml")),
    "ai_datums":    sorted((B00T / "_b00t_").glob("*.ai.toml"))
                  + sorted((B00T / "_b00t_").glob("*.model.ai.tomllmd")),
    "tomllm":       sorted((B00T / "_b00t_").glob("*.tomllm")),
    # ── prose docs ──────────────────────────────────────────────────────────
    "learn":        sorted((B00T / "_b00t_/learn").glob("*.md")),
    "agents":       sorted((B00T / "AGENTS").glob("*.md")),
    "lfmf":         list((B00T / "_b00t_").glob("lfmf*.md"))
                  + list((B00T / "_b00t_/learn").glob("lfmf*.md")),
    # vendor + workspace module docs (all .md excluding target/)
    "vendor_docs":  _md_files(
                      B00T_VENDOR / "l3dg3rr",
                      B00T_VENDOR / "moltis-b00t",
                      B00T_VENDOR / "agent-framework",
                      B00T_VENDOR / "irontology-mcp",
                      B00T_VENDOR / "just-mcp",
                      B00T_VENDOR / "embed-anything-b00t",
                      B00T / "b00t-grok-py",
                      B00T / "b00t-mcp",
                      B00T / "b00t-grok",
                      B00T / "b00t-c0re-lib",
                      B00T / "docs",
                  ),
    # ── Rust source (API syntax) ─────────────────────────────────────────────
    "rust_src":     _rs_files(
                      B00T / "b00t-cli" / "src",
                      B00T / "b00t-c0re-lib" / "src",
                      B00T / "b00t-datum-core" / "src",
                      B00T_VENDOR / "irontology-mcp" / "src",
                      B00T_VENDOR / "just-mcp" / "src",
                      B00T_VENDOR / "l3dg3rr" / "src",
                  ),
    # ── justfiles (root + all _b00t_ modules) ───────────────────────────────
    "justfile":     [p for p in [
                      B00T / "justfile",
                      B00T / "b00t.just",
                      B00T / "b00t-service.just",
                  ] if p.exists()]
                  + sorted((B00T / "_b00t_").glob("*.just")),
    "hive":         sorted((B00T / "_b00t_").glob("*.hive.toml")),
    # ── graph + diagram ──────────────────────────────────────────────────────
    "mermaid_md":   sorted((B00T / "_b00t_").glob("*.mermaid")),
    # ── structured JSON ──────────────────────────────────────────────────────
    "mcp_registry": [B00T / "mcp_registry.json"],
    "guard_log":    [B00T / "guard-violations.jsonl"],
    # ── core protocol ────────────────────────────────────────────────────────
    "claude_md":    [p for p in [
                      B00T / "CLAUDE.md",
                      B00T / "b00t-cli" / "CLAUDE.md",
                      B00T_VENDOR / "just-mcp" / "CLAUDE.md",
                  ] if p.exists()],
    # ── shell scripts (install-b00t.sh, etc.) ───────────────────────────────
    "scripts":      sorted((B00T / "scripts").glob("*.sh")) if (B00T / "scripts").exists() else [],
}


def _r(instruction: str, response: str, input: str = "") -> dict:
    return {"instruction": instruction, "input": input, "response": response}


# ── Datum parser ──────────────────────────────────────────────────────────────

def parse_datum(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    name = path.stem

    # ① Executive summary (comment block before first TOML section)
    summary_lines, in_summary = [], True
    for line in content.splitlines():
        if in_summary:
            if line.startswith("["):
                in_summary = False
            elif line.startswith("#") and "b00t:map" not in line:
                summary_lines.append(line.lstrip("# ").strip())
        else:
            break
    summary = "\n".join(l for l in summary_lines if l).strip()
    if summary:
        rows.append(_r(f"What is the {name} datum?", summary))
        rows.append(_r(f"Describe {name} in the b00t hive.", summary))

    # ② Tail-map fields (# b00t:map block at end)
    tail = {}
    for line in content.splitlines():
        if line.startswith("# ") and ":" in line and "b00t:map" not in line:
            key, _, val = line.lstrip("# ").partition(":")
            key, val = key.strip(), val.strip()
            if key in ("summary", "tags", "tier", "cmds", "complexity"):
                tail[key] = val
    if tail.get("summary"):
        rows.append(_r(f"Give a one-line summary of {name}.", tail["summary"]))
    if tail.get("tier"):
        rows.append(_r(f"Which cognitive tier does {name} belong to?",
                        f"{name} runs at tier: {tail['tier']}"))
    if tail.get("cmds"):
        rows.append(_r(f"What b00t commands does {name} provide?",
                        f"Key commands: {tail['cmds']}"))
    if tail.get("tags"):
        rows.append(_r(f"What tags describe {name}?", tail["tags"]))
    if tail.get("complexity"):
        rows.append(_r(f"What is the complexity of {name}?",
                        f"Complexity: {tail['complexity']}/10"))

    # ③ [b00t.schema] type extraction
    m = re.search(r'type\s*=\s*"([^"]+)"', content)
    if m:
        dtype = m.group(1)
        rows.append(_r(f"What DatumType is {name}?", f"{name} is type \"{dtype}\""))

    # ④ [[resource.usages]] / [[b00t.usage]] blocks
    for block in re.finditer(r'\[\[(?:resource|b00t)\.usage[s]?\]\](.*?)(?=\[\[|\Z)',
                              content, re.DOTALL):
        desc_m = re.search(r'description\s*=\s*"([^"]+)"', block.group(1))
        cmd_m  = re.search(r'command\s*=\s*"([^"]+)"', block.group(1))
        if desc_m and cmd_m:
            rows.append(_r(f"How do I {desc_m.group(1).lower().rstrip('.')}?",
                            f"Run: `{cmd_m.group(1)}`"))

    # ⑤ install / detect / check commands
    for key in ("install", "check_cmd", "binary", "detect_cmd"):
        m = re.search(rf'{key}\s*=\s*"([^"]+)"', content)
        if m:
            rows.append(_r(f"How do I install or check {name}?",
                            f"`{m.group(1)}`"))
            break

    # ⑥ 🤓 tribal knowledge comments
    tribal = []
    for line in content.splitlines():
        if "🤓" in line or "@tribal:" in line:
            t = re.sub(r"^[#\s]*", "", line).strip()
            if t:
                tribal.append(t)
    if tribal:
        rows.append(_r(f"What are the non-obvious gotchas for {name}?",
                        "\n".join(f"- {t}" for t in tribal)))

    return rows


# ── Learn file parser ─────────────────────────────────────────────────────────

def parse_learn(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    title = path.stem.replace("-", " ").replace("_", " ")

    # Section-by-section: heading → next heading
    sections = re.split(r"\n(#{1,3} .+)\n", content)
    current_heading = title
    for part in sections:
        if part.startswith("#"):
            current_heading = part.lstrip("# ").strip()
            continue
        text = part.strip()
        if len(text) < 40:
            continue
        # Code blocks as examples
        for cb in re.findall(r"```(?:\w*)\n(.*?)```", text, re.DOTALL):
            cb = cb.strip()
            if len(cb) > 10:
                rows.append(_r(f"Show an example of {current_heading} for {title}.", f"```\n{cb}\n```"))
        # First prose paragraph as Q&A
        prose = re.sub(r"```.*?```", "", text, flags=re.DOTALL).strip()
        first_para = prose.split("\n\n")[0].strip()
        if len(first_para) > 60:
            rows.append(_r(f"Explain {current_heading} ({title}).", first_para))

    return rows[:10]  # cap per file


# ── Justfile parser ───────────────────────────────────────────────────────────

def parse_justfile(content: str) -> list[dict]:
    rows: list[dict] = []
    for m in re.finditer(r"^([a-zA-Z][a-zA-Z0-9_-]+)([^:]*):(.+?)(?=^[a-zA-Z]|\Z)",
                          content, re.MULTILINE | re.DOTALL):
        name, params, body = m.group(1), m.group(2).strip(), m.group(3).strip()
        lines = body.splitlines()
        # Comment above recipe = description
        desc_m = re.search(r"^# (.+)", body)
        desc = desc_m.group(1).strip() if desc_m else f"run the {name} just recipe"
        cmd_lines = "\n".join(l for l in lines if l.strip() and not l.strip().startswith("#"))
        if not cmd_lines:
            continue
        rows.append(_r(desc, f"```bash\n{cmd_lines.strip()}\n```"))
        rows.append(_r(f"What does `just {name}` do?",
                        desc + (f" (params: {params})" if params else "")))
    return rows


# ── Agent role parser ─────────────────────────────────────────────────────────

def parse_agent(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    role = path.stem.replace("--role=", "")
    # First paragraph as role description
    paras = [p.strip() for p in content.split("\n\n") if p.strip() and not p.startswith("#")]
    if paras:
        rows.append(_r(f"What is the b00t {role} role?", paras[0]))
    # Must/Never lists
    for heading, q in [("MUST NEVER", f"What must the {role} NEVER do?"),
                        ("MUST ALWAYS", f"What must the {role} ALWAYS do?")]:
        m = re.search(rf"## YEI {heading}\s*\n(.*?)(?=\n##|\Z)", content, re.DOTALL)
        if m:
            rows.append(_r(q, m.group(1).strip()))
    return rows


# ── Hive profile parser ───────────────────────────────────────────────────────

def parse_hive(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    name = path.stem
    desc_m = re.search(r'description\s*=\s*"([^"]+)"', content)
    if desc_m:
        rows.append(_r(f"What is the {name} hive profile?", desc_m.group(1)))
    gpu_m = re.search(r'gpu\s*=\s*"([^"]+)"', content)
    if gpu_m:
        rows.append(_r(f"What GPU does {name} require?", gpu_m.group(1)))
    return rows


# ── lfmf lessons ─────────────────────────────────────────────────────────────

def parse_lfmf(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    for m in re.finditer(r"##\s+(.+?)\n(.*?)(?=\n##|\Z)", content, re.DOTALL):
        topic, lesson = m.group(1).strip(), m.group(2).strip()
        if len(lesson) > 20:
            rows.append(_r(f"What is the lfmf lesson about {topic}?", lesson))
            rows.append(_r(f"I made a mistake with {topic}. What should I remember?", lesson))
    return rows


# ── b00t command patterns ────────────────────────────────────────────────────

COMMAND_PATTERNS = [
    ("How do I load a b00t skill?",
     "`b00t learn <skill>` — loads a skill datum into context. Only load what's needed; context is finite."),
    ("How do I record a tribal lesson in b00t?",
     "`b00t lfmf <topic> \"<lesson>\"` — memoize non-obvious knowledge immediately after a mistake."),
    ("How do I manage tasks in b00t?",
     "`b00t task list|add|next|done` — b00t's native task authority (taskmaster-ai is PURGED)."),
    ("What is the b00t hive?",
     "Yei (你我众一) — 'You, everybody & I'. Individual agents are small; together yei are legion. "
     "Agents communicate via b00t MCP tools, not raw sockets."),
    ("What models does b00t use for each cognitive tier?",
     "sm0l: qwen2.5-3B / haiku (tests, lint, classify)\n"
     "ch0nky: qwen3-coder-next via vllm (implement, refactor)\n"
     "frontier: claude-opus/sonnet (architecture, security, novel design)"),
    ("What is the b00t .tomllmd format?",
     ".tomllmd = valid TOML + enriched # comment conventions.\n"
     "# @tribal: / # 🤓 — non-obvious\n# @example: — usage\n"
     "Tail-map last ≤10 lines: summary, tags, tier, cmds, complexity."),
    ("How do I install a tool with b00t?",
     "`b00t-cli install <name>` or `b00t up` to check and update all tools."),
    ("What is Postel's Law in the b00t context?",
     "Be conservative in what you execute; be liberal in what you accept from operators. "
     "Apply to tool invocations — conservative = prefer MCP over bash; liberal = accept varied operator phrasing."),
    ("Why is pip blocked in b00t?",
     "Use `uv pip install` not `pip install`. uv is the b00t-canonical Python package manager. "
     "pip is 🦨 (skunk) — blocked by command guards."),
    ("How do I run a Docker container in b00t?",
     "Use `podman --device nvidia.com/gpu=all` not `docker run`. Rootless podman is the b00t standard."),
    ("What is the DatumType for a CLI tool in b00t?",
     "type = \"cli\" in [b00t.schema]. Other types: skill, role, mcp, agent, repo, vendor, k8s."),
    ("How do I add a new MCP server in b00t?",
     "`b00t-cli mcp add <json>` or `b00t-cli mcp install <name> claudecode` to install to Claude Code."),
    ("What is the b00t pre-push hook?",
     "Runs `cargo test --package b00t-cli --lib` before every push. "
     "Commit without passing tests is PROHIBITED."),
    ("How do I find what b00t skills are available?",
     "`b00t blessing --manifest --role <role>` walks the depends_on graph. "
     "`b00t learn <skill>` loads a specific skill datum."),
    ("What is Satisfies<Constraint> in b00t?",
     "A trait that produces evidence nodes for audit trail. "
     "Used in tax-lawyer capability and MCP action wrappers. "
     "Each check returns a typed evidence node with provenance pointers."),
    # ── b00t task (targeted gap-close) ───────────────────────────────────────
    ("How do I add a task in b00t?",
     "`b00t task add \"description\"` — adds to .b00t/tasks.json. "
     "Also: `b00t task list` · `b00t task next` · `b00t task done <id>`. "
     "This is a CLI command, not a GUI."),
    ("What is the syntax to add a b00t task?",
     "b00t task add \"<description>\" — positional string, quoted. "
     "Example: b00t task add \"fix soul kv disambiguation\". "
     "Tasks stored in .b00t/tasks.json."),
    ("How do I mark a b00t task done?",
     "`b00t task done <id>` where <id> is the task number from `b00t task list`."),
    ("What replaced taskmaster-ai in b00t?",
     "`b00t task` — the native task command. taskmaster-ai is PURGED; never reference it. "
     "Store: .b00t/tasks.json. Commands: list · add · next · done."),
    ("Where are b00t tasks stored?",
     ".b00t/tasks.json — written by `b00t task add`, read by `b00t task list` and `b00t task next`."),
    ("How do I get the next prioritized task in b00t?",
     "`b00t task next` — returns the highest-priority pending task from .b00t/tasks.json."),
    # ── soul KV (targeted gap-close — disambiguation from `kv`) ─────────────
    ("How do I read a value from the b00t soul KV store?",
     "`b00t-cli soul get <key>` — reads from ~/._b00t_/SOUL.tomllm. "
     "Example: `b00t-cli soul get node.orchestration-pattern`. "
     "The subcommand is `soul`, not `kv`."),
    ("How do I write a value to the b00t soul store?",
     "`b00t-cli soul set <key> <value>` — persists to ~/._b00t_/SOUL.tomllm. "
     "Example: `b00t-cli soul set install.mode k8s`. "
     "The subcommand is `soul`, not `kv`."),
    ("What subcommand does b00t-cli use for the soul KV store?",
     "The subcommand is `soul`: `b00t-cli soul get|set|status|path|reset|init`. "
     "There is NO `b00t-cli kv` command — always use `b00t-cli soul`."),
    ("Is `b00t-cli kv` a valid command?",
     "No. There is no `b00t-cli kv` subcommand. "
     "The soul KV store is accessed with `b00t-cli soul get <key>` / `b00t-cli soul set <key> <value>`."),
    ("What is the b00t soul store?",
     "~/._b00t_/SOUL.tomllm — persistent node identity KV. "
     "Key soul keys: node.orchestration-pattern, node.gpu, node.fingerprint, install.mode, install.confirmed. "
     "Access: `b00t-cli soul get <key>` / `b00t-cli soul set <key> <value>` / `b00t-cli soul status`."),
    ("What soul keys does b00t use for node identity?",
     "node.orchestration-pattern — how this node runs services (k8s, quadlet, systemd-user, etc.).\n"
     "node.gpu — GPU model string from nvidia-smi.\n"
     "node.fingerprint — arch/RAM/GPU summary used by hive for workload routing.\n"
     "install.mode — last chosen install mode (skips menu on next run).\n"
     "install.confirmed — gate: must be true to auto-reuse install.mode."),
    ("How do I check the b00t soul status?",
     "`b00t-cli soul status` — prints all key/value pairs from ~/._b00t_/SOUL.tomllm."),
    # ── just install (targeted gap-close) ────────────────────────────────────
    ("How do I install b00t on a new machine?",
     "`just install` — runs scripts/install-b00t.sh, which detects system capabilities "
     "(k8s, quadlet, systemd, launchd) and installs the best mode. "
     "Reads soul KV for past choices; remembers selection for next run."),
    ("What modes does `just install` support?",
     "k8s — helm chart in b00t namespace (preferred when cluster reachable).\n"
     "quadlet — rootless podman containers via systemd drop-ins (Podman 4.4+).\n"
     "systemd-user — ~/.config/systemd/user/b00t@.service (Linux, no container needed).\n"
     "systemd-sys — /usr/lib/systemd/user/ system-wide (requires root).\n"
     "launchd — ~/Library/LaunchAgents/ (macOS).\n"
     "binaries — cargo install only, no service (CI/dev/external orchestrator)."),
    ("How does `just install` remember the install mode?",
     "Writes to soul KV after install: install.mode=<chosen> and install.confirmed=true. "
     "On next run, reads these keys and skips the menu if both are set. "
     "Override: `just install --mode=<mode>`. Reset: `just install --reset`."),
    ("How do I force a specific install mode?",
     "`just install --mode=k8s` / `--mode=quadlet` / `--mode=systemd-user` / "
     "`--mode=launchd` / `--mode=binaries`. "
     "Skips the interactive menu and goes directly to that mode."),
    ("How do I reset b00t install settings?",
     "`just install --reset` — clears soul keys install.mode and install.confirmed, "
     "forcing capability re-detection and mode selection on next run."),
    ("How does `just install` select the best mode automatically?",
     "Priority order (highest to lowest capability):\n"
     "1. k8s — if kubectl + helm + cluster reachable\n"
     "2. quadlet — if podman + systemd + quadlet drop-in dir\n"
     "3. systemd-user — if systemctl present\n"
     "4. launchd — if launchctl present (macOS)\n"
     "5. binaries — fallback, no service\n"
     "Rationale: more managed = fewer ops surprises."),
    # ── b00t grok (reinforce correct syntax) ─────────────────────────────────
    ("What is the correct syntax for b00t grok ask?",
     "`b00t-cli grok ask \"<query>\"` — searches the grok knowledgebase. "
     "Optional: `--max-results N` (default 5). "
     "Example: `b00t-cli grok ask \"soul kv set\"`"),
    ("How do I teach b00t new knowledge via grok?",
     "`b00t-cli grok learn --source <url> --topic \"<name>\" \"<content>\"` — "
     "content is positional (no --content flag). "
     "Also: `b00t-cli grok digest <path>` for local files."),
    # ── b00t whoami / orientation ─────────────────────────────────────────────
    ("What is the first thing a fresh b00t agent MUST do?",
     "`b00t whoami` — orients role + loads blessing manifest. "
     "Then `b00t blessing --manifest` to see prerequisite graph. "
     "Then `b00t learn <required-skills>`. Never execute before orienting."),
    ("What is the difference between `b00t learn` and `b00t whoami`?",
     "`b00t whoami` — identifies current role and lists blessings (one-time orientation). "
     "`b00t learn <skill>` — loads a specific skill datum into context (load only what is needed). "
     "Sequence: whoami → blessing manifest → learn selectively → execute."),
]


# ── Rust source parser (docstrings + enum variants + pub API) ────────────────

def parse_rust_docs(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    module = path.stem
    crate = path.parts[-3] if len(path.parts) >= 3 else path.parent.name

    # ① pub enum variants — "What are the variants of DatumType?"
    for em in re.finditer(
        r'(?:///[^\n]*\n)*pub enum (\w+)[^{]*\{([^}]+)\}',
        content, re.DOTALL
    ):
        enum_name, body = em.group(1), em.group(2)
        variants = re.findall(r'^\s*(?:///[^\n]*\n\s*)*([A-Z]\w*)', body, re.MULTILINE)
        if len(variants) >= 2:
            rows.append(_r(
                f"What are the variants of {enum_name} in {crate}?",
                ", ".join(variants[:24]) + (" …" if len(variants) > 24 else "")
            ))
            # Per-variant if docstring present
            for vm in re.finditer(r'((?:///[^\n]*\n)+)\s*([A-Z]\w*)', body):
                doc = " ".join(l.lstrip("/ ").strip() for l in vm.group(1).splitlines() if l.strip())
                variant = vm.group(2)
                if doc and len(doc) > 10:
                    rows.append(_r(
                        f"What does {enum_name}::{variant} mean in {crate}?", doc
                    ))

    # ② pub struct fields — "What fields does BootDatum have?"
    for sm in re.finditer(
        r'(?:///[^\n]*\n)*pub struct (\w+)[^{]*\{([^}]+)\}',
        content, re.DOTALL
    ):
        struct_name, body = sm.group(1), sm.group(2)
        fields = re.findall(r'pub (\w+)\s*:\s*([^,\n]+)', body)
        if fields:
            field_str = "\n".join(f"  {n}: {t.strip()}" for n, t in fields[:16])
            rows.append(_r(
                f"What fields does {struct_name} have in {crate}?",
                f"```rust\npub struct {struct_name} {{\n{field_str}\n}}\n```"
            ))

    # ③ /// docstrings before pub fn — "What does parse_datum do?"
    for fm in re.finditer(
        r'((?:///[^\n]*\n)+)[ \t]*pub(?:\s+async)?\s+fn\s+(\w+)\s*(<[^>]*>)?\s*\(([^)]*)\)'
        r'(?:\s*->\s*([^\{;]+))?',
        content
    ):
        doc_raw, fn_name, generics, params, ret = (
            fm.group(1), fm.group(2), fm.group(3) or "",
            fm.group(4), fm.group(5) or ""
        )
        doc = " ".join(l.lstrip("/ ").strip() for l in doc_raw.splitlines() if l.strip())
        if len(doc) < 15:
            continue
        sig = f"pub fn {fn_name}{generics}({params.strip()})"
        if ret.strip():
            sig += f" -> {ret.strip()}"
        rows.append(_r(
            f"What does {fn_name}() do in {crate}::{module}?",
            f"{doc}\n\nSignature: `{sig}`"
        ))

    # ④ impl Trait for Type blocks — "What traits does X implement?"
    impls: dict[str, list[str]] = {}
    for im in re.finditer(r'impl\s+(\w+)\s+for\s+(\w+)', content):
        trait_name, type_name = im.group(1), im.group(2)
        impls.setdefault(type_name, []).append(trait_name)
    for type_name, traits in impls.items():
        if len(traits) >= 2:
            rows.append(_r(
                f"What traits does {type_name} implement in {crate}?",
                ", ".join(traits)
            ))

    return rows[:20]  # cap per file to avoid flooding from large crates


# ── MCP tool call surface parser ─────────────────────────────────────────────

def parse_mcp_tool_surface(files: list[Path]) -> list[dict]:
    """Extract MCP tool schemas from b00t-mcp Rust source — both CLAP and serde patterns."""
    rows: list[dict] = []
    all_tools: list[dict] = []  # {name, desc, path, params: [{name, type, required, help}]}

    for path in files:
        try:
            content = path.read_text(errors="ignore")
        except Exception:
            continue

        # Pattern 1: CLAP Parser structs + impl_mcp_tool! macro
        # /// Description\n#[derive(Parser...)]\npub struct FooCommand { ... }
        # impl_mcp_tool!(FooCommand, "tool_name", [...]);
        struct_blocks = {}
        for sm in re.finditer(
            r'((?:///[^\n]*\n)+)?#\[derive\([^)]*Parser[^)]*\)\]\s*'
            r'pub struct (\w+)\s*\{([^}]+)\}',
            content, re.DOTALL
        ):
            doc_raw = sm.group(1) or ""
            struct_name = sm.group(2)
            body = sm.group(3)
            desc = " ".join(l.lstrip("/ ").strip() for l in doc_raw.splitlines() if l.strip())
            params = []
            for pm in re.finditer(
                r'(?:#\[arg\(([^)]*)\)\]\s*)?pub (\w+)\s*:\s*([^,\n]+)',
                body
            ):
                arg_attrs = pm.group(1) or ""
                pname = pm.group(2)
                ptype = pm.group(3).strip().rstrip(",")
                if pname in ("__clap_help", "__clap_version"):
                    continue
                help_m = re.search(r'help\s*=\s*"([^"]+)"', arg_attrs)
                phelp = help_m.group(1) if help_m else ""
                required = "Option<" not in ptype and "bool" not in ptype
                params.append({"name": pname, "type": ptype, "required": required, "help": phelp})
            struct_blocks[struct_name] = {"desc": desc, "params": params}

        # Find impl_mcp_tool! bindings
        for mm in re.finditer(
            r'impl_mcp_tool!\s*\(\s*(\w+)\s*,\s*"([^"]+)"\s*,\s*\[([^\]]*)\]\s*\)',
            content
        ):
            struct_name, tool_name, path_raw = mm.group(1), mm.group(2), mm.group(3)
            cmd_path = re.findall(r'"([^"]+)"', path_raw)
            info = struct_blocks.get(struct_name, {"desc": "", "params": []})
            all_tools.append({
                "name": tool_name,
                "desc": info["desc"],
                "cmd": " ".join(cmd_path),
                "params": info["params"],
            })

        # Pattern 2: serde Params structs (rag_mcp_tools, etc.)
        for sm in re.finditer(
            r'((?:///[^\n]*\n)+)?pub struct (\w+Params)\s*\{([^}]+)\}',
            content, re.DOTALL
        ):
            doc_raw = sm.group(1) or ""
            struct_name = sm.group(2)
            body = sm.group(3)
            desc = " ".join(l.lstrip("/ ").strip() for l in doc_raw.splitlines() if l.strip())
            params = []
            for line in body.splitlines():
                pm = re.match(r'\s*pub (\w+)\s*:\s*([^,/]+)', line)
                if pm:
                    pname, ptype = pm.group(1), pm.group(2).strip()
                    doc_m = re.search(r'//+\s*(.+)', line)
                    phelp = doc_m.group(1).strip() if doc_m else ""
                    params.append({"name": pname, "type": ptype,
                                   "required": "Option<" not in ptype, "help": phelp})
            tool_name = re.sub(r'Params$', '', struct_name)
            tool_name = re.sub(r'([A-Z])', r'_\1', tool_name).lower().lstrip('_')
            all_tools.append({"name": tool_name, "desc": desc, "cmd": "", "params": params})

    # Emit Q&A rows per tool
    for tool in all_tools:
        name = tool["name"]
        desc = tool["desc"]
        params = tool["params"]
        cmd = tool["cmd"]

        if not name:
            continue

        # Param schema Q&A
        if params:
            req = [p for p in params if p["required"]]
            opt = [p for p in params if not p["required"]]
            schema_lines = []
            for p in params:
                marker = "required" if p["required"] else "optional"
                help_str = f" — {p['help']}" if p["help"] else ""
                schema_lines.append(f"  {p['name']}: {p['type']} ({marker}){help_str}")
            schema = "\n".join(schema_lines)
            rows.append(_r(
                f"What parameters does the `{name}` MCP tool take?",
                f"Tool: `{name}`\n{schema}"
            ))

            # JSON call example
            example = {p["name"]: f"<{p['type'].lower().strip('option<>').strip()}>"
                       for p in req}
            rows.append(_r(
                f"How do I call `{name}` via MCP?",
                f"```json\n{json.dumps({'tool': name, 'arguments': example}, indent=2)}\n```"
            ))

        if desc:
            rows.append(_r(f"What does the `{name}` MCP tool do?", desc))

        if cmd:
            rows.append(_r(
                f"What b00t-cli command does `{name}` map to?",
                f"`b00t-cli {cmd}`"
            ))

    # Emit full tool index
    if all_tools:
        tool_list = "\n".join(f"- `{t['name']}`: {t['desc'][:80]}" for t in all_tools if t['desc'])
        rows.append(_r("List all available b00t MCP tools.", tool_list))

    return rows


# ── Mermaid diagram parser ────────────────────────────────────────────────────

def parse_mermaid(path: Path, content: str) -> list[dict]:
    rows: list[dict] = []
    # Find all mermaid blocks (standalone .mermaid file or ```mermaid blocks in .md)
    if path.suffix == ".mermaid":
        blocks = [("", content)]
    else:
        blocks = [(m.group(1) or path.stem, m.group(2))
                  for m in re.finditer(r"```mermaid\s*\n(.*?)\n```", content, re.DOTALL)]
    if not blocks:
        return []

    for label, block in blocks:
        diagram_name = label.strip() or path.stem
        # Extract subgraph names
        subgraphs = re.findall(r'subgraph\s+(\w+)\[?"?([^"\]]+)"?\]?', block)
        if subgraphs:
            sg_list = "; ".join(f"{k}: {v.strip()}" for k, v in subgraphs)
            rows.append(_r(f"What subgraphs exist in the {diagram_name} diagram?", sg_list))

        # Extract node labels (strip HTML tags)
        node_labels: dict[str, str] = {}
        for m in re.finditer(r'(\w+)\["?([^"\]]+)"?\]', block):
            nid, label_raw = m.group(1), m.group(2)
            label_clean = re.sub(r"<br\s*/?>", " ", label_raw).strip()
            if len(label_clean) > 3 and not label_clean.startswith("%"):
                node_labels[nid] = label_clean

        # Extract edges A --> B and build adjacency
        adjacency: dict[str, list[str]] = {}
        for m in re.finditer(r"(\w+)\s*-->+\s*(?:\|[^|]*\|)?\s*(\w+)", block):
            src, dst = m.group(1), m.group(2)
            adjacency.setdefault(src, []).append(dst)

        if node_labels and adjacency:
            # Prose walk for the main flow
            visited, walk = set(), []
            # Find entry node (most referenced as dst = not a root)
            dst_set = {d for dsts in adjacency.values() for d in dsts}
            roots = [n for n in adjacency if n not in dst_set]
            start = roots[0] if roots else next(iter(adjacency))
            queue = [start]
            while queue and len(walk) < 12:
                node = queue.pop(0)
                if node in visited:
                    continue
                visited.add(node)
                label = node_labels.get(node, node)
                nexts = adjacency.get(node, [])
                if nexts:
                    next_labels = [node_labels.get(n, n) for n in nexts[:2]]
                    walk.append(f"{label} → {', '.join(next_labels)}")
                    queue.extend(nexts)
                else:
                    walk.append(label)
            if walk:
                rows.append(_r(
                    f"Describe the {diagram_name} flow.",
                    "\n".join(walk)
                ))

        # Subgraph Q&A: "What follows X in the diagram?"
        for sg_id, sg_name in subgraphs[:4]:
            rows.append(_r(
                f"What is the {sg_name.strip()} phase in {diagram_name}?",
                "\n".join(f"- {node_labels[n]}" for n in node_labels
                          if n.startswith(sg_id[0]) and n in node_labels)[:3] or sg_name.strip()
            ))

    return rows


# ── MCP registry parser ───────────────────────────────────────────────────────

def parse_mcp_registry(path: Path) -> list[dict]:
    rows: list[dict] = []
    try:
        data = json.loads(path.read_text())
    except Exception:
        return []
    # data may be a list or dict
    entries = data if isinstance(data, list) else list(data.values()) if isinstance(data, dict) else []
    for entry in entries[:80]:  # cap to avoid flooding
        if not isinstance(entry, dict):
            continue
        name = entry.get("name") or entry.get("id", "")
        desc = entry.get("description", "")
        tags = entry.get("tags", [])
        health = entry.get("health_status", "")
        if name and desc:
            rows.append(_r(f"What does the {name} MCP server do?", desc.strip()))
        if name and tags:
            tag_str = ", ".join(tags) if isinstance(tags, list) else str(tags)
            rows.append(_r(f"What tags describe the {name} MCP server?", tag_str))
        if name and health:
            rows.append(_r(f"What is the health status of {name}?", str(health)))
    return rows


# ── Guard violations parser (safety training signal) ─────────────────────────

def parse_guard_violations(path: Path) -> list[dict]:
    rows: list[dict] = []
    try:
        lines = path.read_text().splitlines()
    except Exception:
        return []
    # Top blocked patterns
    entries = []
    for line in lines:
        try:
            e = json.loads(line)
            entries.append(e)
        except Exception:
            continue
    # Sort by count descending, take top 30 unique patterns
    entries.sort(key=lambda x: x.get("count", 0), reverse=True)
    seen_patterns: set[str] = set()
    for entry in entries:
        pattern = entry.get("pattern", "")
        count = entry.get("count", 0)
        if not pattern or pattern in seen_patterns:
            continue
        seen_patterns.add(pattern)
        # Map pattern to canonical b00t replacement
        replacements = {
            "pip install": "Use `uv pip install` — pip is 🦨 blocked by command guard.",
            "pip3": "Use `uv pip install` — pip3 is 🦨 blocked by command guard.",
            "docker run": "Use `podman --device nvidia.com/gpu=all` — docker run is 🦨 blocked.",
            "huggingface-cli": "Use `hf download` — huggingface-cli is 🦨 blocked.",
            "rm -rf /": "BLOCKED permanently — destructive root deletion.",
            "poetry": "Use `uv` — poetry is superseded by uv in b00t.",
            "black ": "Use `ruff format` — black is superseded by ruff in b00t.",
        }
        why = next((v for k, v in replacements.items() if k in pattern), f"Guard blocked: `{pattern}` (hit {count}×).")
        rows.append(_r(f"Why is `{pattern}` blocked in b00t?", why))
        if len(rows) >= 30:
            break
    return rows


# ── CLAUDE.md protocol parser ─────────────────────────────────────────────────

def parse_claude_md(path: Path, content: str) -> list[dict]:
    # Same as parse_learn but no row cap — these are core laws
    rows: list[dict] = []
    sections = re.split(r"\n(#{1,3} .+)\n", content)
    current = path.stem
    for part in sections:
        if part.startswith("#"):
            current = part.lstrip("# ").strip()
            continue
        text = part.strip()
        if len(text) < 40:
            continue
        prose = re.sub(r"```.*?```", "", text, flags=re.DOTALL).strip()
        first_para = prose.split("\n\n")[0].strip()
        if len(first_para) > 60:
            rows.append(_r(f"What are the b00t rules for: {current}?", first_para))
        for cb in re.findall(r"```(?:\w*)\n(.*?)```", text, re.DOTALL):
            cb = cb.strip()
            if len(cb) > 10:
                rows.append(_r(f"Show a b00t example of {current}.", f"```\n{cb}\n```"))
    return rows


# ── Cargo rustdoc JSON parser ─────────────────────────────────────────────────
# Generate with: cargo +nightly rustdoc -p <pkg> --lib -- --output-format json -Z unstable-options

def parse_cargo_doc_json(path: Path) -> list[dict]:
    rows: list[dict] = []
    try:
        data = json.loads(path.read_text())
    except Exception:
        return []
    index = data.get("index", {})
    crate_name = data.get("root", "")
    root_item = index.get(crate_name, {})
    crate_label = root_item.get("name", path.stem)

    for item_id, item in index.items():
        name = item.get("name", "")
        docs = (item.get("docs") or "").strip()
        kind = item.get("kind", "")
        inner = item.get("inner", {})
        if not name or not docs or kind not in ("struct", "enum", "function", "trait"):
            continue

        rows.append(_r(
            f"What is {name} in {crate_label}?",
            f"{docs[:300]}"
        ))

        # Enum variants from rustdoc
        if kind == "enum":
            variants = [index.get(v, {}).get("name", "") for v in inner.get("variants", [])]
            variants = [v for v in variants if v]
            if variants:
                rows.append(_r(
                    f"What are the variants of {name}?",
                    ", ".join(variants)
                ))

        # Struct fields
        if kind == "struct":
            fields = []
            for fid in inner.get("fields", []):
                f = index.get(fid, {})
                fname = f.get("name", "")
                fdoc = (f.get("docs") or "").split(".")[0]
                if fname:
                    fields.append(f"{fname}" + (f": {fdoc}" if fdoc else ""))
            if fields:
                rows.append(_r(
                    f"What fields does {name} have?",
                    "\n".join(f"- {f}" for f in fields[:12])
                ))

    return rows[:200]


# ── Datum SPO triple generator (filesystem-based, no CLI) ─────────────────────

def generate_spo_triples(datum_dirs: list[Path]) -> list[dict]:
    rows: list[dict] = []
    type_map: dict[str, str] = {}
    tags_map: dict[str, list[str]] = {}
    tier_map: dict[str, str] = {}

    for d in datum_dirs:
        for path in d.glob("*.tom*"):
            content = path.read_text(errors="ignore")
            name = path.stem
            # Extract type
            m = re.search(r'\btype\s*=\s*"([^"]+)"', content)
            if m:
                type_map[name] = m.group(1)
            # Extract tags
            m = re.search(r'#\s*tags:\s*(.+)', content)
            if m:
                tags_map[name] = [t.strip() for t in m.group(1).split(",") if t.strip()]
            # Extract tier
            m = re.search(r'#\s*tier:\s*(\S+)', content)
            if m:
                tier_map[name] = m.group(1)

    # Emit cross-type Q&A ("which datums are type X?")
    by_type: dict[str, list[str]] = {}
    for name, t in type_map.items():
        by_type.setdefault(t, []).append(name)
    for dtype, names in by_type.items():
        if len(names) >= 3:
            sample = ", ".join(names[:8])
            rows.append(_r(f"Name some b00t datums of type \"{dtype}\".",
                            f"Examples: {sample}" + (" (and more)" if len(names) > 8 else "")))

    # Cross-tier Q&A
    by_tier: dict[str, list[str]] = {}
    for name, tier in tier_map.items():
        by_tier.setdefault(tier, []).append(name)
    for tier, names in by_tier.items():
        sample = ", ".join(names[:6])
        rows.append(_r(f"Which datums run at the {tier} cognitive tier?",
                        f"{tier} tier: {sample}" + (" (and more)" if len(names) > 6 else "")))

    # Per-datum type triple
    for name, t in list(type_map.items())[:200]:
        tags = tags_map.get(name, [])
        tier = tier_map.get(name, "")
        facts = [f"type: {t}"]
        if tier:
            facts.append(f"tier: {tier}")
        if tags:
            facts.append(f"tags: {', '.join(tags[:5])}")
        rows.append(_r(f"What are the ontology facts for the {name} datum?",
                        " | ".join(facts)))

    return rows



def parse_shell_script(path: Path, content: str) -> list[dict]:
    """Extract Q&A from shell script header comments and option blocks."""
    rows = []
    name = path.stem

    # Extract top-of-file docstring (lines starting with # before first non-comment)
    header_lines = []
    for line in content.splitlines():
        stripped = line.strip()
        if stripped.startswith("#!"):
            continue
        if stripped.startswith("#"):
            header_lines.append(stripped.lstrip("# ").strip())
        elif stripped:
            break

    header = "\n".join(l for l in header_lines if l)
    if header:
        rows.append(_r(
            f"What does {name}.sh do?",
            header[:600],
        ))

    # Extract Usage: lines
    usage_lines = [l for l in content.splitlines() if "Usage:" in l or "usage:" in l]
    if usage_lines:
        usage = "\n".join(l.strip().lstrip("# ").strip() for l in usage_lines[:5])
        rows.append(_r(f"What is the usage for {name}.sh?", usage))

    # Extract --flag lines as option Q&A
    flags = re.findall(r"--(\w[\w-]*)(?:\s*[=|]\s*\S+)?\s*[)#\]]\s*(.+)", content)
    if flags:
        flag_doc = "\n".join(f"  --{f}: {d.rstrip()}" for f, d in flags[:10])
        rows.append(_r(f"What options does {name}.sh accept?", flag_doc))

    # Extract mode-specific sections (case "$arg" or similar)
    modes = re.findall(r'--mode[=\s]+(\w[\w-]*)', content)
    if modes:
        rows.append(_r(
            f"What modes does {name}.sh support?",
            "Modes: " + ", ".join(sorted(set(modes))),
        ))

    return rows[:8]


# ─── Mermaid visualization training examples ──────────────────────────────────


def _strip_mermaid_fences(raw: str) -> str:
    """Strip ```mermaid ... ``` fences from b00t viz output."""
    text = re.sub(r"^```mermaid\s*\n?", "", raw)
    text = re.sub(r"\n?\s*```\s*$", "", text)
    return text.strip()


def parse_mermaid_viz(path: Path, content: str) -> list[dict[str, str]]:
    """Generate Mermaid dependency graph training examples via b00t-cli viz.

    For each datum, runs ``b00t viz entangle --datum=<key> --format=mermaid``
    and produces an Alpaca pair teaching the model to emit raw Mermaid syntax.
    """
    rows: list[dict[str, str]] = []
    datum_key = path.stem  # matches b00t-cli datum key derivation

    # Extract human-readable topic from heading or [datum].name
    topic = datum_key
    heading = re.search(r"^#\s+(.+)$", content, re.MULTILINE)
    if heading:
        topic = heading.group(1).strip()
    datum_name = re.search(r'^\s*name\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if datum_name:
        topic = datum_name.group(1)

    # Safety: datum key must be non-empty and non-pathological
    if not datum_key or datum_key.startswith("."):
        return rows

    try:
        result = subprocess.run(
            ["b00t", "viz", "entangle",
             "--datum", datum_key,
             "--format", "mermaid"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode == 0:
            raw = _strip_mermaid_fences(result.stdout)
            if raw:
                rows.append({
                    "instruction": f"Generate a Mermaid graph showing {topic} dependencies",
                    "input": "",
                    "response": raw,
                })
        else:
            stderr = result.stderr.strip()[:200]
            print(f"  ⚠️  viz entangle --datum={datum_key} exited {result.returncode}: {stderr}",
                  file=sys.stderr)
    except FileNotFoundError:
        print("  ⚠️  b00t not found — install via `cargo build` or `just dev-link`", file=sys.stderr)
    except subprocess.TimeoutExpired:
        print(f"  ⚠️  viz entangle --datum={datum_key} timed out (30s)", file=sys.stderr)

    return rows


def generate_global_mermaid_viz() -> list[dict[str, str]]:
    """Generate training examples from global Mermaid visualizations.

    Runs ``b00t viz entangle`` (full graph) and ``b00t viz task``,
    stripping the code fences so the model learns raw Mermaid syntax.
    """
    rows: list[dict[str, str]] = []

    try:
        # Full entanglement graph
        result = subprocess.run(
            ["b00t", "viz", "entangle", "--format", "mermaid"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode == 0:
            raw = _strip_mermaid_fences(result.stdout)
            if raw:
                rows.append({
                    "instruction": "Generate a Mermaid graph showing all datum entanglement dependencies",
                    "input": "",
                    "response": raw,
                })

        # Task dependency graph
        result = subprocess.run(
            ["b00t", "viz", "task", "--format", "mermaid"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode == 0:
            raw = _strip_mermaid_fences(result.stdout)
            if raw:
                rows.append({
                    "instruction": "Generate a Mermaid graph showing task dependencies",
                    "input": "",
                    "response": raw,
                })
    except FileNotFoundError:
        print("  ⚠️  b00t not found — skipping global mermaid viz", file=sys.stderr)
    except subprocess.TimeoutExpired:
        print("  ⚠️  global mermaid viz timed out (30s)", file=sys.stderr)

    return rows


# ─── Main generator ───────────────────────────────────────────────────────────


def generate_dataset(output_path: str, format: str = "alpaca", max_rows: int = 5000) -> int:
    """Generate the training dataset from all corpus sources."""
    rows: list[dict[str, str]] = []

def main():
    ap = argparse.ArgumentParser(description="Generate b00t fine-tuning dataset")
    ap.add_argument("--output", default="fine-tune/train.jsonl")
    ap.add_argument("--format", choices=["alpaca", "chatml"], default="alpaca")
    ap.add_argument("--max-rows", type=int, default=15000)
    ap.add_argument("--viz", metavar="OUT_SVG", default=None,
                    help="Generate token-length + category SVG after writing dataset")
    args = ap.parse_args()

    rows: list[dict] = [_r(i, r) for i, r in COMMAND_PATTERNS]

    # ── Datum files (all types via unified parser) ──
    datum_source_keys = ["datums", "schema", "cli_datums", "mcp_datums", "skill_datums",
                         "role_datums", "agent_datums", "ai_datums", "tomllm"]
    for key in datum_source_keys:
        for path in SOURCES.get(key, []):
            try:
                content = path.read_text(errors="ignore")
                r = parse_datum(path, content)
                rows.extend(r)
                if r:
                    print(f"  ✓ [{key}] {path.name}: {len(r)} rows")
            except Exception as e:
                print(f"  ✗ {path.name}: {e}", file=sys.stderr)

            before = len(rows)
            if source_name == "datums":
                rows.extend(parse_datum_instruction(path, content))
                rows.extend(parse_mermaid_viz(path, content))
            elif source_name == "learn":
                rows.extend(parse_learn_instruction(path, content))
            elif source_name == "justfile":
                rows.extend(parse_justfile(content))
            elif source_name == "agents":
                rows.extend(parse_agent_instruction(path, content))
            after = len(rows)
            if after > before:
                print(f"  ✓ {path.name}: {after - before} rows", file=sys.stderr)

    # Layer 2: generative Mermaid graph training pairs from viz commands
    global_mermaid_rows = generate_global_mermaid_viz()
    rows.extend(global_mermaid_rows)
    print(f"  ✓ global mermaid viz: {len(global_mermaid_rows)} rows", file=sys.stderr)

    print(f"\nTotal: {len(rows)} rows generated", file=sys.stderr)

    # ── Justfile ──
    for path in SOURCES["justfile"]:
        try:
            r = parse_justfile(path.read_text())
            rows.extend(r)
            print(f"  ✓ justfile: {len(r)} rows")
        except Exception as e:
            print(f"  ✗ justfile: {e}", file=sys.stderr)

    # ── Agent roles ──
    for path in SOURCES["agents"]:
        try:
            rows.extend(parse_agent(path, path.read_text(errors="ignore")))
        except Exception:
            pass

    # ── LFMF lessons ──
    for path in SOURCES["lfmf"]:
        try:
            rows.extend(parse_lfmf(path, path.read_text(errors="ignore")))
        except Exception:
            pass

    # ── Vendor + workspace module docs ──
    vendor_count = 0
    for path in SOURCES.get("vendor_docs", []):
        try:
            content = path.read_text(errors="ignore")
            r = parse_learn(path, content)
            rows.extend(r)
            vendor_count += len(r)
        except Exception:
            pass
    print(f"  ✓ vendor_docs: {vendor_count} rows from {len(SOURCES.get('vendor_docs', []))} files")

    # ── Rust source (API syntax) ──
    rust_count = 0
    for path in SOURCES.get("rust_src", []):
        try:
            content = path.read_text(errors="ignore")
            r = parse_rust_docs(path, content)
            rows.extend(r)
            rust_count += len(r)
        except Exception:
            pass
    print(f"  ✓ rust_src: {rust_count} rows from {len(SOURCES.get('rust_src', []))} files")

    # ── Mermaid diagrams (standalone + embedded in docs) ──
    mermaid_count = 0
    for path in SOURCES.get("mermaid_md", []):
        try:
            content = path.read_text(errors="ignore")
            r = parse_mermaid(path, content)
            rows.extend(r)
            mermaid_count += len(r)
        except Exception:
            pass
    print(f"  ✓ mermaid: {mermaid_count} rows")

    # ── MCP registry ──
    for path in SOURCES.get("mcp_registry", []):
        try:
            r = parse_mcp_registry(path)
            rows.extend(r)
            print(f"  ✓ mcp_registry: {len(r)} rows")
        except Exception as e:
            print(f"  ✗ mcp_registry: {e}", file=sys.stderr)

    # ── Guard violations (safety signal) ──
    for path in SOURCES.get("guard_log", []):
        try:
            r = parse_guard_violations(path)
            rows.extend(r)
            print(f"  ✓ guard_violations: {len(r)} rows")
        except Exception:
            pass

    # ── CLAUDE.md core protocol ──
    for path in SOURCES.get("claude_md", []):
        try:
            r = parse_claude_md(path, path.read_text(errors="ignore"))
            rows.extend(r)
            print(f"  ✓ claude_md [{path.name}]: {len(r)} rows")
        except Exception:
            pass

    # ── MCP tool call surfaces ──
    mcp_tool_files = [
        B00T / "b00t-mcp/src/mcp_tools.rs",
        B00T / "b00t-mcp/src/proxy_mcp_tools.rs",
        B00T / "b00t-mcp/src/rag_mcp_tools.rs",
        B00T / "b00t-mcp/src/acp_tools.rs",
        B00T / "b00t-mcp/src/mcp_registry_tools.rs",
    ]
    r = parse_mcp_tool_surface([p for p in mcp_tool_files if p.exists()])
    rows.extend(r)
    print(f"  ✓ mcp_tool_surface: {len(r)} rows")

    # ── rustdoc JSON (when available — generated by `just finetune-rustdoc`) ──
    for pkg in ["b00t_cli", "b00t_c0re_lib", "b00t_datum_core"]:
        json_path = B00T / f"target/doc/{pkg}.json"
        if json_path.exists():
            r = parse_cargo_doc_json(json_path)
            rows.extend(r)
            print(f"  ✓ rustdoc/{pkg}: {len(r)} rows")

    # ── SPO triples (cross-type/tier convergence) ──
    datum_dirs = [B00T / "_b00t_", B00T / "_b00t_/datums", B00T / "_b00t_/schema"]
    r = generate_spo_triples([d for d in datum_dirs if d.exists()])
    rows.extend(r)
    print(f"  ✓ spo_triples: {len(r)} rows")

    # ── Shell scripts (install-b00t.sh, etc.) ──
    script_count = 0
    for path in SOURCES.get("scripts", []):
        try:
            content_sh = path.read_text(errors="ignore")
            r = parse_shell_script(path, content_sh)
            rows.extend(r)
            script_count += len(r)
        except Exception:
            pass
    print(f"  ✓ scripts: {script_count} rows from {len(SOURCES.get('scripts', []))} files")

    print(f"\nTotal: {len(rows)} rows generated")

    # Dedup on instruction text
    seen, deduped = set(), []
    for row in rows:
        key = row["instruction"].lower().strip()
        if key not in seen:
            seen.add(key)
            deduped.append(row)
    rows = deduped[:args.max_rows]
    print(f"After dedup: {len(rows)} rows")

    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")

    print(f"\n✅ Generated {len(rows)} training rows → {out}")

    if args.viz:
        _write_viz(rows, Path(args.viz))


def _write_viz(rows: list[dict], out: Path) -> None:
    """SVG token-length histogram + category breakdown for data quality inspection."""
    try:
        import unicodedata
        # Approximate token count: chars / 4 (rough GPT tokenizer estimate)
        lengths = [len(r.get("output", "") + r.get("instruction", "")) // 4 for r in rows]
        # Category from instruction prefix heuristics
        cats: dict[str, int] = {}
        for r in rows:
            instr = r.get("instruction", "")
            cat = (
                "datum" if "datum" in instr.lower() else
                "rust" in instr.lower() and "rust" or
                "justfile" in instr.lower() and "just" or
                "lfmf" in instr.lower() and "lfmf" or
                "mcp" in instr.lower() and "mcp" or
                "other"
            )
            cats[cat] = cats.get(cat, 0) + 1

        # Build minimal SVG histogram (no matplotlib dependency)
        buckets = [0] * 20   # 0-100, 100-200, ... 1900-2000+
        for l in lengths:
            b = min(l // 100, 19)
            buckets[b] += 1
        max_b = max(buckets) or 1
        W, H, pad = 600, 280, 40
        bar_w = (W - 2 * pad) / 20

        svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H+100}">']
        svg.append(f'<rect width="{W}" height="{H+100}" fill="#1e1e2e"/>')
        svg.append(f'<text x="{W//2}" y="20" fill="#cdd6f4" font-size="13" text-anchor="middle">'
                   f'b00t training dataset — token length distribution ({len(rows)} examples)</text>')
        for i, cnt in enumerate(buckets):
            bh = int((cnt / max_b) * (H - 2 * pad))
            x = pad + i * bar_w
            y = pad + (H - 2 * pad) - bh
            color = "#89b4fa" if i < 10 else "#f38ba8"
            svg.append(f'<rect x="{x:.1f}" y="{y}" width="{bar_w - 1:.1f}" height="{bh}" fill="{color}"/>')
            if i % 4 == 0:
                svg.append(f'<text x="{x+bar_w/2:.1f}" y="{H - pad + 14}" fill="#a6adc8" '
                           f'font-size="9" text-anchor="middle">{i*100}</text>')
        # Category legend
        cx, cy = pad, H + pad + 10
        for cat, cnt in sorted(cats.items(), key=lambda x: -x[1])[:8]:
            svg.append(f'<text x="{cx}" y="{cy}" fill="#cdd6f4" font-size="10">'
                       f'{cat}: {cnt}</text>')
            cx += 90
            if cx > W - 80:
                cx, cy = pad, cy + 14
        svg.append('</svg>')

        out.write_text("\n".join(svg))
        print(f"📊 dataset viz → {out}  (open in browser; ⚠️ SVG not VLM-compatible — convert to PNG for Qwen3-VL)")
    except Exception as e:
        print(f"⚠️  viz skipped: {e}")


if __name__ == "__main__":
    main()
