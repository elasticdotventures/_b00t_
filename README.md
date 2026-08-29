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

This is the **canonical, zero-to-hero installer** — it's the only supported path and the
one to link people to. It downloads the release binary for your platform (Linux
x86_64/aarch64/armv7, macOS Intel/Apple Silicon) and verifies its SHA256 checksum; if no
matching release asset exists (unsupported platform, GitHub unreachable), it falls back to
installing `rustup` and building from source automatically — no manual intervention either
way. It also lays out `~/.b00t/_b00t_` (the datum compendium) and exports `_B00T_Path` in
your shell rc — **this step is not optional**, `b00t` is non-functional without it (see
Troubleshooting below).

After it finishes, `source ~/.bashrc` (or open a new terminal) — `curl | sh` runs in a
subshell, so the current one doesn't have the updated `PATH`/`_B00T_Path` yet.

**Developer / full-hive install** (clone the repo, get the capability-aware installer that
also wires up a systemd/quadlet/launchd/k8s service depending on what your machine has):

```bash
git clone https://github.com/elasticdotventures/_b00t_.git && cd _b00t_
./scripts/install-b00t.sh              # auto-detects best service mode, prompts if ambiguous
./scripts/install-b00t.sh --mode k8s   # or force one: k8s | quadlet | systemd-user | systemd-sys | launchd | binaries
```

⚠️ **Don't `cargo install b00t-cli` on its own.** It builds the binary but ships none of the
`_b00t_` datums the binary needs to do anything (`b00t whoami`, `b00t learn`, etc. will fail)
— use one of the two installers above, which always pair the binary with its datums.

### 🔄 Update

```bash
b00t version check          # compare installed vs. latest release
b00t version upgrade -y     # re-runs the installer above (release binary, or source-build fallback)
```

`b00t version upgrade` shells out to the same `install.sh` — one script, one code path, for
both first install and every upgrade after it. Inside a repo checkout it also offers
`--strategy=workspace-build` (plain `cargo install --path` from your local tree) or
`--strategy=workspace-sync` for iterating on b00t itself.

### 🩺 Troubleshooting

**`b00t whoami` / `b00t learn` fail or come back empty** — `_B00T_Path` isn't set, or points
somewhere with no datums. Check `echo $_B00T_Path` (should be `~/.b00t/_b00t_` after the
installer); if empty, `source ~/.bashrc` or re-run the installer. This is the single most
common way to end up with a b00t binary that looks installed but does nothing.

---

## 🪟 Windows desktop and browser extension

**Public Windows distribution is being prepared.** There is currently no published
`l3dg3rr` Winget package and no Microsoft Edge Add-ons listing for `b00t-browser-ext`.
Do not use a guessed Winget package identifier or install an extension from an
untrusted ZIP.

Until release artifacts are published, build the desktop installer locally:

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

Release artifacts will be available from [GitHub Releases](https://github.com/elasticdotventures/_b00t_/releases).
The desktop installer will then be submitted to Winget; the extension will be submitted
to Microsoft Edge Add-ons. These are separate channels: Winget does not install browser
extensions.

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
