---
name: tax-lawyer-us-r-and-d-credit
description: |
  US R&D Tax Credit under IRC Section 41 (Internal Revenue Code). The 4-part
  test for qualifying research, QRE categories (wages, supplies, contract), 
  ASC vs Regular Credit, Form 6765 filing requirements, and AU-parent/US-sub
  structure considerations.
version: 1.0.0
tags: [us, rd, tax-credit, irc41, qre, form6765, 4-part-test, irs]
tier: frontier
complexity: 9
applies_to:
  - "R&D Tax Credit US"
  - "IRC Section 41"
  - "Section 41"
  - "QRE"
  - "qualified research expenses"
  - "Form 6765"
  - "4-part test"
  - "research credit"
output_types: [.json, .md]
depends_on: [tax-lawyer-ufo-types, tax-lawyer-evidence-graph]
unlocks:
  - "ledgerr_tax_us_rd_*"
metadata:
  ufo_stereotype: Endurant
  satisfies_constraint: UsRdcFourPartTest
  legislation:
    - "Internal Revenue Code (IRC) Section 41 (Research credit)"
    - "IRC Section 174 (Research experimental expenditures — amortization after 2021)"
    - "Treas. Reg. § 1.41-4 (qualified research)"
    - "Treas. Reg. § 1.41-2 (qualified research expenses)"
    - "IRS Notice 2023-63 (Sec 174 amortization guidance)"
  iso_types: ["ISO 4217"]
---

## IRC Section 41 — Research Credit Overview

:citation: IRC § 41; Treas. Reg. § 1.41-1

Two credit calculation methods:

| Method | Formula | Notes |
|---|---|---|
| **Regular Credit** | 20% × (Current QREs − Base Amount) | Base = Fixed-Base% × Avg gross receipts (prior 4 yrs) |
| **ASC (Alternative Simplified Credit)** | 14% × (Current QREs − 50% × Avg QREs prior 3 yrs) | If no prior QRE history: 6% × current QREs |

Most companies use **ASC** — simpler calculation, avoids historical base period issues.

Credit is a **general business credit** (IRC § 38) — subject to tax liability limitations.
Unused credits carry back 1 year, forward 20 years.

**Payroll Tax Election (IRC § 41(h))**: Qualified small businesses (≤ AUD 5M gross receipts,
< 5 years old) may elect to apply up to **$500,000 credit against payroll tax** (FICA).
This is critical for pre-revenue companies with no income tax.

## IRC Section 41 Four-Part Test

:citation: IRC § 41(d); Treas. Reg. § 1.41-4(a)

An activity is **Qualified Research** only if ALL FOUR parts are satisfied:

### Part 1: Permitted Purpose

:citation: IRC § 41(d)(1)(A)

Research must be undertaken to discover information which is:
- Technological in nature (relies on principles of physical/biological/computer science
  or engineering)
- Useful in the development of a new or improved:
  - Business component (product, process, computer software, technique, formula, invention)

**FAILS if**: Research relates to style, taste, cosmetic, or seasonal design changes.

### Part 2: Technological in Nature

:citation: IRC § 41(d)(1)(B)(i); Treas. Reg. § 1.41-4(a)(2)

The activities must fundamentally rely on **principles of physical/biological/computer
science, or engineering**.

**Key case**: _United States v. McFerren_ — "technological in nature" requires more
than applying existing knowledge; must involve hard sciences.

### Part 3: Technical Uncertainty

:citation: IRC § 41(d)(1)(B)(ii); Treas. Reg. § 1.41-4(a)(3)

Activities intended to discover information to eliminate:
- **Uncertainty** concerning the capability, method, or appropriate design of the business component

The uncertainty test is met if, at the time activities begin, the information
needed to develop the business component is NOT **readily available**.

**Readily available**: If reasonably available through literature, industry practice,
or prior experience → fails technical uncertainty.

### Part 4: Process of Experimentation

:citation: IRC § 41(d)(1)(B)(iii); Treas. Reg. § 1.41-4(a)(5)

Substantially all activities must constitute elements of a **process of experimentation**:
- Evaluating alternatives
- Testing hypotheses
- Identifying and analyzing uncertainties
- Refining or discarding approaches

**Substantially all**: 80%+ of activities must constitute experimentation.

**Critical exclusion**: _Developing internal-use software_ requires additional heightened
scrutiny (High Threshold of Innovation test: Treas. Reg. § 1.41-4(c)(6)(iv)).

### 4-Part Test exclusions (IRC § 41(d)(4))

| Exclusion | Examples |
|---|---|
| Research after commercial production | Adaptation, efficiency improvements post-launch |
| Surveys/market research | Customer interviews, A/B testing for UI |
| Management/organizational functions | HR software, accounting software |
| Social science / arts | Business strategy, behavioral economics |
| Internal-use software (basic) | ERP, CRM, payroll — unless High Threshold applies |
| Foreign research | Activities conducted outside US (Guam/PR qualify) |
| Funded research | Research funded by grants, contracts (other party bears risk) |

## Qualified Research Expenses (QREs)

:citation: IRC § 41(b); Treas. Reg. § 1.41-2

### Category 1: Employee Wages
- W-2 wages for employees **directly engaged** in qualified research
- W-2 wages for employees **directly supervising** qualified research
- W-2 wages for employees **directly supporting** qualified research

**Wage apportionment**: Use time tracking (% of time on qualifying activities).
IRS requires contemporaneous records — estimates accepted if based on reasonable methodology.

### Category 2: Supplies
- **Tangible property** used and consumed in qualified research
- NOT: general overhead, land, buildings, major equipment (those are capital)
- Examples: chemicals, lab materials, prototype components, test equipment consumables

### Category 3: Contract Research (65% Rule)
- 65% of amounts paid to a **non-employee** for qualified research services
- Must pay for qualified research (other party bears economic risk OR owner retains
  substantial rights in the results)
- If contractor bears no risk and company retains rights → 65% rule applies
- If contractor funds own research → 0% (funded research exclusion)

## IRC Section 174 Amortization (post-TCJA 2022)

:citation: IRC § 174; IRS Notice 2023-63

**Critical change effective 2022**: Section 174 research expenditures can NO LONGER be
immediately deducted. Must be **amortized** over 5 years (domestic) or 15 years (foreign)
using mid-year convention.

This does NOT affect the Section 41 credit calculation directly, but:
- Increases taxable income in early years (less deduction)
- The credit remains available to offset tax
- Many companies file refund claims for prior years under old rules

## Form 6765 — Credit for Increasing Research Activities

:citation: Form 6765; IRC § 41

Sections:
- **Section A**: Regular Credit calculation (not recommended for new filers)
- **Section B**: Alternative Simplified Credit (ASC) — use this
- **Section C**: Payroll tax election (for QSBs)
- **Section D**: Summary

**Documentation requirements** (IRS best practices):
- Activity-level description (what was researched, why uncertain)
- Employee list with % time on R&D per activity
- Supplies and contract research invoices
- Project contemporaneous records (lab notebooks, code commits, design docs)

## AU Parent / US Subsidiary Structure

For Australian parent companies with US subsidiaries claiming R&D credit:

- Credit claimed by **US entity** (C-Corp or S-Corp)
- AU parent cannot claim US credit directly
- Inter-company R&D funding arrangements must NOT constitute "funded research"
  (if AU parent funds and takes rights → US sub has no QREs)
- Structure: US sub conducts and owns research, AU parent has license or cost-sharing

### Satisfies<UsRdcFourPartTest> for each activity:
All 4 parts must PASS. Partial credit not available for partial satisfaction.

## QreActivity Struct (Rust domain model)

```rust
pub struct QreActivity {
    pub ufo: UfoCategory,               // Endurant
    pub ufo_kind: EndurantStereotype,   // Kind
    pub activity_id: Uuid,
    pub entity_ein: String,             // Employer Identification Number (XX-XXXXXXX)
    pub activity_name: String,
    pub tech_area: TechArea,
    pub permitted_purpose_pass: bool,
    pub tech_in_nature_pass: bool,
    pub technical_uncertainty_pass: bool,
    pub process_of_experimentation_pass: bool,
    pub tax_year: u32,
}

pub struct UsRdcFourPartTest;

impl Satisfies<UsRdcFourPartTest> for QreActivity {
    type Evidence = EvidenceNode;
    type Error = anyhow::Error;

    fn satisfies(&self, _: &UsRdcFourPartTest) -> Result<Vec<EvidenceNode>, anyhow::Error> {
        let mut nodes = vec![];
        // Part 1
        nodes.push(check_permitted_purpose(self));
        // Part 2
        nodes.push(check_tech_in_nature(self));
        // Part 3
        nodes.push(check_technical_uncertainty(self));
        // Part 4
        nodes.push(check_process_of_experimentation(self));
        Ok(nodes)
    }
}
```

# b00t:map v1
# summary: IRC Sec 41 US R&D Tax Credit — 4-part test, QREs, ASC, Form 6765
# tags: us, rd, tax-credit, irc41, qre, form6765, 4-part-test, irs
# tier: frontier
# complexity: 9
