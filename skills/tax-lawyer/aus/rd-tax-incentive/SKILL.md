---
name: tax-lawyer-au-rd-tax-incentive
description: |
  Australian R&D Tax Incentive (RDTI) — ITAA 1997 Division 355. Covers
  eligibility criteria for core and supporting R&D activities, registration
  with AusIndustry/IP Australia, expenditure categories, offset rates, and
  the company size threshold test.
version: 1.0.0
tags: [au, rd, tax-incentive, itaa1997, div355, ausindustry, offset, ato]
tier: frontier
complexity: 9
applies_to:
  - "R&D Tax Incentive"
  - "RDTI"
  - "Division 355"
  - "AusIndustry"
  - "core R&D"
  - "supporting R&D"
  - "ITAA 1997"
  - "ATO R&D"
output_types: [.json, .md]
depends_on: [tax-lawyer-ufo-types, tax-lawyer-evidence-graph]
unlocks:
  - "ledgerr_tax_au_rd_*"
metadata:
  ufo_stereotype: Endurant
  satisfies_constraint: AuRdEligibility
  legislation:
    - "Income Tax Assessment Act 1997 (ITAA 1997) Div 355"
    - "Industry Research and Development Act 1986 s 27A-27J"
    - "Corporations Act 2001 s 45A (small proprietary company threshold)"
  iso_types: ["ISO 17442 (LEI)"]
---

## AU R&D Tax Incentive — Overview

:citation: ITAA 1997 Division 355; Industry Research and Development Act 1986

Two-tier tax offset for R&D conducted in Australia by eligible entities:

| Turnover | Offset Rate | Type |
|---|---|---|
| < AUD 20M | **43.5%** refundable | Cash refund if tax loss |
| ≥ AUD 20M | **38.5%** non-refundable | Reduces tax payable |

Program run jointly by ATO + IP Australia/AusIndustry.
Registration: via Industry Research and Development portal (RDTI online portal).

## Eligible Entity Test

:citation: ITAA 1997 s 355-35

An entity is eligible if it is:
- A company incorporated under **Corporations Act 2001** (Australian or foreign with PE in AU), OR
- A **corporate limited partnership**, OR
- A **public trading trust**

NOT eligible:
- Sole traders, individuals, partnerships (non-corporate)
- Tax-exempt entities
- Entities exempt under special rules

### Satisfies<AuEntityEligibility> checklist:
- [ ] Incorporated company (or CLP/PTT)
- [ ] Not tax-exempt under ITAA 1997 s 50-1
- [ ] Has a valid LEI (for large entity claims)

## Core R&D Activity Test

:citation: ITAA 1997 s 355-25(1)

A core R&D activity MUST:

1. **Experimental activities** — involve systematic progression of work based on principles
   of established science/technology
2. **New knowledge** — conducted for the purpose of generating new knowledge
   (including new knowledge in the form of new/improved materials, products,
   devices, processes or services)
3. **Outcome not known** — the outcome cannot be known or determined in advance
   (genuine uncertainty)
4. **Hypothesis test** — activities undertaken to evaluate a hypothesis, generate results,
   or test a design or model

### What is NOT a core R&D activity:

:citation: ITAA 1997 s 355-25(2)

- Market research, market testing
- Pre-production activities (trial production, tooling, commercial production)
- Prospecting, exploration, drilling for minerals/petroleum
- Developing/maintaining computer software for internal administration
- Acquiring another entity's R&D results
- Creating artistic works

### Satisfies<CoreRdActivityTest> evidence pattern:

```
EvidenceNode {
  type: Claim,
  body: "Activity 'X' involves systematic experimentation to generate new knowledge in domain Y",
  citation: "ITAA 1997 s 355-25(1)(a)",
}
EvidenceNode {
  type: Claim,
  body: "Outcome not determinable in advance — genuine technical uncertainty exists",
  citation: "ITAA 1997 s 355-25(1)(b)",
}
```

## Supporting R&D Activity Test

:citation: ITAA 1997 s 355-30(1)

A supporting R&D activity is one that:
- Is directly related to, and supports, a core R&D activity
- Would be included in the R&D program

Supporting activities can include:
- Production runs to test experimental hypotheses
- Collection of data directly supporting core activity
- Maintenance of specialized equipment used for core activity

### 100% test for support activities:
If a supporting activity could be conducted independently without the core R&D activity,
it FAILS the supporting activity test.

## Expenditure Categories

:citation: ITAA 1997 s 355-45 to 355-105

| Category | Deduction Basis | Common Examples |
|---|---|---|
| **Employee labour** | 100% of time on R&D | Researcher salaries, direct overheads |
| **Contractor labour** | 100% of contracted amount | External R&D contractors |
| **Materials** | 100% used/transformed in R&D | Chemicals, components |
| **Plant operating costs** | Apportioned % R&D use | Lab equipment, computers |
| **Feedstock input** | Feedstock formula | Raw materials partially converted |

### Exclusions from R&D expenditure:
- Interest expenses
- Core technology (existing IP incorporated into R&D)
- Amounts paid to associates at non-arm's length prices (must use arm's length amount)

## Registration Requirements

:citation: Industry Research and Development Act 1986 s 27J

- Register via AusIndustry RDTI Online portal
- Registration deadline: **10 months after end of income year**
  (for 30 June year end: deadline = 30 April following year)
- Must describe: activity name, description, field of science, expected outcomes
- Late registration: possible application to Commissioner (discretion)

## Advance Finding (optional)

Companies can seek an advance finding from IP Australia before incurring expenditure:
- Binding determination on whether activities are core/supporting R&D
- Valid for 3 income years
- Fee: AUD ~6,000–15,000 depending on complexity

## AuRdActivity Struct (Rust domain model)

```rust
pub struct AuRdActivity {
    pub ufo: UfoCategory,                    // Endurant
    pub ufo_kind: EndurantStereotype,        // Kind
    pub lei: String,                         // ISO 17442 LEI of claimant
    pub activity_id: Uuid,
    pub activity_name: String,
    pub field_of_science: String,            // ANZSRC Field of Research code
    pub start_year: u32,                     // income year start
    pub end_year: u32,                       // income year end
    pub domain: RdDomain,
    pub activity_type: RdActivityType,       // Core | Supporting
    pub registration_date: Option<NaiveDate>,
    pub advance_finding: Option<AdvanceFinding>,
}

pub enum RdActivityType { Core, Supporting }
pub enum RdDomain {
    SoftwareEngineering, Biotechnology, Chemistry,
    Materials, Aerospace, Other(String)
}
```

# b00t:map v1
# summary: ITAA 1997 Div 355 R&D Tax Incentive — eligibility, expenditure, registration
# tags: au, rd, tax-incentive, itaa1997, div355, ausindustry, ato
# tier: frontier
# complexity: 9
