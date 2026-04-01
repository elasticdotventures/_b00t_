# Blessing Orchestration System: Implementation TODOs

## Current Status (431+ tests passing)
✅ Blessing trifecta (usage notes, execute access, data permissions)
✅ Tool abstraction (prefer newer tools like tofu)
✅ Role-based bash safety filters (per-role command whitelisting)
✅ Prayer workflow (agent blessing requests with policy evaluation)
✅ Executive override mechanism
✅ **Phase 7: Knowledge Base & RAG Integration** (83 new tests, 28 🦨 Phase 8 integration hooks)

---

## Phase 2: Validation & Verification (Next)

### Documentation Tasks
- [ ] Add inline docs for BlessingEvaluator policy checks
- [ ] Document blessing TOML schema with examples
- [ ] Create operator guide: "How to define blessings for your team"
- [ ] Add architecture diagram: prayer workflow flow chart
- [ ] Document tool_preference syntax and precedence rules

### Capability Integration Tasks
- [ ] Wire prayer workflow into k0mmand3r /negotiate command
- [ ] Add blessing evaluation to orchestrator agent
- [ ] Create examples: defining executor, observer, architect blessings
- [ ] Implement blessing introspection commands (list, filter, validate)

---

## Phase 3: Validation Checkpoints ("China Shop Rules")

### Datum Mutation Safety
- [ ] Implement validation hooks for datum changes
- [ ] Create checkpoint system (frozen state snapshots)
- [ ] Implement rollback to last valid checkpoint
- [ ] Add pre-commit blessing validation (.githook integration)
- [ ] Document "break it → fix it → or rollback" pattern

### Irontology Enhancement
- [ ] Add constraint validation (e.g., budget overflow detection)
- [ ] Validate blessing composition at mutation time
- [ ] Detect circular dependencies and composition conflicts
- [ ] Add remediation suggestions for validation failures

---

## Phase 4: Kaizen Loop (Assimilate Agent)

### Continuous Improvement
- [ ] Track denied blessings in audit log
- [ ] Detect patterns: "terraform blessed 47 times, denied 5 times"
- [ ] Trigger assimilate when denial rate exceeds threshold
- [ ] Wire assimilate to agent-skill-creator for new capability generation
- [ ] Implement feedback loop: denied → created → validated → deployed

### Orchestrator Integration
- [ ] Assimilate watches blessing graph for gaps
- [ ] Proactively suggests new blessings based on usage patterns
- [ ] Implements Kaizen: identify → fix → improve → repeat

---

## Phase 5: moltis-Hooks Integration

### Full k0mmand3r Wiring
- [ ] Complete k0mmand3r step transitions with hooks
- [ ] Implement pre/post-blessing hooks
- [ ] Add conditional blessing grants based on hook evaluation
- [ ] Support blessing composition (blessing:A requires hook:B to pass)

### Multi-Hook System
- [ ] Validate blessing changes via pre-commit hooks
- [ ] Coordinate multi-step workflows (hook chain)
- [ ] Support hook composition (hook A → hook B → hook C)
- [ ] Implement hook timeout and failure handling

---

## Phase 6: Sandbox Isolation

### Execution Boundaries
- [ ] Implement actual process/container isolation
- [ ] Enforce resource limits (CPU %, RAM, timeout)
- [ ] Restrict file access (readable_paths, writable_paths enforcement)
- [ ] Implement network isolation (allowed_hosts blocking)
- [ ] Add VPN requirement enforcement

### Model for Blessing Constraints
- [ ] Translate ExecuteAccess → process isolation rules
- [ ] Translate DataPermissions → file system ACLs
- [ ] Translate BashSafetyFilter → syscall filtering (seccomp)
- [ ] Support privilege escalation prevention

---

## ✅ Phase 7: Knowledge Base & RAG Integration (COMPLETE)

**Completed Artifacts:**
- ✅ LLMInference trait abstraction (backend-agnostic, object-safe async trait)
- ✅ CandleBackend (Rust-native GGUF loading, GPU device detection via nvidia-smi)
- ✅ LlamaCppBackend (feature-gated, deprecated path to Phase 9)
- ✅ RipgrepFallback (always-available text search, BM25-based scoring)
- ✅ KnowledgeBase (semantic metadata storage, layer caching)
- ✅ GraphRAG (DAG node/edge management, cycle detection, topological sort)
- ✅ CompositionPlan integration (blessing approvals → model layer stacking)
- ✅ Phase 8 integration hooks (ModelCache, SemanticDiscoveryCallback, CompositionValidation, AuditEventEmitter)
- ✅ 344-line GGUF layer documentation (schema, examples, backend support table)
- ✅ 83 new passing tests (22 inference, 19 RAG, 14 prayer workflow extensions)
- ✅ 28 🦨 skunks marked for Phase 8 refinement (model lifecycle, orchestrator hooks)

---

## Phase 8: Model Lifecycle & Orchestrator Integration (Ready - 28 🦨 markers)

### Semantic Discovery & Quality Feedback
- [ ] Implement SemanticDiscoveryCallback trait (assimilate agent notifications)
- [ ] Wire blessing discovery events to Kaizen loop (capture denial patterns)
- [ ] Quality score feedback loop (blessings improve over time)
- [ ] Phase 8 audit event hooks (composition_audit, denial_audit emission)

### Model Cache & Layer Management
- [ ] Implement ModelCache trait completion (layer eviction, LRU strategies)
- [ ] GPU memory budget enforcement (fits_in_budget validation)
- [ ] Device detection sophistication (handle mixed CUDA/MPS/CPU scenarios)
- [ ] Model swap performance profiling (batch processing optimization)

### Composition Validation & Checkpointing
- [ ] Implement CompositionValidation trait (checkpoint before state transitions)
- [ ] Rollback to last valid checkpoint on validation failure
- [ ] DAG validation (blessing dependency cycles, constraint overflow)
- [ ] Remediation suggestions (Irontology constraint refinement)

### Orchestrator State Machine Integration
- [ ] Wire composition_plan output to k0mmand3r step transitions
- [ ] Implement pre/post-blessing hooks for prayer workflow
- [ ] Support conditional blessing grants based on hook evaluation
- [ ] Blessing composition constraints (blessing:A requires hook:B to pass)

**Integration Points Designed:**
- `blessing/prayer/mod.rs`: CompositionValidation, AuditEventEmitter traits (Phase 8 stubs)
- `blessing/rag/mod.rs`: SemanticDiscoveryCallback trait (Phase 8 stub)
- `blessing/inference/mod.rs`: ModelCache trait completion needed
- `blessing/inference/candle.rs`: 3 🦨 skunks (memory calc, quantization detection, GPU tuning)
- `blessing/inference/llamacpp.rs`: 3 🦨 skunks (CPU→GPU migration)
- `blessing/inference/fallback.rs`: 5 🦨 skunks (ripgrep integration, BM25 scoring)
- `blessing/rag/graph.rs`: 3 🦨 skunks (constraint validation, budget overflow, remediation)
- `blessing/rag/mod.rs`: 4 🦨 skunks (semantic discovery, quality feedback, layer versioning)
- `blessing/prayer/mod.rs`: 6 🦨 skunks (Phase 8 orchestrator integration)

---

## Testing Priorities

### Current (Phase 2)
- [ ] Integration tests: full prayer workflow end-to-end
- [ ] Policy evaluation edge cases
- [ ] Budget calculation under heavy load
- [ ] Circular dependency detection

### Future (Phases 3-7)
- [ ] Checkpoint/rollback scenarios
- [ ] Kaizen loop pattern detection accuracy
- [ ] Hook execution and composition
- [ ] Sandbox isolation enforcement
- [ ] RAG retrieval accuracy
- [ ] Model swapping performance

---

## Documentation Structure

```
/docs/
├── blessing-system/
│   ├── overview.md                 # High-level architecture
│   ├── trifecta-model.md           # Blessing trifecta components
│   ├── prayer-workflow.md          # Agent blessing requests
│   ├── policy-evaluation.md        # How decisions are made
│   ├── operator-guide.md           # How to define blessings
│   └── examples/
│       ├── executor-blessings.md
│       ├── observer-blessings.md
│       └── architect-blessings.md
├── assimilate/
│   ├── kaizen-loop.md              # Continuous improvement
│   └── capability-generation.md
└── rag/
    ├── knowledge-base.md
    ├── model-layers.md
    └── inference-optimization.md
```

---

## Success Criteria

- ✅ All 348+ tests passing → **431+ tests passing (Phase 7 complete)**
- ✅ **RAG system stores and retrieves blessed capabilities** (Phase 7)
- ✅ **LLM inference with three-tier fallback** (Candle → llama.cpp → ripgrep)
- ✅ **Blessing composition planning** (prayer workflow extended with CompositionPlan)
- ✅ **Phase 8 integration points designed** (28 🦨 markers for semantic/cache/validation/orchestrator work)
- ⏳ Prayer workflow fully integrated into orchestrator (Phase 2)
- ⏳ Checkpoint/rollback system operational (Phase 3)
- ⏳ Assimilate detects gaps and creates capabilities (Phase 4)
- ⏳ moltis hooks fully wired into k0mmand3r (Phase 5)
- ⏳ Sandbox isolation enforces all permission types (Phase 6)
- ⏳ **Model swapping on GPU functional for batch processing** (Phase 8)

---

## Notes

- Each phase should maintain 100% test coverage
- Use TDD for all new implementations
- Keep phase scope manageable (1-2 weeks each)
- Get user feedback after each phase
- Document as you go (not at the end)
