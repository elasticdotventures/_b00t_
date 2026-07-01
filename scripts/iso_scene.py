"""Generate isometric SVG from mermaid graph text. Called by b00t-admin."""
import sys, math, re

mermaid = sys.argv[1] if len(sys.argv) > 1 else sys.stdin.read()

# Parse nodes: id["label"] or id("label")
nodes = []
for m in re.finditer(r'(\w[\w_-]*)\[(?:")?(.+?)(?:"\]|\])', mermaid):
    nid, label = m.group(1), m.group(2).replace('\\n',' ')[:30]
    nodes.append((nid, label))

# Parse edges: from --> to or from -->|label| to
edges = []
for line in mermaid.split('\n'):
    if '-->' not in line: continue
    parts = line.split('-->')
    if len(parts) < 2: continue
    frm = parts[0].strip()
    to_part = parts[1].strip()
    # Handle |label| format
    if '|' in to_part:
        to = to_part.split('|')[-1].strip()
    else:
        to = to_part.strip()
    edges.append((frm, to))

# Isometric projection
ISO_X = math.cos(math.radians(30))
ISO_Y = math.sin(math.radians(30))

def iso_project(x, z, y, scale, ox, oy):
    sx = ox + (x - z) * scale * ISO_X
    sy = oy + (x + z) * scale * ISO_Y - y * scale
    return sx, sy

scale = 3.0
origin = (400, 300)
spacing = 80

# Layout: grid-based positioning
cols = math.ceil(math.sqrt(len(nodes))) if nodes else 1
positions = {}
for i, (nid, _) in enumerate(nodes):
    col = i % cols
    row = i // cols
    positions[nid] = (col * spacing, row * spacing, 0.0)

# Bounding box
min_sx = min_sy = float('inf')
max_sx = max_sy = float('-inf')
for nid, _ in nodes:
    x, z, y = positions.get(nid, (0, 0, 0))
    sx, sy = iso_project(x, z, y, scale, *origin)
    min_sx = min(min_sx, sx)
    min_sy = min(min_sy, sy)
    max_sx = max(max_sx, sx)
    max_sy = max(max_sy, sy)

padding = 80
w = int(max_sx - min_sx + 2 * padding)
h = int(max_sy - min_sy + 2 * padding)
ox = -min_sx + padding
oy = -min_sy + padding

# SVG
svg = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">']
svg.append(f'<rect width="100%" height="100%" fill="#0f172a"/>')
svg.append(f'<g font-family="monospace" font-size="11">')

# Grid
for i in range(cols + 1):
    x1, y1 = iso_project(i * spacing, 0, 0, scale, ox, oy)
    x2, y2 = iso_project(i * spacing, cols * spacing, 0, scale, ox, oy)
    svg.append(f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="#1e293b" stroke-width="0.5"/>')
for i in range(cols + 1):
    x1, y1 = iso_project(0, i * spacing, 0, scale, ox, oy)
    x2, y2 = iso_project(cols * spacing, i * spacing, 0, scale, ox, oy)
    svg.append(f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="#1e293b" stroke-width="0.5"/>')

# Edges
colors = ["#3b82f6","#10b981","#f59e0b","#ef4444","#8b5cf6","#ec4899"]
for i, (frm, to) in enumerate(edges):
    if frm not in positions or to not in positions: continue
    fx, fz, fy = positions[frm]
    tx, tz, ty = positions[to]
    sx1, sy1 = iso_project(fx, fz, fy, scale, ox, oy)
    sx2, sy2 = iso_project(tx, tz, ty, scale, ox, oy)
    c = colors[i % len(colors)]
    svg.append(f'<line x1="{sx1:.1f}" y1="{sy1:.1f}" x2="{sx2:.1f}" y2="{sy2:.1f}" stroke="{c}" stroke-width="1" opacity="0.5"/>')

# Nodes
for i, (nid, label) in enumerate(nodes):
    x, z, y = positions.get(nid, (0, 0, 0))
    sx, sy = iso_project(x, z, y, scale, ox, oy)
    c = colors[i % len(colors)]
    short = label[:18]
    svg.append(f'<g transform="translate({sx:.1f},{sy:.1f})">')
    svg.append(f'<rect x="-50" y="-16" width="100" height="32" rx="4" fill="{c}" opacity="0.85" stroke="#e2e8f0" stroke-width="1"/>')
    svg.append(f'<text x="0" y="-2" text-anchor="middle" fill="#fff" font-size="8">{short}</text>')
    svg.append(f'<text x="0" y="10" text-anchor="middle" fill="#94a3b8" font-size="7">{nid}</text>')
    svg.append(f'</g>')

svg.append('</g>')
svg.append('</svg>')
print('\n'.join(svg))
