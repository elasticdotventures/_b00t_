#!/usr/bin/env python3
"""Convert blessed.rs crates.json to b00t blessed TOML format."""
import json, sys, os

data = json.load(sys.stdin)
lines = ['# Blessed Rust crates — sourced from blessed.rs (nicoburns/blessed-rs)']
lines.append('# Auto-generated. Regenerate: just blessed-sync')
lines.append('')

total = 0
for group in data['crate_groups']:
    for sub in group.get('subgroups', []):
        for purpose in sub.get('purposes', []):
            name = purpose['name']
            recs = purpose.get('recommendations', [])
            if not recs:
                continue
            total += len(recs)
            lines.append('[[crate]]')
            lines.append(f'category = "{group["name"]}/{sub["name"]}"')
            lines.append(f'use_case = "{name}"')
            crates = [r['name'] for r in recs]
            lines.append(f'recommended = {json.dumps(crates)}')
            notes = {r['name']: r.get('notes', '') for r in recs}
            lines.append(f'notes = {json.dumps(notes)}')
            lines.append('')

out = os.path.expanduser('~/.dotfiles/_b00t_/blessed/rust.toml')
os.makedirs(os.path.dirname(out), exist_ok=True)
with open(out, 'w') as f:
    f.write('\n'.join(lines))

print(f'{total} crate references → {out}')
