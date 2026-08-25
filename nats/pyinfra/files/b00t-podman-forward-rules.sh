#!/usr/bin/env bash
# Idempotent: ensures podman's bridge (podman0) has FORWARD-chain ACCEPT rules.
# k0s's kube-router rebuilds the FORWARD chain's policy/ruleset on every
# k0scontroller start (including boot), and its default-DROP policy has no
# rule for podman0 — without this, any podman container's published port
# (host:PORT -> container-ip:PORT) silently fails with "No route to host"
# even though the container itself is healthy and listening.
set -euo pipefail
iptables -C FORWARD -o podman0 -j ACCEPT -m comment --comment 'allow inbound traffic to podman containers (b00t-nats, b00t-forge-kv)' 2>/dev/null \
  || iptables -I FORWARD 5 -o podman0 -j ACCEPT -m comment --comment 'allow inbound traffic to podman containers (b00t-nats, b00t-forge-kv)'
iptables -C FORWARD -i podman0 -j ACCEPT -m comment --comment 'allow outbound traffic from podman containers' 2>/dev/null \
  || iptables -I FORWARD 6 -i podman0 -j ACCEPT -m comment --comment 'allow outbound traffic from podman containers'
