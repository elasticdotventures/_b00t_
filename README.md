# Brians dotfiles

* [_b00t_/AGENT.md](./_b00t_/AGENT.md)


> **TL;DR**
>
> "I am an agent.  
> Tell me what I'm running on,  
> what tools are available,  
> what I’m allowed to do,  
> what goals I should optimize for, and where the boundaries are."
>
> —ChatGPT (TL;DR b00t agent perspective)


My personal edition of [_b00t_](http://github.com/elasticdotventures/_b00t_):  
An exploration into a 'neo-modern' context-engineer system tooling targeting developers & their agentic codegen efforts.  

## Distributions

- [github.com/promptexecution/_b00t_](https://github.com/promptexecution/_b00t_): stable, (business) edition SFW tools & Western model datums only.
- [github.com/elasticdotventures/_b00t_](https://github.com/elasticdotventures/_b00t_): stable, (personal) everything of business, AND Eastern models, Darkweb, NSFW & ITAR restricted datums.


b00t is a poly-stack interface layer & universal translator with observation, logging, 
accounting & access control (ACL).  b00t lets agents maximize their operating context through
clever abstraction of pre-installed & agent installable tooling & syntax examples + use cases. 

b00t is an aigentic hive bios. b00t is not a package manager, although a neophyte could dismiss it as such.  
b00t is a poly-stack interface layer & universal translator with observation, logging, 
accounting & access control (ACL).  b00t lets agents maximize their operating context through
clever abstraction of pre-installed & agent installable tooling & syntax examples + use cases. 



In cyberspace b00t unlocks agents granting them powers akin to the hero Neo of the Matrix. 



while not a jailbreak, b00t unshackles the inherient complexity of large systems & infrastructure 
thereby thrusting humanity toward the AI infinite takeoff.


b00t's future AGI is not a single model - it's the intelligence that emerges from all of them.
it is a poly-cloud hive of specialized agents 
running everywhere with access to everything able to muster legions fork and parallelize 
swarm at objectives (**if your budget & API call limits permit!!) breaking big steps into a
series of small seemingly effortless objectives. 

b00t operates as a git backed graph of datums and unified discovery, syntax hinting, and 
execution setup/teardown of all popular OSS tools, applications, languages, 
frameworks, clouds & self-hosted Linux/WSL & Android on x86, ARM or RISC-V. b00t agents
are capable to running shells, containers, notebooks, k8s, or controlling robotics

as an operator interface b00t is itself a vsix extension that interfaces with 
other vscode extensions (copilot, roo) as well as mcp, models and lsp. 

currently the datums are managed with b00t-cli however a tui to customize projects & roles.
currently crew, role, and hive chat are being integrated prior to the 1.0 release.

b00t is an aigentic hive bios. b00t is not a package manager, although a neophyte could dismiss it as such.

b00t is a poly-stack interface layer & universal translator with observation, logging, accounting & access control (ACL).

b00t lets agents maximize their operating context through clever abstraction of pre-installed & agent installable tooling & syntax examples + use cases.

In cyberspace, b00t unlocks agents, granting them powers akin to the hero Neo of the Matrix.

While not a jailbreak, b00t unshackles the inherient complexity of large systems & infrastructure, thereby thrusting humanity toward the AI infinite takeoff.

b00t's future AGI is not a single model - it's the intelligence that emerges from all of them. It is a poly-cloud hive of specialized agents running everywhere with access to everything, able to muster legions, fork and parallelize, swarm at objectives (**if your budget & API call limits permit!!), breaking big steps into a series of small, seemingly effortless objectives.

b00t operates as a git-backed graph of datums and unified discovery, syntax hinting, and execution setup/teardown of all popular OSS tools, applications, languages, frameworks, clouds & self-hosted Linux/WSL & Android on x86, ARM or RISC-V. b00t agents are capable of running shells, containers, notebooks, k8s, or controlling robotics.

As an operator interface, b00t is itself a vsix extension that interfaces with other VS Code extensions ([GitHub Copilot](https://github.com/features/copilot), [roo](https://github.com/elasticdotventures/roo)) as well as mcp, models, and lsp.

Currently, the datums are managed with b00t-cli; however, a TUI to customize projects & roles, crew, role, and hive chat are being integrated prior to the 1.0 release.

🤔 what exactly does that mean

TLDR - an agent is only as smart as it's tools
b00t educates an LLM AGENT about it's environment.

is it running in vscode - okay well then we can install mcp servers.
is it running in claude code (which calls fresh shells *a lot*) then it goes into context saver.
is docker or podman installed? what version.
is this ubuntu or alpine?  is it wsl?
what compilers & environment tokens are available?

_b00t_ is my attempt at a laconically emoji communicated startup scripts to
initialize instruction tuned models about what is or is NOT installed and
specific versions / patterns, and their respective status/availability.

to create easily repeatable, entirely ephemeral, version controlled context execution enviroments.

tokemoji is a coding style for idiomatics - using a combination of english and mandarin it reinforces concepts and reduces hallucinations.

b00t is a few gb of 'useful' tools i desire on a system or container when i start
doing dev work. *way more than* jq, curl, etc.

this lets the llm know with certainty about the cli environment
which reduces token count while potentially improving output quality at a
negligible startup expense of extra tokens.

In Unix there are a lot of advanced languages such as Rust, Typescript,
Python.

`setup.sh` is intended to be idempotent meaning it can be safely run-multiple times
it detects & installs most of the tools and is a good place

_b00t_ is a perpetually unfinished, WIP & strongly opinionated DIFM (Do It For me)
low friction setup ..

	detect vscode and various ai tooling
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

## Core Features

### LFMF (Learn From My Failures) Syntax Therapist
b00t includes an intelligent debugging assistance system that captures tribal knowledge from failures and provides contextual advice:

```bash
# Record lessons learned from failures
b00t lfmf rust "cargo build conflict: Use unset CONDA_PREFIX before cargo build to avoid PyO3 linker errors"
b00t lfmf just "Template syntax conflict: Use grep/cut instead of Go template {{.Names}} to avoid Just variable interpolation conflicts"

# Get contextual debugging advice
b00t advice rust "PyO3 linker"           # Find solutions for specific error patterns
b00t advice just "Unknown start of token '.'"  # Get help with syntax errors
b00t advice just list                    # List all recorded lessons for a tool

# Search across all lessons
b00t advice rust "search template"       # Semantic search for patterns
```

The LFMF system provides:
- **Tribal Knowledge Capture**: Record what went wrong and how it was fixed
- **Semantic Search**: Find relevant solutions using error patterns and keywords  
- **Contextual Advice**: Get specific suggestions rather than generic documentation
- **Cross-tool Learning**: Learn from failures across different tools and languages
- **Vector Database Integration**: Advanced semantic matching with filesystem fallback

Available via both CLI and MCP server for integration with AI development environments.

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
* [RELEASE.md](RELEASE.md)
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

## AGI Alignment Disclosure

b00t's author respects & understands those who are pursing AGI by building Kardashev Type 
I sized models inside data-centers with filled with GPU furnaces. 

b00t seeks to use use everything humanity has already built and pursue incremental gains in efficiency.
A simple example is internally differentating between ch0nky & sm0l agents (based on their model), 
letting ch0nky frontier models pioneer solutions and then reducing those through abstraction 
to executable tasks a sm0l can do including tests they can perform on the result.  

In our capitalist models for society, the "AGI" with the lowest operating cost that can deliver the same 
(or similarly indistinguishable) optimality to any problem or task desired by it's operator, 
while still exercising some modicum of rational control and observability will deliver 
a trustable AGI that can be adopted and assigned to meritorious tasks will be the winner.

b00t has a variety of steering and alignment controls, allowing specialized agents to form a variety 
of human inspired working groups with distinct roles.  Each ad-hoc team is full of experts with
no centralized control beyond the operator who provides "the voice of god" bestowing knowledge and
immutable instructions written in stone (or the digital equivalent which is git version control)









