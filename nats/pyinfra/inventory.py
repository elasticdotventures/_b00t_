# pyinfra inventory — b00t-node, the sole surviving Vultr VPS.
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
