# pyinfra inventory — b00t-node (Vultr VPS) + hive worker nodes reachable via
# tailnet (sm3lly, fung1). Hive nodes are targeted by tailnet IP, not a
# public one — they have no public IP, and tailnet addresses are stable
# across DHCP/reboots (confirmed via `tailscale status`, 2026-09-11).
#
# vultr1 (207.148.87.195) was destroyed via the Vultr console 2026-08-25;
# see _b00t_/datums/PROVIDER-VULTR.provider.tomllmd for the full record.
# b00t-node is targeted by IP, not an ssh-config alias, because no alias is
# guaranteed to exist on every operator machine this might run from. If you
# have a `b00t-node` alias in ~/.ssh/config, you can override on the CLI:
#   pyinfra --user root b00t-node nats/pyinfra/deploy_b00t_node.py

b00t_node = [
    ("149.28.189.45", {"ssh_user": "root"}),
]

# Hive worker nodes — for per-node systemd services (e.g. a future graph-
# reindex agent, SP5 continuation: docs/superpowers/specs/
# 2026-09-11-sp5-graph-artifact-publish-design.md). ssh_user matches each
# node's tailnet owner (`brianh@` per `tailscale status`).
hive_nodes = [
    ("100.88.206.37", {"ssh_user": "brianh", "hostname": "sm3llsl1k3s0ld3r"}),
    ("100.65.187.40", {"ssh_user": "brianh", "hostname": "fung1"}),
]
