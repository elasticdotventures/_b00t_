---
yq not sed for YAML: yq -i preserves comments/quoting and is multi-doc aware (select(.kind=="Deployment") before pathing); sed breaks on indent drift and doc boundaries; kubectl patch mutates cluster not manifest (drift) — yq the file then apply
