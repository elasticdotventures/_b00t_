
# Brians dotfiles

```
apt install stow

gh repo clone elasticdotventures/dotfiles ~/.dotfiles

```

## usage

```
# stow -d ~/.dotfiles ~ <package>
stow -d ~/.dotfiles -t ~ bash
```

## to update files
```
# stow --adopt .
stow --adopt -d ~/.dotfiles -t ~ bash
```

## todo
* https://github.com/webpro/awesome-dotfiles
* https://bbarrows.com/posts/kubernetes-aliases-functions
* https://krew.sigs.k8s.io/plugins/

# Start ssh-agent if not running
if ! pgrep -u "$USER" ssh-agent > /dev/null; then
    eval "$(ssh-agent -s)"
fi
ssh-add ~/.ssh/id_rsa
ssh-add ~/.ssh/id_ed25519
ssh-add -l

```
ssh -A
```

nano ~/.ssh/config
Host remote_host_alias
    HostName remote_host
    User username
    ForwardAgent yes

