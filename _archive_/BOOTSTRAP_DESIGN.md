# b00t Self-Bootstrapping Architecture

**Status**: 🚧 Design Phase
**Branch**: `feat/bootstrap-self-config`
**Target**: Self-configuring, self-healing b00t hive infrastructure

---

## Vision

Transform b00t from a dotfile management tool into a **self-aware, self-configuring software stack** that:

1. **Bootstraps itself** - Zero-to-production with `b00t init`
2. **Discovers local hives** - Auto-find collaborating b00t instances on trusted networks
3. **Manages its own dependencies** - Docker containers, MCP servers, agent pools
4. **Adapts at runtime** - Agents modify config, hot-reload without full restart
5. **Secures secrets** - OS keychain integration, encrypted storage
6. **Delegates intelligently** - Route tasks to optimal model (Codex, Gemini, ADK)

---

## Key Technologies

### Toon Format (Context Serialization)
- **Purpose**: 30-60% token reduction vs JSON for LLM context
- **Use Case**: Session snapshots, RAG context, agent-to-agent communication
- **Format**: TOML-based tabular arrays with schema declarations
- **Research**: See `BOOTSTRAP_DESIGN.md` § Toon Format

### Service Discovery
- **Local**: mDNS/Bonjour for zero-config discovery
- **Remote**: Tailscale for secure mesh, Cloudflare Zero Trust for public exposure
- **Registry**: Consul for service catalog (future: distributed b00t hives)

### Secrets Management
- **Primary**: OS keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager)
- **Fallback**: age-encrypted files when no OS keychain available
- **Pattern**: MCP servers retrieve credentials at runtime (never hardcode)

### Agent Delegation
- **Framework**: DSPy for declarative LLM programming
- **Routing**: Task → Agent based on capabilities, cost, availability
- **Models**: Claude Sonnet (code), Gemini Flash (research), GPT-4o (general)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                          b00t Bootstrap Layer                        │
│  ┌────────────────┐  ┌──────────────┐  ┌─────────────────────────┐ │
│  │ b00t-init      │  │ b00t-discover│  │ b00t-menuconfig         │ │
│  │ (First Contact)│─▶│ (Network)    │◀─│ (Interactive Setup)     │ │
│  └────────────────┘  └──────────────┘  └─────────────────────────┘ │
└──────────────┬──────────────────────────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────────────────────────┐
│                    b00t Datum Registry (Ontology)                    │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ Stack Datums (_b00t_/*.toml)                                │   │
│  │ ├─ mcp/*.mcp.toml      (MCP server configs)                 │   │
│  │ ├─ docker/*.docker.toml (Docker services)                   │   │
│  │ ├─ agent/*.agent.toml   (Agent definitions) ◀── NEW         │   │
│  │ ├─ network/*.net.toml   (Network configs) ◀── NEW           │   │
│  │ └─ secrets/*.keychain   (Encrypted vault) ◀── NEW           │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────┬──────────────────────────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────────────────────────┐
│                    b00t Runtime Orchestrator                         │
│  ┌─────────────────┐  ┌──────────────────┐  ┌──────────────────┐   │
│  │ MCP Hub         │  │ Agent Pool       │  │ Service Manager  │   │
│  │ ├─Dedup         │  │ ├─Hot Reload     │  │ ├─Docker        │   │
│  │ ├─Forward       │  │ ├─Multi-Model    │  │ ├─Qdrant        │   │
│  │ └─RPC Channels  │  │ └─Delegation     │  │ └─Redis         │   │
│  └─────────────────┘  └──────────────────┘  └──────────────────┘   │
└──────────────┬──────────────────────────────────────────────────────┘
               │
┌──────────────▼──────────────────────────────────────────────────────┐
│                    State & Context Layer                             │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │ Toon Format (Serialization)                                  │   │
│  │ ├─ .toon files (Normalized context snapshots)               │   │
│  │ ├─ Vector store (Qdrant) for semantic search                │   │
│  │ └─ Session state (Redis) for ephemeral data                 │   │
│  └──────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 0: Foundation (MVP)
**Timeline**: Week 1
**Goal**: Bootstrap existing b00t installation with minimal intervention

**Deliverables**:
- `_b00t_/bootstrap.toml` - Bootstrap configuration
- `b00t-cli bootstrap` - Dependency checker, config skeleton generator
- `.toon` report format for install summary

**Files Created**:
```
_b00t_/
└─ bootstrap.toml          # Bootstrap config

b00t-cli/src/
└─ bootstrap/
   ├─ mod.rs               # Bootstrap orchestrator
   ├─ prereq.rs            # Dependency checker
   ├─ skeleton.rs          # Config skeleton generator
   └─ report.rs            # .toon report serializer
```

### Phase 1: Interactive Config
**Timeline**: Week 2
**Goal**: Menuconfig-style setup with fzf, sane defaults, timeout automation

**Key Decision**: Fork [dialoguer](https://github.com/console-rs/dialoguer) + [skim](https://github.com/lotabout/skim) to add timeout behavior

**Deliverables**:
- Timeout-aware prompts (30s default → auto-select)
- Datum-driven menu generation
- Starship-consistent theming

### Phase 2-8: See Full Architecture Document

For complete phase breakdown, see sections below.

---

## Critical Design Decisions

### 1. Toon Format over JSON/YAML
**Rationale**: 30-60% token reduction for RAG/LLM context
**Trade-off**: Custom format vs ecosystem tooling
**Mitigation**: Keep TOML for human-editable config, Toon for machine context

### 2. OS Keychain over .env Files
**Rationale**: Secure by default, no plaintext secrets in repo
**Trade-off**: Platform-specific backends vs universal .env
**Mitigation**: age-encrypted fallback when OS keychain unavailable

### 3. MCP Hub (Centralized) vs Distributed
**Rationale**: Massive resource savings (1 GitHub MCP vs N instances)
**Trade-off**: Single point of failure vs process overhead
**Mitigation**: Auto-restart on crash, agents spawn local fallback

### 4. Rust CLI + Python MCP Servers
**Rationale**: Rust for infrastructure (fast, single binary), Python for AI (rich ecosystem)
**Trade-off**: Two-language maintenance vs best-of-breed tools
**Mitigation**: Clear boundary - Rust for system, Python for intelligence

---

## Security & Privacy

### Encrypted Storage
- **Secrets**: OS keychain > age-encrypted file
- **Sessions**: Toon files contain PII → encrypt ~/.b00t/sessions/ at rest
- **Network**: Zero-trust by default (Tailscale/Cloudflare)

### Datum Validation
```rust
pub fn validate_datum(path: &Path) -> Result<()> {
    // 1. Schema validation (against _b00t_/schemas/)
    // 2. Security checks (no hardcoded secrets)
    // 3. Dependency resolution (all deps available)

    if content.contains("password =") {
        return Err(anyhow!("🚩 Hardcoded secret detected"));
    }
}
```

---

## File Structure

```
_b00t_/
├─ bootstrap.toml                # NEW: Bootstrap config
├─ agent/                        # NEW: Agent definitions
│  ├─ codex.agent.toml
│  ├─ gemini.agent.toml
│  └─ router.toml
├─ network/                      # NEW: Network discovery
│  ├─ hive.net.toml
│  └─ mesh.net.toml
├─ secrets/                      # NEW: Keychain metadata
│  └─ keychain.toml
└─ schemas/                      # NEW: Datum schemas
   ├─ agent.schema.toml
   └─ network.schema.toml

b00t-cli/src/
├─ bootstrap/                    # NEW: Phase 0
├─ menuconfig/                   # NEW: Phase 1
├─ network/                      # NEW: Phase 2
├─ onboard/                      # NEW: Phase 3
├─ toon/                         # NEW: Phase 4
├─ daemon/                       # NEW: Phase 5
├─ secrets/                      # NEW: Phase 6
└─ agent/                        # NEW: Phase 8

b00t-mcp-hub/                    # NEW: Phase 7 (separate crate)
├─ src/
│  ├─ pool.rs                    # Server pool management
│  ├─ router.rs                  # Request routing
│  └─ health.rs                  # Health monitoring
└─ Cargo.toml

~/.b00t/                         # User-specific state
├─ config/local.toml             # User overrides
├─ sessions/*.toon               # Toon snapshots
└─ logs/                         # Daemon logs
```

---

## Next Steps

**Immediate (This PR)**:
1. ✅ Create `BOOTSTRAP_DESIGN.md` (this document)
2. ✅ Create `_b00t_/bootstrap.toml` skeleton
3. ⏳ Implement Phase 0 prerequisite checker
4. ⏳ Generate `.toon` install report

**Future PRs**:
- Phase 1: Interactive menuconfig
- Phase 2: Network discovery (mDNS/Tailscale)
- Phase 4: Toon serialization for sessions
- Phase 7: MCP hub orchestrator

---

## References

- **Toon Format**: https://github.com/toon-format/toon
- **Linux Kconfig**: https://www.kernel.org/doc/Documentation/kbuild/kconfig-language.txt
- **DSPy**: https://dspy.ai/
- **Consul**: https://developer.hashicorp.com/consul
- **mDNS RFC**: https://datatracker.ietf.org/doc/html/rfc6762

---

**Last Updated**: 2025-11-09
**Status**: Initial Design
**Next Milestone**: Phase 0 MVP
