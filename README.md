# 🥾 b00t — Agentic Hive OS

[![Release](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml/badge.svg)](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/b00t-cli.svg)](https://crates.io/crates/b00t-cli)

> **"Tell me what I'm running on, what tools are available, what I'm allowed to do, what goals I should optimize for, and where the boundaries are."**

**b00t** is a loop harness for agents. It bestows a fresh model with typed context, a
compendium of skills, formal verification surfaces, and a playbook of low-cognitive-cost
deterministic commands — so the expensive tokens go to *judgment*, not rediscovery.

Design stance, MBSE-style: every capability is a **typed artifact** (a datum) with a schema,
a lifecycle, a verifier, and an evidence trail. The model proposes; the type system,
grammars, and solvers dispose. If the harness can check it, the model never gets to lie
about it.

---

## ⚡ Install

```bash
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/install.sh | bash
```

Downloads the binary from GitHub Releases and verifies its SHA256 checksum. Linux
x86_64/aarch64/armv7, macOS Intel/Apple Silicon.

```bash
cargo install b00t-cli          # from crates.io
# or from source:
git clone https://github.com/elasticdotventures/_b00t_.git && cd _b00t_ && cargo install --path b00t-cli
```

---

## 🪟 Windows desktop and browser extension

**`ledgrrr` desktop (Windows x64)** — MSI and NSIS installers are published on
[GitHub Releases](https://github.com/elasticdotventures/_b00t_/releases/tag/ledgrrr-desktop-v1.9.0):

```powershell
# MSI
curl -LO https://github.com/elasticdotventures/_b00t_/releases/download/ledgrrr-desktop-v1.9.0/ledgrrr_1.9.0_x64_en-US.msi
curl -LO https://github.com/elasticdotventures/_b00t_/releases/download/ledgrrr-desktop-v1.9.0/ledgrrr_1.9.0_x64_en-US.msi.sha256
certutil -hashfile ledgrrr_1.9.0_x64_en-US.msi SHA256   # compare against the .sha256 file

# or NSIS
curl -LO https://github.com/elasticdotventures/_b00t_/releases/download/ledgrrr-desktop-v1.9.0/ledgrrr_1.9.0_x64-setup.exe
curl -LO https://github.com/elasticdotventures/_b00t_/releases/download/ledgrrr-desktop-v1.9.0/ledgrrr_1.9.0_x64-setup.exe.sha256
certutil -hashfile ledgrrr_1.9.0_x64-setup.exe SHA256   # compare against the .sha256 file
```

These are unsigned installer builds — verify the SHA256 before running. A Winget
package (`PromptExecution.ledgrrr`) has been prepared and is pending submission; there is
still no published Microsoft Edge Add-ons listing for `b00t-browser-ext`. Do not use a
guessed Winget package identifier or install an extension from an untrusted ZIP.

To build the desktop installer from source instead (e.g. other architectures):

```powershell
git clone --recurse-submodules https://github.com/elasticdotventures/_b00t_.git
cd _b00t_/vendor/ledgrrr/crates/ledgerr-tauri
cargo install tauri-cli --version "^2" --locked
cargo tauri build --bundles msi,nsis
# Run the MSI in target/release/bundle/msi/ or the NSIS EXE in target/release/bundle/nsis/
```

To build the b00t browser extension for local Microsoft Edge testing:

```powershell
cd _b00t_/b00t-browser-ext
npm ci
npm run package
Expand-Archive build/chrome-mv3-prod.zip build/edge-unpacked
# In Edge: edge://extensions → enable Developer mode → Load unpacked → build/edge-unpacked
```

The desktop installer is submitted to Winget separately from the browser extension, which
is submitted to Microsoft Edge Add-ons — these are separate channels: Winget does not
install browser extensions.

---

## 🧠 The agent boot sequence

A fresh agent runs four deterministic commands and is operational:

```bash
b00t whoami                        # identity, role, session context, boundaries
b00t blessing --manifest --role=X  # prerequisite graph → tool authorization manifest
b00t learn <skill>                 # load exactly the blessings the task needs (context is finite)
b00t task next                     # what to do; b00t task done <id> when verified
```

Learning a skill datum unlocks the tools in its `unlocks` field. **No learning = no auth.**
Skills discover by concept, not name — `b00t learn "constrained decoding"` finds
`grammar-verify` via DWIW fanout (datum search + ontology graph adjacency, weighted).

---

## 🧬 The type system is the operating system

Every tool, model, skill, role, gate, and lesson is a **datum**: TOML dialects
(`.toml` < `.tomllm` < `.tomllmd`, rank-shadowed by key) with schema stanzas and a
machine-readable tail-map. 22+ `DatumType` variants (cli, mcp, skill, ai, k8s, verifier…).

```bash
b00t ontology sparql --subject <X> --predicate all   # walk the triple graph
b00t learn chalk-interner                            # DatumStore ⇄ Chalk Interner mapping
just validate-datums                                 # CI gate over the whole datum tree
```

**Datums are a language.** `b00t-lsp` (tower-lsp over `b00t-datum-core`) gives editors and
agents diagnostics (parse spans, tail-map contract, rank shadowing, unknown types), hover,
and cross-datum references (`depends_on`, `composes_with`). Tier-1 taplo support ships
JSON Schemas generated from the same constants — schema and diagnostics cannot drift.
Registered in serena's solidlsp, so symbolic code tools work *on the datum graph itself*.

---

## 🔬 Verification: LLM proposes, grammar constrains, Z3 disposes

Structural hallucination is not discouraged — it is made **unrepresentable**.

```bash
b00t learn grammar-verify                      # the full pattern, with recipes
echo '(assert (and (> x 0) (< x 0)))(check-sat)' | z3 -smt2 -in   # → unsat
```

- **Decode-time constraints**: GBNF grammars (`_b00t_/b00t-verify.gbnf`) force claim-shaped
  output through a verify call; JSON-Schema constraints derive from the consuming Rust type
  (`schemars`) and stamp per-backend dialects (vLLM `guided_json`, llama-server
  `json_schema`, OpenAI `response_format`) via one abstraction.
- **Runtime verification**: the `verify` MCP tool routes SMT2 through Z3 (~50ms round-trip);
  b00t-mcp's LLM proxy executes model-emitted verify tool-calls and **audits** grammar-shaped
  claims — a hallucinated `sat` gets rewritten to the real verdict before the client sees it.
- **Gates + evidence**: `_b00t_/gates/*.gate.toml` validate on commit; every PASS appends to
  the evidence log; PASS evidence converts to verified training examples
  (`just ai-finetune::evidence-train`).

---

## 🧭 Cognitive tiers & the loop

Route work by complexity; compress ruthlessly at every boundary:

| Tier | Models | Work | Output contract |
|---|---|---|---|
| `sm0l` | qwen3.6-A3B, haiku | tests, lint, classify, route | `PASS` or `FAIL: <5-line excerpt>` |
| `ch0nky` | qwen3.6 local (vLLM/llamacpp) | implement, refactor, debug | diff + test result |
| `frontier` | claude opus/sonnet | architecture, novel design | structured decision |

```bash
b00t-loop -n 10                    # ralph-style iteration loop
b00t ooda run                      # typed Observe→Orient→Decide→Act pipeline nodes
b00t hive activate=<profile>       # resource-gated system state (GPU exclusion groups)
```

Knowledge flows both directions: `b00t lfmf <tool> "<lesson>"` memoizes tribal knowledge
(salvage-first: malformed input degrades to a tagged lesson, never bail-and-discard;
`b00t lfmf stats all` reports hit/salvage/miss telemetry). Lessons are **endurants** —
temporal bugs go to `b00t task add "bug: ..."` instead.

---

## 🔎 Semantic code ops (serena, c0re)

Symbol-scoped reading and patching via LSP — measured **83–96% context savings** vs
whole-file reads on this repo. Packaging ladder: **k8s > podman > host binary > uvx**
(encapsulation bounds the reasoning surface).

```bash
podman build -t serena:latest vendor/serena/     # Containerfile — auditable surface
kubectl apply -f _b00t_/k8s/serena.yaml          # b00t-serena namespace, live pod
b00t mcp install serena claudecode               # datum → registry → client config
scripts/serena-smoke.sh <launch-cmd...>          # same handshake drives every rung
```

`find_symbol`, `find_referencing_symbols`, `replace_symbol_body`, `insert_after_symbol` —
edits address the symbol graph, not line numbers, so they survive file drift.

---

## 🤖 MCP integration

```bash
# b00t-mcp: compile-time generated tools + exec/discover proxy
claude mcp add b00t -- b00t-mcp --stdio
b00t_discover("<keyword>")         # find the command   → b00t_exec("task list") runs it
b00t_mcp_stack_load("serena")      # dynamic capability extension at runtime

# OpenAI-compatible LLM gateway (b00t-server): soul-registry upstream discovery,
# 🎂 cake budget hard gate, spotlight usage telemetry, agentic verify loop
b00t-mcp --llm -p 3000
```

Assimilate the outside world: `b00t grok assimilate -t <topic> --source-url <url> --b00tyverse`
distills content to git-blob datums, registers vendor forks, and feeds the ontology.

---

## 🐝 Hive coordination

```bash
b00t agent capability              # announce role + skills
b00t agent discover --role=qa      # find peers
b00t agent message / vote          # A2A messaging + consensus
just compile-agent <role> 3 /tmp/agent.md && claude --agent /tmp/agent.md
```

Sub-agent output contract: compressed summaries only. Raw output never enters executive
context.

---

## 🎯 Mission

The convergence target: a **ch0nky model that speaks native b00t** — every workstream
feeds one of three legs: training signal (evidence→train, transcript harvest),
decode-time constraint (grammars/schemas), or tool surface (verify, serena, b00t-lsp).
Don't train the model *or* evolve the harness — do both, with one shape.

---

## 🌐 Platform

Linux x86_64/aarch64/armv7 · macOS Intel/AS · single-node k8s (k0s/k3s) friendly ·
rootless podman (`--userns=keep-id`) · Python via `uv` only.

## 🛠 Development

```bash
cargo build --workspace
cargo test -p b00t-cli --lib       # 927 tests
just -l                            # recipe survey
just validate-datums               # datum tree gate
```

## 📖 Docs

`AGENTS.md` (agent protocol) · `CLAUDE.md` (harness boilerplate) · `_b00t_/` (the datum
compendium — the real documentation) · `b00t learn <anything>` (ask the system itself).
