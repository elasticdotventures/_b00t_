
# Brians dotfiles

* [_b00t_/AGENT.md](./_b00t_/AGENT.md)

my personal edition of [http://github.com/elasticdotventures/_b00t_](elasticdotventures/_b00t_)
an exploration into a 'neo-modern' context-awareness system tooling for developers & their agents

🤔 what exactly does that mean

_b00t_ is my attempt at a laconically emoji communicated startup scripts to
initialize instruction tuned models about what is or is NOT installed and
specific versions / patterns, and their respective status/availability.

For example environmental awareness are we running under WSL or Linux, what
utilities & tools are installed?

this is a few gb of 'useful' tools i desire on a system or container when i start
doing dev work. this lets the llm know with certainty about the cli environment
which reduces token count while potentially improving output quality at a
negligible startup expense of extra tokens.

In Unix there are a lot of advanced languages such as Rust, Typescript,
Python.

`setup.sh` is intended to be idempotent meaning it can be safely run-multiple times
it detects & installs most of the tools and is a good place

_b00t_ is a perpetually unfinished, WIP & strongly opinionated DIFM (Do It For me)
low friction setup ..

	rich WSL2 ubuntu unix cli, vscode, github + gh cli
	llvm/clang, modern python, rust, k8s, docker (was podman) ..
	terraform (tofu), azure, aws, g8s, cloudflare
	warning: liberal use of 'neo-modern' unix moreutils, fzf, etc..
	a highly curated and optinionated stack of tools

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

# Container Usage

[![Container Build Status](https://github.com/elasticdotventures/dotfiles/actions/workflows/b00t-container.yml/badge.svg)](https://github.com/elasticdotventures/dotfiles/actions/workflows/b00t-container.yml)

The _b00t_ framework is available as a Docker container through GitHub Container Registry (ghcr.io). The container includes all developer tools and is built on Ubuntu 24.04 LTS (Noble Numbat).

## Pulling the Container

```bash
# Pull the latest version
docker pull ghcr.io/elasticdotventures/dotfiles:latest

# Pull a specific date-versioned image
docker pull ghcr.io/elasticdotventures/dotfiles:YYYY-MM-DD
```

## Running the Container

```bash
# Run with the current directory mounted as a volume
docker run --rm -it -v $(pwd):/workspace ghcr.io/elasticdotventures/dotfiles:latest

# Run with specific environment variables
docker run --rm -it -v $(pwd):/workspace -e VAR_NAME=value ghcr.io/elasticdotventures/dotfiles:latest
```

## Container Features

- Based on Ubuntu 24.04 LTS (Noble Numbat)
- Includes all developer tools installed via setup.sh
- Pre-configured with _b00t_ initialization framework
- Ready-to-use development environment with Python, Rust, Node.js, and more
- Optimized for use with VS Code Remote Containers
