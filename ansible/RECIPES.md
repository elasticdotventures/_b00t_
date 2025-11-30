# Ansible Playbook Recipes

Each recipe here maps an `_b00t_` datum to an Ansible playbook. Add a row per new playbook so
`b00t stack ansible` becomes a dependable list of available automation.

| Playbook | Datum(s) | Inventory | Purpose | Notes |
|----------|----------|-----------|---------|-------|
| `ansible/playbooks/k0s_kata.yaml` | `k0s-kata.stack` | `~/.config/b00t/k0s-inventory.yaml` | Install k0s controller/workers with Kata runtime | Targets Debian+ nodes; requires passwordless sudo. |

## Adding new recipes
1. Create the playbook under `ansible/playbooks/` and keep roles in `ansible/roles/`.
2. Update the recorder table above with the datum(s) that now declare the `[ansible]` block.
3. Mention any extra vars or inventory requirements so operators can run `b00t stack ansible` knowing what to supply.
