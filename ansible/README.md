# b00t Ansible Support

This directory contains Ansible playbooks that provision hive infrastructure targets. The initial focus is bringing up k0s nodes with the Kata shim so edge ARM/NPU machines can join the hive cluster without relying on Rancher k3s.

## Layout

- `inventory.sample.yaml` – starter inventory showing how to describe k0s hosts.
- `playbooks/k0s_kata.yaml` – entry point playbook that calls the `k0s_kata` role.
- `roles/k0s_kata` – installs OS prerequisites, k0s, and Kata runtime classes.

## Usage

1. Copy `inventory.sample.yaml` to a secure location, update hostnames/IPs, and export `ANSIBLE_INVENTORY` if desired.
2. Run `just ansible-k0s INVENTORY=/path/to/inventory.yaml` to install k0s + Kata.
3. Use `k0s kubectl get nodes` on the target to confirm it is Ready and exposes the `kata` runtime class.

All tasks expect passwordless sudo or `ansible_become_pass` configured, and target hosts should run a Debian-based OS (Ubuntu 22.04+ recommended).
