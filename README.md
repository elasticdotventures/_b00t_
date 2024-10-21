
# Brians dotfiles

```
apt install stow

gh repo clone elasticdotventures/dotfiles ~/.dotfiles
# or 
gh repo clone elasticdotventures/dotfiles ~/.dotfiles -- --depth 1

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

just install
```

## more:
* [setup.sh](setup.sh)
* [ssh notes](ssh-notes.md)
* [git notes](git-notes.md)


# stupid wsl tricks:

explorer.exe $(wslpath -w ./rendered_pdfs/)

## todo
* https://github.com/webpro/awesome-dotfiles
* https://bbarrows.com/posts/kubernetes-aliases-functions
* https://krew.sigs.k8s.io/plugins/
* https://github.com/xero/dotfiles
