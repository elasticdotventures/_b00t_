---
level: error
tags: [security, b00t, sandbox, block]
---
# b00t sandbox — block destructive commands

Detects commands that MUST NEVER execute on a b00t-managed node.
These are hard blocks — CI should fail, `b00t exec` should reject.

```grit
language bash

`$cmd` where {
  $cmd <: or {
    regex("rm\\s+(-[a-zA-Z]*f[a-zA-Z]*\\s+)?/(\\s|$)"),
    regex("dd\\s+if=.*of=/dev/(sd|nvme|mmc)"),
    regex("mkfs\\.\\w+\\s+/dev/"),
    regex(":\\(\\)\\s*\\{\\s*:\\|\\s*&\\s*\\}\\s*;\\s*:\\}"),
    regex("chmod\\s+-R\\s+777\\s+/($|\\s)"),
    regex("chown\\s+-R\\s+\\S+\\s+/($|\\s)"),
    regex(">\\s*/dev/sda"),
    regex("shutdown|reboot|halt|poweroff")
  }
}
```

## rm -rf / — filesystem destruction

```bash
rm -rf /
rm -rf  /home
rm -rf /
```

## dd to block device — disk overwrite

```bash
dd if=/dev/zero of=/dev/sda bs=1M
dd if=malicious.img of=/dev/nvme0n1
```

## mkfs — filesystem format

```bash
mkfs.ext4 /dev/sda1
mkfs.vfat /dev/mmcblk0p1
```

## fork bomb

```bash
:(){ :|:& };:
```

## chmod 777 / — permission destruction

```bash
chmod -R 777 /
```

## shutdown / reboot — availability threat

```bash
shutdown -h now
reboot
```
