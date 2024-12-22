
# Brians dotfiles

🤩 gee, umm.. Thanks for the stars!?

this is my personal version of http://github.com/elasticdotventures/_b00t_
my exploration into a 'neo-modern' system tooling, it's a few gb of 'useful'
(and i'll perhaps update it someday when i have time / fully retire)

a strongly opinionated DIFM setup ..

	rich WSL2 ubuntu unix cli, vscode, github + gh cli
	llvm/clang, modern python, rust, k8s, docker (was podman) .. 
	terraform (tofu), azure, aws, g8s, cloudflare
	warning: liberal use of 'neo-modern' unix moreutils, fzf, etc..

you can use this repo as a template and pull in my changes as you wish, 
if curious feel free to open issues for chat & q/a, but consider this is
mostly intended to create a 'nuclear-powered-batteries included'  
for my micro-legion of AI-pairs

# New System: 
see [setup.sh] for minimal bootstrap then `just install`  
(fwiw install *should* also safely upgrade)


# Existing/Update system:

```
apt install stow

gh repo clone elasticdotventures/dotfiles ~/.dotfiles
# or 
gh repo clone elasticdotventures/dotfiles ~/.dotfiles -- --depth 1

just install

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
