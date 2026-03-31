# Blessing Orchestration System: Implementation TODOs

## Current Status (348 tests passing)
✅ Blessing trifecta (usage notes, execute access, data permissions)
✅ Tool abstraction (prefer newer tools like tofu)
✅ Role-based bash safety filters (per-role command whitelisting)
✅ Prayer workflow (agent blessing requests with policy evaluation)
✅ Executive override mechanism

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

## Phase 7: Knowledge Base & RAG Integration

### Semantic Knowledge Storage
- [ ] Implement GGUF model loading with candle or llama.cpp
- [ ] Create embeddings of assimilated capabilities
- [ ] Store semantic knowledge in vector database
- [ ] Implement blessing retrieval by semantic similarity
- [ ] Support fine-tuning on specialized blessings (domain-specific)

### Model Layer Composition
- [ ] Support blessing-based model layer stacking
- [ ] Implement GPU memory optimization (batch processing)
- [ ] Create control sequence injection for tool calls
- [ ] Implement model swapping on shared GPU

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

- ✅ All 348 tests passing (maintained throughout)
- ⏳ Prayer workflow fully integrated into orchestrator
- ⏳ Checkpoint/rollback system operational
- ⏳ Assimilate detects gaps and creates capabilities
- ⏳ moltis hooks fully wired into k0mmand3r
- ⏳ Sandbox isolation enforces all permission types
- ⏳ RAG system stores and retrieves blessed capabilities
- ⏳ Model swapping on GPU functional for batch processing

---

## Notes

- Each phase should maintain 100% test coverage
- Use TDD for all new implementations
- Keep phase scope manageable (1-2 weeks each)
- Get user feedback after each phase
- Document as you go (not at the end)
