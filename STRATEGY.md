---
name: australian-tax-capability
last_updated: 2026-06-21
---

# Australian Tax Capability Strategy

## Target problem

Australian taxpayers and accountants lack an AI-assisted tool that can interpret ATO rulings, calculate tax obligations across income years, and generate audit-ready documentation. Existing solutions (spreadsheets, commercial tax software) require manual data entry, don't explain reasoning, and can't adapt to legislative changes without vendor updates. The Australian tax code is complex (ITAA 1936, ITAA 1997, GST Act) and changes annually via Budget measures and ATO rulings.

## Our approach

Build a b00t-powered tax capability that ingests ATO legislation and rulings as structured knowledge (arxiv-like document pipeline), answers natural-language tax queries with provenance (proxy-pointer RAG to source legislation), calculates obligations across financial years, and generates audit-ready reports. Leverage b00t's existing pipeline infrastructure (document chunking, evidence extraction, requirement derivation, FOL verification) to produce legally-reasoned outputs.

## Target users

- **Individual taxpayers**: Simple tax queries, deduction eligibility, tax return estimates
- **Accountants/bookkeepers**: Client tax planning, compliance checks, ruling interpretation
- **Tax agents**: Multi-client scenario analysis, ATO correspondence drafting

## Key metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Query accuracy | >95% correct ATO ruling references | Manual audit of 100 queries |
| Calculation correctness | >99% vs ATO calculators | Automated test suite |
| Response time | <10s for standard queries | b00t-admin health endpoint |
| Audit trail completeness | 100% provenance links | Every answer links to source legislation |
| User satisfaction | NPS >40 | In-app survey |

## Tracks

1. **Legislation ingestion**: ATO API → document pipeline → structured knowledge base
2. **Natural language query**: Tax questions → evidence extraction → reasoned answers
3. **Calculation engine**: Income tax, GST, CGT, FBT calculators (ATO formula-aligned)
4. **Audit reporting**: Auto-generated compliance reports with legislative provenance
5. **Agent integration**: b00t-admin dashboard, MCP tool for tax agents

## Non-goals

- Lodging tax returns directly with ATO (this is advice, not submission)
- Replacing registered tax agents (augmentation, not substitution)
- Supporting non-Australian tax jurisdictions (scope boundary)
