# b00t + Ansible via TRIZ Synergies

This memo captures how TRIZ (Theory of Inventive Problem Solving) principles drive deeper b00t + Ansible integration.

### 1. Principle 1 + 13 – Segmentation / Other Way Round
Split installation strategy between b00t discovery (`stack install …`, dependency resolver) and Ansible execution (`b00t stack ansible --run …`). Only stacks that list `ansible.stack` trigger a playbook; lightweight stacks stay CLI-only.

### 2. Principle 10 – Prior Action
Let datums publish `extra_vars`, env hooks, and `skill` metadata before running Ansible. The stack runner already injects those values via `b00t stack ansible --run datum <name> -- key=value…`, so each playbook can assume prerequisites are satisfied.

### 3. Principle 24 – Intermediary
`scripts/ansible-datum-status.py` acts as a subagent for the conversion queue: run `next`, then convert that datum. Mark it `Done` once a playbook exists and the stack dependency is declared. The doc queue (docs/ANSIBLE_DATUM_QUEUE.md) becomes the shared “intermediary” workspace.

### 4. Principle 35 – Parameter Changes
Use the new `run_stack_ansible` helper to forward parameters from `b00t stack ansible --run … -- key=value` into playbooks. Combine this with `scripts/rustfs-skill.sh <skill> start` so each skill context (Matlab, Python, etc.) is passed to the same playbook with different vars, providing flexible configuration without copy/paste.

### 5. Principle 18 – Mechanical Vibration (Oscillation)
Use the queue/script + `just` tasks to iterate conversions regularly. Each demo stack (e.g., `ansible-demo`) and command (`b00t stack ansible --run script …`) forms a “pulse” you can reuse; let the queue remind you which datum to revisit next.

### Actionable Template
1. Choose a datum (run `scripts/ansible-datum-status.py next`).
2. Write a playbook in `ansible/playbooks/`.
3. Point the datum’s `[b00t.ansible]` section to the playbook and document usage.
4. Update the queue/documentation.
5. Run `b00t stack ansible --run datum <name>` (or `stack install <pattern>`) as a TRIZ-aligned demo.

Repeat with the remaining table entries.
