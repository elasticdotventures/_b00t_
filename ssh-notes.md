# Start ssh-agent if not running
```
if ! pgrep -u "$USER" ssh-agent > /dev/null; then
    eval "$(ssh-agent -s)"
fi
ssh-add ~/.ssh/id_rsa
ssh-add ~/.ssh/id_ed25519
ssh-add -l
```

🤓 then use `ssh -A`
Enables forwarding of connections from an authentication agent such as  ssh-agent(1) or use ForwardAgent below.

```
nano ~/.ssh/config
Host remote_host_alias
    HostName remote_host
    User username
    ForwardAgent yes
```

To Test

```bash
ssh -T git@github.com
# Hi elasticdotventures! You've successfully authenticated, but GitHub does not provide shell access.
```

