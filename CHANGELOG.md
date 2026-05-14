# Changelog

All notable changes to this project will be documented in this file.

## [unreleased]

### 🚀 Features

- Gate MCP registration, observability, telemetry merge, Arbor bridge (#374)
- *(gates)* Reusable rhai gate module, hook_detect wiring, merge fixes
- *(gates)* Add b00t gates list subcommand
- *(observability)* Events + guards subcommands with --follow
- Add ++abstract.agent metatype — universal base class for all hive agents
- Implement bouncer pattern gatekeeper for b00t hive agents
- Integrate bouncer pattern into ++abstract.agent and all agent datums
- A2A protocol SDK, governance epoch 2-8, hive CMDB, autotask mode
- Maximize OODA loop with state machine, guard rails, handshake, autoresearch
- #309 — additional agents/crew roles
- H3rmes integration, gh issue triage (7 resolved), new _b00t_ datums (#387)
- #312 — android dev skill, role sync, knowledge store backend (#408)
- Telemetry pipeline unification + wrkflw skill (#411)

### 🐛 Bug Fixes

- *(gates)* Fix obvious issues - status in JSON, shadowing, .tomllm scan, corrupted jsonl
- *(observability)* Tail follow, unused cutoff, remove test artifact
- Resolve duplicate enum defs, missing args, broken submodule
- Update stale l3dg3rr→ledgrrr docs URL and rename ledgerr→ledgrrr in code
- Complete ledgerr→ledgrrr rename (env vars, strings, binary name, config files)
- Add codebase_memory underscore alias to GrokBackend::from_flag
- Resolve all remaining conflict markers across 5 files
- Close 4 gap-analysis issues
- Delegate_task missing approval param (from #65 gate impl)
- Apply all code review feedback from PR #382 review
- Add explicit precondition assertions to test_spend_decreases_balance_and_supply
- Enforce CakeTransaction amount invariants in constructors
- Github.mcp.toml bootstrap path (_b00t_/mcp/ → _b00t_/)
- WOW candle/ledgrrr integrity, CRIT-3/MAJ-1 fixes (#415)

### 💼 Other

- Origin/main into feat/agentic-docs-loop-LappyX23, fix CodebaseMemory duplicate in GrokBackend
- Origin/main into feat/observability-events-LappyX23

### ⚙️ Miscellaneous Tasks

- Update l3dg3rr upstream to 1d8588f
- Batch-2 gap verification — k0mmand3r, redis, MCP, skills, rust

## [0.8.0] - 2026-05-06

### 🚀 Features

- Add docker usage documentation (#42)
- Uplift docker and k8s datum types with resource management (#43)
- Create cargo workspace for centralized rust project management (#47)
- Implement kubernetes mcp deployment and cli commands (#48)
- Add session memory with get/set/incr/decr operations and README tracking (#56)
- Centralize version management and fix versioning issues (#68)
- LiteLLM Integration with b00t Datum System (#36) (#75)
- Move session file to .git directory to prevent tracking (#80)
- Grok phase 1 (#84)
- Implement b00t-j0b-py web crawler system for grok phase 3 (#86)
- OAuth 2.1 authorization server for remote MCP deployment (#90)
- Integrate b00t-mcp with dashboard for unified AI model management
- Integrate ACP hive communication into b00t-mcp for multi-agent coordination
- Implement comprehensive MCP TOML schema validation system (#92)
- Add systemd services for SOCKS5 proxy and port mapping (#98)
- B00t uplift (#100)
- Agents as a datum (#101)
- *(mcp)* Add dual-role MCP registry with dependency installation (#104)
- *(grok)* Add URL crawling with crawl4ai integration
- *(bootstrap)* Add self-installation with podman support and binary aliasing
- *(orchestrator)* Implement silent service orchestration for b00t
- *(api-composition)* Implement multi-level API abstraction architecture
- *(jobs)* Implement cleanup logic and Google ADK integration
- *(jobs)* Add multi-provider LLM support to ADK integration
- *(jobs)* Add multi-provider LLM support to ADK integration
- *(jobs)* Replace Google ADK with pydantic-ai for production agents
- Add b00t up command with _b00t_.toml configuration support
- Implement b00t up with _b00t_.toml configuration loading
- *(b00t up)* Fix trait version conflicts by refactoring to lib.rs
- Implement unified datum schema system with TypeScript-Rust bridge
- Implement comprehensive MCP TOML schema validation system
- Add b00t browser extension with CI/CD packaging
- Acp from sm3llyd0s
- Agents as a datum
- Fix compilation errors and add universal install script
- Add agent-focused README with universal installation guide
- *(mcp)* Add dual-role MCP registry with dependency installation
- TOGAF enterprise architecture capability datums (#170)
- Aws cli runbooks and doc archival (#171)
- Rust-based PlantUML Server with TOGAF Integration (#180)
- Laconic cli up output with suppressed errors (#183)
- *(project)* Overhaul documentation and define executive role
- *(aws)* Implement unified AWS tool generator
- *(mcp)* Add bidirectional sync with kiro/claude support (#190)
- Add pm2-mcp MCP server (from PR #189)
- Isolate rustfs skills (#186)
- Add blender b00t stack - initial project foundation (#108)
- Add PRD validation script (#10)
- Add retry logic and streaming for Claude Code (#45)
- Merge upstream Ralph PRs (#10, #45, #64)
- B00t up / tutorial / ontology MVP (#235)
- Local Qwen3-Coder vLLM/podman CDI + soup-of-the-day model router (#251)
- *(soul)* B00t soul serve — HTTP K/V REST API for MoltisMemory_🥾 integration (#254)
- Hive maintenance — ralph loop + codex dispatch (#253)
- Marketplace schema fix, agentsys P1/P2 skills, naming conventions (#252)
- *(soul)* Distill + SoulMemoryWriter + per-workspace soul dirs + serve HTTP API (#257)
- Exec/quit commands, llama-server datum, irontology-mcp submodule, opencode config (#265)
- Azure datums, control-plane service, terraform module, irontology plan (#287)
- *(grok+irontology)* Irontology-mcp integration — issues #260 #261 #262 #263 (#298)
- *(datum)* Search, filter, graph, neighbors, semantic-search (#198 #199 #200 #201) (#299)
- *(datum)* Assimilate qwen-code as b00t hive agent (#303)
- *(skills)* Multi-dir skill resolver + SKILL.md parser + crew handoff (#305)
- *(datum)* Add uninstall + hook_uninstall lifecycle to BootDatum (#314)
- *(grok+irontology)* Dual-backend e2e — conflict resolve, CLI shape fix, parser bug fix (#310)
- *(installer)* B00t polyglot installer — TUI deploys skills/agents/hooks to 5 runtimes (#318)
- Publish deterministic Claude marketplace skill bundles (#328)
- Achieve 100% core and 50% advanced feature test coverage (#332)
- Phase 7 - LLM Inference + RAG Integration + Gospel Enhancements (#333)
- *(k0mmand3r)* Datum-driven slash command dispatch in b00t-cli (#335)
- *(pi+moltis)* Fix pi agent dispatch + moltis hive integration (#339)
- *(operator)* Assimilate microsoft/mcp + MCP patterns for operator role (#340)
- Adversarial gemma4 loop + epiphany culture + OpenHarness gap-fill (#345)
- *(ralph)* RL research assimilation + H2 checkpoint + R2 tier pre-filter (#346)
- *(capability)* Add unified capability registry with --list and --capability-type flags
- *(capability)* Add unified capability registry (#353)
- *(opencode-agent-harness)* Opencode ch0nky skill-improve loop + ch0nky upgrades (#354)
- Add tomllmd ledg3rr integration (#360)
- *(l3dg3rr)* Add visualization utility workspace member and docs (#363)
- *(ux)* Data-driven gate pipeline, rhai kg/telemetry, mcp list diagnostics, exit codes (#368)
- WOW integrity system, AbDataSchema, FOCUS v1.3, model lifecycle, justfile modules (#370)
- Hive peer store with crypto signing, mDNS discovery, gossip protocol (#369)
- *(ux)* Gate pipeline, rhai kg/telemetry, mcp list diagnostics, exit codes (#372)
- *(gates)* Reusable rhai gate module, hook_detect wiring, hook engine gate_check

### 🐛 Bug Fixes

- Bad merge
- B00t-mcp more rusty (#58)
- *(build)* Complete centralized version management and fix CI build
- *(grok)* Resolve nested tokio runtime panic and Python module path
- *(orchestrator)* Add debug tracing and fix datum loading for all types
- *(orchestrator)* Properly detect and restart stopped containers
- Make 'b00t cli up' check-only by default, add --yes flag to update
- Correct _b00t_.toml path resolution logic
- *(test)* Correct test_namespace_helper to use get_hive_namespace
- *(install)* Add shell escaping for INSTALL_DIR in PATH exports (#202)
- *(security)* Require B00T_OPERATOR_JWT for JWT validation, remove placeholder secret (#211)
- Correct misleading comment about output streaming in job execution (#147)
- Remove misleading real-time streaming comments in execute_bash (#148)
- Resolve compilation errors in b00t-cli (#157)
- Resolve HashMap import and b00t_cli namespace errors in datum.rs (#169)
- Codex cant commit? (#172)
- Codex cant commit? (#179)
- Build errors (#197)
- Enforce options indentation in prd SKILL.md (#64)
- Use semver comparison in ConfigDatum::version_status() with fallback
- Use lowercase datum type format in up_command output for consistency
- Remove duplicate types and imports after merge
- Remove missing togaf-dsl and togaf-cli workspace members
- Remove incorrect _b00t_ path duplication in job commands (#228)
- Type qdrant payload values and refresh install deps (#234)
- Repair release workflows and b00t-cli upgrade path (#242)
- Zero-to-hero install audit + ralph OODA datum fixes (#272)
- *(inference-qwen3)* Podman CUDA container replaces brew llama-server (#285)
- *(ci)* Zbus v5 compat + soul.rs unclosed brace (#292)
- *(grok)* Raglight as default backend, drop Python server dependency (#297)
- *(grok)* Update Digest doc examples to use --content flag (#302)
- *(grok+irontology)* Address PR #310 review — ID coherence, blob durability, source handling, test paths, doc sync (#315)
- *(azure-cp)* Migrate to rmcp 0.8.5 + azure-sdk 0.21 APIs (#347)
- *(pi)* Memoize local qwen auth and install path (#355)
- Harden install runtime source and hive capabilities (#357)
- Classify typed datum filenames (#358)
- *(ci)* Unblock container path and narrow release gate failures (#351)
- CRIT-3, MAJ-1, CRIT-1, CRIT-2, CRIT-5, MAJ-3 (#371)
- Implement update_hermes_mcp_config, fix test constants, registry path fallback
- Extract home_dir_str helper, clean up capability_registry_path logic

### 💼 Other

- Complete installation with datums + minimal install via pkgx (#109)
- Unify codex+kv and executive AAIII/LSP/RAG drift slice (#334)
- V0.8.0 -- governance epoch 2, cake economy, hierarchy

### 🚜 Refactor

- Modularize b00t-cli main.rs into composable command modules (#50)
- Migrate all 'learn' and 'lfmf' logic to shared library (#74)
- *(core)* Migrate MCP registry, proxy, and RAG to b00t-c0re-lib
- Remove duplicate loader functions, use generic load_datum_providers
- *(bash)* Modular rc.d loader + resilient logger fallback (#240)
- *(workspace)* Restore local b00t cli integration updates (#362)

### 📚 Documentation

- *(jobs)* Add pydantic-ai evaluation and migration analysis
- *(env)* Implement b00t direnv pattern for environment management
- Clarify POC_DEMO.md describes single-process API demo, not inter-process IPC (#126)
- Update README with new features from merged PRs
- Add comfyui learning notes (#239)
- Rewrite README with accurate install story (#282)

### 🧪 Testing

- Add unit test for datum_type_str lowercase formatting
- Stabilize grok and cli test suite (#226)

### ⚙️ Miscellaneous Tasks

- Update b00t docs (#60)
- Misc
- Upgrade rust deps
- Update workflow and dagu tests (#135)
- Include acp crate in b00t-cli container build
- Tidy (#155)
- Version wouldn't merge
- Remove orphaned plantuml submodule reference
- Ignore unfinished experimental dirs, update plantuml submodule
- Ignore .old files (from PR #195)
- Merge main into feat/mcp-registry-with-dependency-install
- Checkpoint local workspace changes
- Move ralph to yei plugin home and remove plantuml integration
- Move Ralph to ralph-yei plugin home and remove plantuml integration (#233)
- Remove taskmaster core defaults (#258)
- *(vendor)* Bump irontology-mcp submodule to a479f23 (#336)
- Remove taskmaster-ai + gitignore runtime state dirs (#348)
- *(ralph)* Absorb _b00t_/ralph submodule into parent repo (#349)
- *(irontology)* Migrate storage_neumann to inline stubs + v0.7.48 (#356)
- Sunset deprecated b00t-wiggums (#359)
- *(release)* Bump b00t to 0.7.49 (#361)
- Rebase on upstream main + update l3dg3rr submodule to e9da669 (PR #69) (#364)
- Rebase on upstream main + update l3dg3rr submodule to e9da669 (PR #69) (#365)
- Checkpoint recent and 48-hour changes (#366)
- Update vendor/l3dg3rr submodule to ledgrrr (rebranded upstream) (#373)

## [0.0.1] - 2025-07-26

### 🚀 Features

- Improve Docker build workflow and update gitignore
- Add set -ex to setup.sh for verbose logging
- Add VS Code extension management commands (#29) (#41)

### 🐛 Bug Fixes

- Install missing dependencies and ensure path for setup.sh
- Pass GH_TOKEN to Docker build for gh CLI authentication
- Use GITHUB_TOKEN directly in setup.sh and remove GH_TOKEN build arg
- Conditionally skip gh extension install in CI if GITHUB_TOKEN is not set
- Skip gh extension install in CI/Docker builds
- Correctly detect Docker build environment in setup.sh
- Revert gh extension install conditional logic, rely on GH_TOKEN
- Securely pass GH_TOKEN to Docker build using BuildKit secrets
- Make justfile repo-root more robust for Docker builds
- Add uv to PATH in Dockerfile

### 💼 Other

- Add debug output for IS_CI and IS_DOCKER in setup.sh

### ⚙️ Miscellaneous Tasks

- *(version)* 0.0.1

## [1.1.0] - 2025-01-07

### 🚀 Features

- Install a plurality of requirements

### 💼 Other

- Latest jarrgon HSK
- Pipewatch
- Quickbuild zero-configuration build system

<!-- generated by git-cliff -->
