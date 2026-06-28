# Australian Tax Capability — Ranked Ideation
# Generated: 2026-06-21 | Source: grok dual-backend + STRATEGY.md grounding

## Ranked Feature Ideas

### 1. 🥇 ATO Legislation Ingestion Pipeline (Score: 95/100)
**Impact**: Foundational — all other features depend on having structured tax knowledge
**Effort**: Medium (leverage existing arxiv→chunk→evidence→requirement pipeline)
**Risk**: Low (ATO API is stable, document formats are standard)
**Rationale**: Without the legislation knowledge base, no other feature works. This is the necessary first step. b00t's existing document pipeline (doc_pipeline.rs, PipelineNode composition) directly applies.

### 2. 🥈 Natural Language Tax Query (Score: 88/100)
**Impact**: High — the primary user-facing feature
**Effort**: Medium (requires legislation pipeline + LLM integration)
**Risk**: Medium (accuracy of legal reasoning must be verified)
**Rationale**: This is what users actually want — "Can I claim X as a deduction?" answered with provenance to specific ATO rulings. Proxy-pointer RAG already exists in b00t's Evidence→ProvenancePointer chain.

### 3. 🥉 Income Tax Calculator (Score: 82/100)
**Impact**: High — concrete numerical value
**Effort**: Low (deterministic calculation, ATO publishes formulas)
**Risk**: Low (formulas are mathematically verifiable)
**Rationale**: ATO tax tables are public and formulaic. Simple calculator with FOL-verified correctness (∀ input: calculate(input) = ATO_formula(input)). Quick win that builds trust.

### 4. GST Calculator (Score: 75/100)
**Impact**: Medium — business-focused
**Effort**: Low (10% flat rate, simple rules)
**Risk**: Low
**Rationale**: GST is simpler than income tax. Good second calculator after income tax.

### 5. Capital Gains Tax (CGT) Calculator (Score: 70/100)
**Impact**: Medium — investor-focused
**Effort**: Medium (cost base calculation, discounts, exemptions)
**Risk**: Medium (more edge cases than income tax)
**Rationale**: CGT has discount rules (50% for individuals holding >12 months), cost base adjustments, and exemptions. More complex but valuable for investor users.

### 6. Audit Report Generator (Score: 68/100)
**Impact**: High for tax agents
**Effort**: Medium (requires all calculators + legislation pipeline)
**Risk**: Low (templated output from existing data)
**Rationale**: Auto-generate compliance reports with full legislative provenance chain. Each number in the report links back to the ATO ruling that justifies it.

### 7. Fringe Benefits Tax (FBT) Calculator (Score: 55/100)
**Impact**: Niche — employer-focused
**Effort**: Medium (complex rules, statutory formula)
**Risk**: Medium
**Rationale**: Lower priority than core income tax/GST. Important for business users but smaller audience.

### 8. Multi-Year Tax Planning (Score: 50/100)
**Impact**: Medium — financial advisors
**Effort**: High (scenario modeling, year-over-year state)
**Risk**: High (prediction accuracy, legislative changes)
**Rationale**: Complex feature. Depends on all calculators being correct first.

## Recommendation

**Sprint 1 (this cycle)**: #1 Legislation Pipeline + #3 Income Tax Calculator (foundation + quick win)
**Sprint 2**: #2 Natural Language Query (primary UX)
**Sprint 3**: #4 GST + #5 CGT + #6 Audit Reports
**Backlog**: #7 FBT + #8 Multi-Year (revisit after user feedback)
