
# Brians dotfiles

🤩 gee, umm.. Thanks for the stars!?

this is my personal edition of http://github.com/elasticdotventures/_b00t_
an exploration into a 'neo-modern' system tooling

🤔 what exactly does that mean

_b00t_ is my attempt at a laconically emoji communicated startup scripts to initialize instruction tuned models about what is or is NOT installed and specific versions, and their respective status/availability.

it's a few gb of 'useful' tools i desire on a system or container when i start doing dev work. this lets the llm know with certainty about the cli environment which reduces token count while potentially improving output quality at a negligible startup expense of extra tokens.

`setup.sh` is intended to be idempotent meaning it can be safely run-multiple times
it detects & installs most of the tools and is a good place

_b00t_ is a perpetually unfinished, WIP & strongly opinionated DIFM (Do It For me) low friction setup ..

	rich WSL2 ubuntu unix cli, vscode, github + gh cli
	llvm/clang, modern python, rust, k8s, docker (was podman) ..
	terraform (tofu), azure, aws, g8s, cloudflare
	warning: liberal use of 'neo-modern' unix moreutils, fzf, etc..

you could use this repo as a template and pull in my changes as you wish,
if curious feel free to open issues for chat & q/a, but consider this is
mostly intended to create a 'nuclear-powered-batteries included' for a future micro-legion of AI minions

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
* https://olivernguyen.io/w/direnv.run/

gh issue create "subject"
gh issue develop # --checkout



# # Add the container as a submodule
git submodule add https://github.com/simonhyll/devcontainer .devcontainer
