# Ansible Conversion & Validation Process

This document captures the repeatable steps to convert existing datums to use `ansible` playbooks
instead of inline `install` script logic, while also validating the converted recipes.

## 1. Inventory candidates

1. Use `find _b00t_ -name '*.toml'` or `b00t-cli datum list` to enumerate all datums.
2. Focus on datum files that currently embed shell install/update sections (CLI datums, stacks,
   kubernetes installers, etc.).
3. Record their desired behaviors in `ansible/RECIPES.md` (see the template below) so playbook owners
   can compare implementations.

## 2. Create an ansible recipe per datum

1. Add a playbook under `ansible/playbooks/<name>.yaml` that performs the same steps as the current
   installer script. Prefer reusing existing `ansible/roles`; keep idempotency, sudo usage, and host
   filtering explicit.
2. Document required inventories or variables inside the playbook (or `ansible/RECIPES.md`).
3. Update the datum to drop the inline `install`/`update` blocks and declare:
   ```toml
   [b00t.ansible]
   playbook = "ansible/playbooks/<name>.yaml"
   inventory = "ansible/inventory.sample.yaml"  # if shared
   extra_vars = { version = "<desires>", datum = "<name>" }
   ```
4. If the datum still needs `desires`, keep it for version comparisons,
   but use the `[ansible]` section for execution metadata.

## 3. Link ansible capability via stacks

1. The `_b00t_/ansible.stack.toml` consolidates the runtime (`ansible.cli`) and exposes
   `b00t stack ansible` as the canonical entry point for listing/validating playbooks.
2. Any stack that now depends on an ansible recipe adds `ansible.stack` to its `depends_on` or `members` so
   the orchestrator knows ``ansible`` must be available before running the stack.
3. Example: `k0s-kata.stack.toml` already lists `ansible.cli`; it can also depend on `ansible.stack`
   if centralized validation is desired.

## 4. Validation workflow

1. Ensure `ansible-core` is installed via `b00t install ansible` (uses `_b00t_/ansible.cli.toml`).
2. Run `b00t ansible run <datum> --check` to syntax-check the referenced playbook (the command now
   builds args via `AnsibleConfig` and respects datum variables).
3. You can also invoke playbooks directly from a stack context with `b00t stack ansible --run <script|datum> <name> -- <params>` to leverage the new helper (extra `key=value` entries become vars, everything else is passed through as flags).
3. Execute `b00t ansible run <datum>` against a local inventory or ephemeral container to verify
   correctness; capture logs under `logs/orchestrators/` just like existing orchestrator scripts.
4. Optionally, script the above in `just` or CI to iterate through a batch of datums:
   ```bash
   for datum in k0s-kata ...; do
       b00t ansible run "$datum" --check || exit 1
   done
   ```
5. When converted datums are validated, remove their inline scripts and rely entirely on the new
   Ansible playbooks.

## 5. Maintaining the recipe list

`ansible/RECIPES.md` tracks which playbooks exist and which datums reference them. Every new recipe
entry should include:

- Playbook path
- Target datum(s)
- Required inventory or vars
- Manual steps (if interactive)

This makes `b00t stack ansible` the authoritative list for hive operators.
