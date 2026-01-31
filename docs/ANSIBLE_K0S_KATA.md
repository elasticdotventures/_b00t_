# Ansible Provisioning for k0s + Kata

This guide explains how to bring a new edge node under hive management using the bundled Ansible playbook. The playbook installs k0s, configures containerd with the Kata runtime shim, and applies a `RuntimeClass` so workloads can target Kata VMs.

## Requirements

- Control host (WSL or Linux) with `ansible-core` 2.16+.
- Target nodes running Ubuntu 22.04+ (root SSH access, passwordless sudo recommended).
- LVM thin-pool device to dedicate to Kata (defaults to `/dev/mapper/data`).

## Files

- `ansible/inventory.sample.yaml` – copy and adjust for your hosts.
- `ansible/playbooks/k0s_kata.yaml` – entry point.
- `ansible/roles/k0s_kata` – tasks + templates.

## Usage

```bash
cp ansible/inventory.sample.yaml ~/.config/b00t/k0s-inventory.yaml
$EDITOR ~/.config/b00t/k0s-inventory.yaml
just orchestrator-k0s-kata MODE=start INVENTORY=~/.config/b00t/k0s-inventory.yaml        # start/apply
just orchestrator-k0s-kata MODE=stop INVENTORY=~/.config/b00t/k0s-inventory.yaml    # stop/reset
```

The start recipe performs:

1. Installs Kata packages + devmapper thin-pool tooling.
2. Runs the official `get.k0s.sh` installer, renders `/etc/k0s/k0s.yaml` with Kata containerd patches, and installs the controller service.
3. Labels the node and applies a `RuntimeClass` named `kata` (override via `kata_runtime_class` inventory var).

To fully stop/reset a node (stop recipe):

1. Runs `k0s stop` (optionally `k0s reset --force --config ...` if you set `k0s_reset_force=true`).
2. Removes the Kata runtime class label + CRD.
3. Disables k0s services so the node no longer participates.

Set `EXTRA_ARGS="-e k0s_reset_force=true"` when invoking `just orchestrator-k0s-kata MODE=stop` if you truly need to nuke the node’s k0s state; otherwise the playbook leaves other workloads intact.

Every orchestrator invocation writes to `logs/orchestrators/k0s-kata-<mode>-<timestamp>.log` so you can review provisioning output after ansible exits.

After either playbook finishes, confirm:

```bash
k0s status
k0s kubectl get nodes -o wide
k0s kubectl get runtimeclass
```

Target nodes should display `katacontainers.sh/installed=true` and the `kata` runtime class.
