# Ansible Demo Stack

This stack demonstrates how to use the new `b00t stack ansible` runner:

1. `b00t stack install ansible-demo` resolves the stack and ensures `ansible.stack` is available.
2. `b00t stack ansible --run script ansible/playbooks/demo-install.yaml -- skill=demo` executes the playbook which:
   * Creates `~/rustfs-demo`
   * Installs `python3`/`curl` via the host package manager
   * Renders and executes a small `hello.sh` showing the skill context
3. Agents can use `scripts/rustfs-skill.sh demo start` afterward to expose `skills/demo` via RustFS for their quarantined view.

Use this as a template to convert other datums: keep the stack small, state the dependency on `ansible.stack`, and document the command in `docs/ANSIBLE_CONVERSION_PROCESS.md` so the hive knows how to run the demo before scaling it.
