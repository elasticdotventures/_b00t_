#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""b00t harvest — mine Claude Code session transcripts for lfmf-shaped lessons.

Context compaction erases hard-won tribal knowledge from agent memory, but the
raw transcripts persist on disk (~/.claude/projects/*/*.jsonl). This tool
streams those transcripts and extracts candidate lessons with provenance:

  1. error->resolution pairs: a failing tool invocation followed within a
     window by a successful variant of the same command.
  2. explicit knowledge markers: 🤓 / lfmf / "lesson:" / "workaround" / "gotcha"
     in assistant text or executed `b00t lfmf` commands.
  3. repeated-command evolution: same binary invoked >=3 times with changing
     flags until success.

Output: harvest_candidates.jsonl, one candidate per line:
  {source_file, session_ts, kind, tool, candidate_lesson,
   evidence_excerpt (<=200 chars), confidence (0-1)}

Usage:
  uv run scripts/b00t-harvest.py ~/.claude/projects -o harvest_candidates.jsonl --report
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

# ---------------------------------------------------------------------------
# Tunables
# ---------------------------------------------------------------------------

WINDOW = 12  # max invocations between a failure and its resolving success
EXCERPT_LEN = 200
LESSON_LEN = 240

MARKER_WORDS = ("🤓", "lfmf", "lesson:", "workaround", "gotcha")
# lines that are boilerplate re-displays of docs/prompts, not earned knowledge
MARKER_NOISE = (
    "<topic>", "<lesson>", "b00t lfmf <", "lfmf lessons are",
    "system-reminder", "{{", "atone for mistakes",
)

ERROR_HINTS = (
    "error:", "error ", "failed", "failure", "traceback (most recent",
    "command not found", "no such file", "permission denied", "fatal:",
    "panicked at", "exit code 1", "exited with", "unexpected argument",
    "usage:", "econnrefused", "cannot ", "denied",
)

# wrappers whose *second* token is the real binary
WRAPPERS = {"sudo", "env", "uv", "uvx", "npx", "just", "timeout", "nohup", "command"}


# ---------------------------------------------------------------------------
# Streaming + schema helpers
# ---------------------------------------------------------------------------

def iter_events(path: Path):
    """Yield parsed JSON events from a transcript, skipping bad lines."""
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue


def _result_text(content) -> str:
    """tool_result content is either a string or a list of blocks."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for block in content:
            if isinstance(block, dict) and isinstance(block.get("text"), str):
                parts.append(block["text"])
        return "\n".join(parts)
    return ""


def looks_like_error(text: str) -> bool:
    head = text[:600].lower()
    return any(h in head for h in ERROR_HINTS)


def command_head(command: str) -> str:
    """Best-effort 'binary' identity for a shell command line."""
    cmd = command.strip()
    # skip leading VAR=val assignments
    tokens = cmd.split()
    while tokens and re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", tokens[0]):
        tokens.pop(0)
    if not tokens or tokens[0].startswith("#") or tokens[0].startswith("<"):
        return ""  # comment / heredoc fragment, not a binary
    head = tokens[0].rsplit("/", 1)[-1]
    if head in WRAPPERS and len(tokens) > 1:
        second = tokens[1].rsplit("/", 1)[-1]
        if second in ("run", "pip", "tool") and len(tokens) > 2:  # uv run X, uv pip X
            return f"{head} {second}"
        return f"{head} {second}"
    return head


def one_line(text: str, limit: int) -> str:
    return re.sub(r"\s+", " ", text).strip()[:limit]


# ---------------------------------------------------------------------------
# Timeline construction
# ---------------------------------------------------------------------------

def build_timeline(events):
    """Return (invocations, texts).

    invocations: [{idx, ts, tool, command, is_error, result_excerpt}]
    texts: [(ts, source, text)] assistant text blocks + user string prompts
    """
    pending = {}   # tool_use_id -> invocation dict (awaiting result)
    invocations = []
    texts = []

    for ev in events:
        etype = ev.get("type")
        ts = ev.get("timestamp", "")
        msg = ev.get("message")
        if not isinstance(msg, dict):
            continue
        content = msg.get("content")

        if etype == "assistant" and isinstance(content, list):
            for block in content:
                if not isinstance(block, dict):
                    continue
                bt = block.get("type")
                if bt == "tool_use":
                    name = block.get("name", "?")
                    inp = block.get("input") or {}
                    if name == "Bash":
                        cmd = str(inp.get("command", ""))
                    else:
                        cmd = one_line(json.dumps(inp, ensure_ascii=False), 300)
                    inv = {
                        "idx": len(invocations), "ts": ts, "tool": name,
                        "command": cmd, "is_error": False, "result_excerpt": "",
                    }
                    invocations.append(inv)
                    if block.get("id"):
                        pending[block["id"]] = inv
                elif bt == "text" and isinstance(block.get("text"), str):
                    texts.append((ts, "assistant", block["text"]))

        elif etype == "user":
            if isinstance(content, str):
                texts.append((ts, "user", content))
            elif isinstance(content, list):
                for block in content:
                    if not isinstance(block, dict):
                        continue
                    if block.get("type") == "tool_result":
                        inv = pending.pop(block.get("tool_use_id"), None)
                        if inv is None:
                            continue
                        text = _result_text(block.get("content"))
                        err = bool(block.get("is_error")) or looks_like_error(text)
                        inv["is_error"] = err
                        inv["result_excerpt"] = one_line(text, EXCERPT_LEN)

    return invocations, texts


# ---------------------------------------------------------------------------
# Extractors
# ---------------------------------------------------------------------------

def extract_error_resolutions(invocations, source_file):
    out = []
    used_fail = set()
    for i, fail in enumerate(invocations):
        if not fail["is_error"] or i in used_fail:
            continue
        fhead = (fail["tool"], command_head(fail["command"]) if fail["tool"] == "Bash" else "")
        if fail["tool"] == "Bash" and not fhead[1]:
            continue  # unidentifiable command (comment/heredoc) -- no lesson topic
        for j in range(i + 1, min(i + 1 + WINDOW, len(invocations))):
            ok = invocations[j]
            if ok["is_error"]:
                continue
            ohead = (ok["tool"], command_head(ok["command"]) if ok["tool"] == "Bash" else "")
            if ohead != fhead:
                continue
            differs = one_line(ok["command"], 500) != one_line(fail["command"], 500)
            conf = 0.45
            if differs:
                conf += 0.25
            if any(h in fail["result_excerpt"].lower() for h in ("usage:", "unexpected argument", "unknown flag", "unrecognized")):
                conf += 0.15
            conf += max(0.0, 0.1 - 0.01 * (j - i))  # proximity bonus
            topic = fhead[1] or fail["tool"]
            if differs:
                lesson = (f"{topic}: '{one_line(fail['command'], 80)}' fails "
                          f"({one_line(fail['result_excerpt'], 60)}); use '{one_line(ok['command'], 80)}'")
            else:
                lesson = (f"{topic}: '{one_line(fail['command'], 80)}' failed then succeeded on retry "
                          f"(transient: {one_line(fail['result_excerpt'], 60)})")
                conf = min(conf, 0.35)
            out.append({
                "source_file": source_file,
                "session_ts": fail["ts"],
                "kind": "error_resolution",
                "tool": fail["tool"],
                "candidate_lesson": one_line(lesson, LESSON_LEN),
                "evidence_excerpt": one_line(
                    f"FAIL: {fail['command']} => {fail['result_excerpt']}", EXCERPT_LEN),
                "confidence": round(min(conf, 1.0), 2),
                "working_cmd": one_line(ok["command"], 300),
            })
            used_fail.add(i)
            break
    return out


def _marker_in(line: str):
    low = line.lower()
    for m in MARKER_WORDS:
        if m in low or m in line:
            return m.strip(":")
    return None


def extract_markers(invocations, texts, source_file):
    out = []
    # 1. executed `b00t lfmf` commands — already lfmf-shaped, highest confidence
    lfmf_re = re.compile(
        r"""lfmf\s+(?:add\s+)?(?:--?[\w-]+(?:[= ]\S+)?\s+)*([\w./:🥾-]+)\s+(['"])(.*?)\2""", re.S)
    for inv in invocations:
        if inv["tool"] != "Bash" or "lfmf" not in inv["command"]:
            continue
        m = lfmf_re.search(inv["command"])
        if not m:
            continue
        topic, _, lesson = m.groups()
        # skip flag tokens misparsed as topics, template/code fragments, stubs
        if topic.startswith("-") or len(lesson.strip()) < 15 or "${" in lesson:
            continue
        out.append({
            "source_file": source_file,
            "session_ts": inv["ts"],
            "kind": "marker",
            "marker": "lfmf",
            "tool": "Bash",
            "candidate_lesson": one_line(f"{topic}: {lesson}", LESSON_LEN),
            "evidence_excerpt": one_line(inv["command"], EXCERPT_LEN),
            "confidence": 0.95,
        })

    # 2. marker words in conversational text (assistant + user prompts)
    for ts, source, text in texts:
        for line in text.splitlines():
            marker = _marker_in(line)
            if not marker:
                continue
            low = line.lower()
            if any(n in low for n in MARKER_NOISE):
                continue
            body = line.strip().lstrip("-*#> ").strip()
            # the lesson is what FOLLOWS the marker; strip the sentence prefix
            # so re-statements with different lead-ins dedupe together
            pos = low.find(marker if marker != "🤓" else "🤓")
            if pos > 0:
                tail = line[pos:].lstrip("🤓").lstrip(": ").strip()
                if len(tail) >= 15:
                    body = tail
            if len(body) < 15 or len(body.split()) < 4:
                continue
            if "${" in body or body.rstrip().endswith((":", "\\")):
                continue  # template/code fragment or truncated lead-in, not a lesson
            conf = 0.75 if marker in ("🤓", "lfmf") else 0.5
            if source == "user":
                conf += 0.1  # operator-stated knowledge
            lesson = body if ": " in body else f"{marker}: {body}"
            out.append({
                "source_file": source_file,
                "session_ts": ts,
                "kind": "marker",
                "marker": marker,
                "tool": None,
                "candidate_lesson": one_line(lesson, LESSON_LEN),
                "evidence_excerpt": one_line(body, EXCERPT_LEN),
                "confidence": round(min(conf, 1.0), 2),
            })
    return out


def extract_evolutions(invocations, source_file):
    out = []
    by_head = defaultdict(list)
    for inv in invocations:
        if inv["tool"] != "Bash":
            continue
        head = command_head(inv["command"])
        if head:
            by_head[head].append(inv)

    for head, runs in by_head.items():
        # walk contiguous-ish sequences: >=3 attempts, >=2 distinct commands,
        # at least one error, ending in success
        seq = []
        for inv in runs:
            seq.append(inv)
            if inv["is_error"]:
                continue
            distinct = {one_line(x["command"], 500) for x in seq}
            errs = [x for x in seq if x["is_error"]]
            if len(seq) >= 3 and len(distinct) >= 2 and errs:
                first_fail, final = errs[0], inv
                lesson = (f"{head}: after {len(seq)} attempts, "
                          f"'{one_line(final['command'], 100)}' works "
                          f"(first failure: {one_line(first_fail['result_excerpt'], 60)})")
                conf = min(0.4 + 0.1 * len(errs) + 0.1 * (len(distinct) - 1), 0.85)
                out.append({
                    "source_file": source_file,
                    "session_ts": final["ts"],
                    "kind": "evolution",
                    "tool": head,
                    "candidate_lesson": one_line(lesson, LESSON_LEN),
                    "evidence_excerpt": one_line(
                        f"{first_fail['command']} => {first_fail['result_excerpt']}", EXCERPT_LEN),
                    "confidence": round(conf, 2),
                })
            seq = []  # reset after any success
    return out


# ---------------------------------------------------------------------------
# Dedup
# ---------------------------------------------------------------------------

_VOLATILE = [
    (re.compile(r"/[\w./~-]{6,}"), "<path>"),
    (re.compile(r"\b[0-9a-f]{7,40}\b"), "<hex>"),
    (re.compile(r"\b\d+\b"), "<n>"),
    (re.compile(r"toolu_\w+"), "<id>"),
]


def dedup_key(kind: str, lesson: str) -> str:
    norm = lesson.lower()
    for pat, repl in _VOLATILE:
        norm = pat.sub(repl, norm)
    norm = re.sub(r"\s+", " ", norm).strip()
    return hashlib.sha1(f"{kind}|{norm}".encode()).hexdigest()


def dedupe(candidates):
    best = {}
    for c in candidates:
        k = dedup_key(c["kind"], c["candidate_lesson"])
        if k not in best or c["confidence"] > best[k]["confidence"]:
            best[k] = c
    return list(best.values())


# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------

def harvest_file(path: Path) -> list[dict]:
    source_file = str(path)
    invocations, texts = build_timeline(iter_events(path))
    cands = []
    cands += extract_error_resolutions(invocations, source_file)
    cands += extract_markers(invocations, texts, source_file)
    cands += extract_evolutions(invocations, source_file)
    return dedupe(cands)


def harvest_tree(root: Path) -> list[dict]:
    cands = []
    files = sorted(root.rglob("*.jsonl")) if root.is_dir() else [root]
    for f in files:
        try:
            cands.extend(harvest_file(f))
        except OSError as e:
            print(f"warn: skipping {f}: {e}", file=sys.stderr)
    return dedupe(cands), len(files) if root.is_dir() else 1


def report(candidates):
    by_kind = Counter(c["kind"] for c in candidates)
    by_tool = Counter(str(c["tool"]) for c in candidates)
    print(f"total candidates: {len(candidates)}")
    print("\nby kind:")
    for k, n in by_kind.most_common():
        print(f"  {k:18} {n}")
    print("\nby tool (top 10):")
    for t, n in by_tool.most_common(10):
        print(f"  {t:24} {n}")
    print("\ntop 20 by confidence:")
    top = sorted(candidates, key=lambda c: -c["confidence"])[:20]
    for c in top:
        print(f"  [{c['confidence']:.2f}] ({c['kind']}) {c['candidate_lesson'][:150]}")


def main(argv=None):
    ap = argparse.ArgumentParser(description="Mine Claude Code transcripts for lfmf-shaped lessons.")
    ap.add_argument("root", type=Path, help="transcript root dir (e.g. ~/.claude/projects) or single .jsonl")
    ap.add_argument("-o", "--output", type=Path, default=Path("harvest_candidates.jsonl"))
    ap.add_argument("--report", action="store_true", help="print counts + top-20 candidates")
    ap.add_argument("--min-confidence", type=float, default=0.0)
    args = ap.parse_args(argv)

    root = args.root.expanduser()
    candidates, n_files = harvest_tree(root)
    candidates = [c for c in candidates if c["confidence"] >= args.min_confidence]
    candidates.sort(key=lambda c: (-c["confidence"], c["source_file"]))

    with open(args.output, "w", encoding="utf-8") as out:
        for c in candidates:
            out.write(json.dumps(c, ensure_ascii=False) + "\n")

    print(f"scanned {n_files} transcript file(s); wrote {len(candidates)} candidates -> {args.output}")
    if args.report:
        report(candidates)
    return 0


if __name__ == "__main__":
    sys.exit(main())
