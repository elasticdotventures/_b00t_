#!/usr/bin/env python3
"""Query the blessed crate manifest with trust tiers and multi-source support."""
import json, sys, os, re
try:
    import tomllib
except ImportError:
    import tomli as tomllib

BLESSED_DIR = os.path.expanduser("~/.dotfiles/_b00t_/blessed")
AWESOME_DIR = os.path.expanduser("~/.dotfiles/_b00t_/awesome")

TRUST_TIERS = {
    "blessed": {"label": "🥾 blessed", "weight": 3, "desc": "Hand-curated by blessed.rs maintainers"},
    "awesome": {"label": "📋 awesome", "weight": 2, "desc": "From community awesome-lists"},
    "local":   {"label": "📦 project", "weight": 1, "desc": "Detected in your project dependencies"},
}

def load_manifest(lang="rust", source="blessed"):
    dir_map = {"blessed": BLESSED_DIR, "awesome": AWESOME_DIR}
    path = os.path.join(dir_map.get(source, BLESSED_DIR), f"{lang}.toml")
    if not os.path.exists(path):
        return []
    with open(path, "rb") as f:
        data = tomllib.load(f)
    entries = data.get("crate", [])
    for e in entries:
        e["_source"] = source
        e["_trust"] = TRUST_TIERS.get(source, {}).get("label", source)
    return entries

def search(query, lang="rust", limit=5, sources=None):
    if sources is None:
        sources = ["blessed", "awesome"]
    
    all_entries = []
    for src in sources:
        all_entries.extend(load_manifest(lang, src))
    
    results = []
    query_lower = query.lower()
    query_words = query_lower.split()

    for entry in all_entries:
        category = entry.get("category", "").lower()
        use_case = entry.get("use_case", "").lower()
        recommended = entry.get("recommended", [])
        notes = entry.get("notes", {})
        source = entry.get("_source", "unknown")
        trust_weight = TRUST_TIERS.get(source, {}).get("weight", 0)

        score = 0
        if query_lower in use_case:
            score += 10
        for word in query_words:
            if word in use_case:
                score += 5
            if word in category:
                score += 2
            if word in " ".join(recommended).lower():
                score += 1

        if score > 0:
            results.append({
                "category": entry.get("category", ""),
                "use_case": entry.get("use_case", ""),
                "recommended": recommended,
                "notes": notes,
                "score": score,
                "source": source,
                "trust": TRUST_TIERS.get(source, {}).get("label", source),
            })

    # Sort by score, but boost blessed entries by their trust weight
    results.sort(key=lambda r: (r["score"] + trust_weight, r["score"]), reverse=True)
    return results[:limit]

def format_output(results, lang="rust"):
    lines = []
    if not results:
        lines.append(f"No results found for '{lang}' ecosystem.")
        return "\n".join(lines)

    lines.append(f"## Ecosystem Recommendations ({lang})\n")
    for r in results:
        trust = r.get("trust", "")
        lines.append(f"### {r['use_case']} — {r['category']} [{trust}]")
        for crate in r["recommended"]:
            note = r["notes"].get(crate, "")
            lines.append(f"- **{crate}**: {note}")
        lines.append("")
    return "\n".join(lines)

# ── Project scanner ───────────────────────────────────────────────────────

def scan_project(project_path="."):
    """Read Cargo.toml, package.json, go.mod etc. and extract dependencies."""
    deps = []
    cargo_toml = os.path.join(project_path, "Cargo.toml")
    package_json = os.path.join(project_path, "package.json")
    go_mod = os.path.join(project_path, "go.mod")
    
    if os.path.exists(cargo_toml):
        with open(cargo_toml, "rb") as f:
            data = tomllib.load(f)
        for section in ["dependencies", "dev-dependencies", "build-dependencies"]:
            for name, spec in data.get(section, {}).items():
                if isinstance(spec, dict):
                    deps.append(name)
                elif isinstance(spec, str):
                    deps.append(name)
        # Scan workspace members
        for member in data.get("workspace", {}).get("members", []):
            if "*" in member:
                continue  # skip globs
            member_toml = os.path.join(project_path, member, "Cargo.toml")
            if os.path.exists(member_toml):
                try:
                    with open(member_toml, "rb") as f:
                        md = tomllib.load(f)
                    for section in ["dependencies", "dev-dependencies", "build-dependencies"]:
                        for name, spec in md.get(section, {}).items():
                            if isinstance(spec, dict) and "path" not in spec:
                                deps.append(name)
                            elif isinstance(spec, str):
                                deps.append(name)
                except Exception:
                    pass
    
    if os.path.exists(package_json):
        with open(package_json) as f:
            data = json.load(f)
        for section in ["dependencies", "devDependencies"]:
            for name in data.get(section, {}):
                deps.append(name)
    
    if os.path.exists(go_mod):
        with open(go_mod) as f:
            for line in f:
                m = re.match(r'^\s*([\w.-]+)\s+v', line)
                if m:
                    deps.append(m.group(1))
    
    return sorted(set(deps))


def cross_reference(deps, lang="rust"):
    """Match project deps against blessed/awesome manifests."""
    all_entries = load_manifest(lang, "blessed") + load_manifest(lang, "awesome")
    matches = []
    unmatched = []
    
    for dep in deps:
        found = None
        for entry in all_entries:
            if dep in entry.get("recommended", []):
                found = entry
                break
        if found:
            matches.append({
                "dep": dep,
                "use_case": found["use_case"],
                "category": found["category"],
                "trust": found.get("_trust", "unknown"),
                "note": found.get("notes", {}).get(dep, ""),
            })
        else:
            unmatched.append(dep)
    
    return matches, unmatched


# ── Awesome-list scraper ──────────────────────────────────────────────────

def ingest_awesome_list(url, lang="rust", category_prefix=""):
    """Scrape a GitHub awesome-list README and convert to blessed TOML format."""
    import urllib.request, re
    
    # Convert github.com URL to raw
    raw_url = url.replace("github.com", "raw.githubusercontent.com").replace("/blob/", "/")
    
    try:
        with urllib.request.urlopen(raw_url, timeout=15) as resp:
            content = resp.read().decode("utf-8", errors="replace")
    except Exception as e:
        print(f"Error fetching {url}: {e}", file=sys.stderr)
        return []
    
    entries = []
    current_category = ""
    
    for line in content.split("\n"):
        # Category headers: ## Category Name
        h2 = re.match(r'^##\s+(.+)', line)
        if h2:
            current_category = h2.group(1).strip()
            continue
        
        # Crate entries: - [name](url) — description
        # or: * [name](url) — description  
        crate = re.match(r'^[-*]\s+\[([^\]]+)\]\([^)]+\)\s*[—–-]\s*(.+)', line)
        if crate:
            name = crate.group(1).strip()
            desc = crate.group(2).strip()
            cat = f"{category_prefix}{current_category}" if category_prefix else current_category
            
            # Sanitize description for TOML
            desc = desc.replace('"', "'").replace("\\", "").replace("\n", " ").replace("\r", "")[:200]
            entries.append({
                "category": cat or "Uncategorized",
                "use_case": name,
                "recommended": [name],
                "notes": {name: desc},
                "_source": "awesome",
                "_trust": TRUST_TIERS["awesome"]["label"],
            })
    
    return entries

if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("query", nargs="?", default="")
    p.add_argument("--lang", default="rust")
    p.add_argument("--json", action="store_true")
    p.add_argument("--limit", type=int, default=5)
    p.add_argument("--source", default="blessed", help="blessed, awesome, or all")
    p.add_argument("--list-categories", action="store_true")
    p.add_argument("--ingest", help="URL of awesome-list to scrape (prints TOML)")
    p.add_argument("--ingest-category", default="", help="Category prefix for ingestion")
    p.add_argument("--scan-project", metavar="PATH", help="Scan Cargo.toml/package.json for deps and cross-reference")
    args = p.parse_args()

    if args.ingest:
        entries = ingest_awesome_list(args.ingest, args.lang, args.ingest_category)
        if args.json:
            print(json.dumps(entries, indent=2))
        else:
            # Output as TOML (multi-line for safety with special chars)
            for e in entries:
                print("[[crate]]")
                print(f'category = "{e["category"]}"')
                print(f'use_case = "{e["use_case"]}"')
                recs = json.dumps(e["recommended"])
                print(f"recommended = {recs}")
                print("[crate.notes]")
                for k, v in e["notes"].items():
                    print(f'"{k}" = "{v}"')
                print()
    elif args.list_categories:
        if args.source == "all":
            sources = ["blessed", "awesome"]
        else:
            sources = [args.source]
        for src in sources:
            manifests = load_manifest(args.lang, src)
            cats = sorted(set(e["category"] for e in manifests))
            print(f"\n[{src}] {len(manifests)} entries, {len(cats)} categories:")
            for cat in cats:
                entries = [e for e in manifests if e["category"] == cat]
                print(f"  {cat}: {len(entries)} use-cases")
    elif args.scan_project:
        deps = scan_project(args.scan_project)
        matches, unmatched = cross_reference(deps, args.lang)
        if args.json:
            print(json.dumps({"matches": matches, "unmatched": unmatched, "total": len(deps)}, indent=2))
        else:
            print(f"## Project Scan: {args.scan_project}\n")
            print(f"**{len(deps)}** dependencies, **{len(matches)}** known, **{len(unmatched)}** unmatched\n")
            if matches:
                print("### Known (in blessed/awesome)")
                for m in matches:
                    print(f"- **{m['dep']}** [{m['trust']}] — {m['use_case']} ({m['category']})")
                    if m['note']:
                        print(f"  {m['note'][:120]}")
            if unmatched:
                print(f"\n### Unmatched ({len(unmatched)})")
                for u in unmatched:
                    print(f"- `{u}`")
    elif args.query:
        if args.source == "all":
            sources = ["blessed", "awesome"]
        else:
            sources = [args.source]
        results = search(args.query, args.lang, args.limit, sources)
        if args.json:
            print(json.dumps(results, indent=2))
        else:
            print(format_output(results, args.lang))
    else:
        for src in ["blessed", "awesome"]:
            manifests = load_manifest(args.lang, src)
            if manifests:
                print(f"[{src}] {len(manifests)} entries in {args.lang}")
