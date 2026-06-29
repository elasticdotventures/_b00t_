#!/usr/bin/env python3
"""Query the blessed crate manifest with semantic search."""
import json, sys, os, tomllib

BLESSED_DIR = os.path.expanduser("~/.dotfiles/_b00t_/blessed")

def load_manifest(lang="rust"):
    path = os.path.join(BLESSED_DIR, f"{lang}.toml")
    if not os.path.exists(path):
        return []
    with open(path, "rb") as f:
        data = tomllib.load(f)
    return data.get("crate", [])

def search(query, lang="rust", limit=5):
    manifests = load_manifest(lang)
    results = []
    query_lower = query.lower()

    for entry in manifests:
        category = entry.get("category", "").lower()
        use_case = entry.get("use_case", "").lower()
        recommended = entry.get("recommended", [])
        notes = entry.get("notes", {})

        # Score: exact use_case > category match > crate name match
        score = 0
        if query_lower in use_case:
            score += 10
        if any(word in use_case for word in query_lower.split()):
            score += 5
        if any(word in category for word in query_lower.split()):
            score += 2
        if query_lower in " ".join(recommended).lower():
            score += 1

        if score > 0:
            results.append({
                "category": entry.get("category", ""),
                "use_case": entry.get("use_case", ""),
                "recommended": recommended,
                "notes": notes,
                "score": score,
            })

    results.sort(key=lambda r: r["score"], reverse=True)
    return results[:limit]

def format_output(results, lang="rust"):
    lines = []
    if not results:
        lines.append(f"No blessed {lang} crates found matching your query.")
        return "\n".join(lines)

    lines.append(f"## Blessed {lang.title()} Crates\n")
    for r in results:
        lines.append(f"### {r['use_case']} ({r['category']})")
        for crate in r["recommended"]:
            note = r["notes"].get(crate, "")
            stars = r.get("metrics", {}).get(crate, {}).get("stars", "")
            star_str = f" ⭐{stars}" if stars else ""
            lines.append(f"- **{crate}**{star_str}: {note}")
        lines.append("")
    return "\n".join(lines)

if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("query", nargs="?", default="")
    p.add_argument("--lang", default="rust")
    p.add_argument("--json", action="store_true")
    p.add_argument("--limit", type=int, default=5)
    p.add_argument("--list-categories", action="store_true")
    args = p.parse_args()

    if args.list_categories:
        manifests = load_manifest(args.lang)
        cats = sorted(set(e["category"] for e in manifests))
        print(json.dumps(cats, indent=2) if args.json else "\n".join(cats))
    elif args.query:
        results = search(args.query, args.lang, args.limit)
        if args.json:
            print(json.dumps(results, indent=2))
        else:
            print(format_output(results, args.lang))
    else:
        manifests = load_manifest(args.lang)
        print(f"{len(manifests)} entries in {args.lang} manifest")
        cats = sorted(set(e["category"] for e in manifests))
        for cat in cats:
            entries = [e for e in manifests if e["category"] == cat]
            use_cases = [e["use_case"] for e in entries]
            print(f"  {cat}: {', '.join(use_cases)}")
