---
name: tax-lawyer-software-interfaces
description: |
  MCP tool design patterns for the Tax-Lawyer Platform. Thin Satisfies wrappers
  (<=10 lines per handler), TaxArgs contract types, and the full tool name
  taxonomy for ledgerr_tax MCP actions.
version: 1.0.0
tags: [mcp, satisfies, contract, ledgerr, tool-design, thin-wrapper]
tier: ch0nky
complexity: 5
applies_to:
  - "MCP tool design"
  - "ledgerr_tax"
  - "TaxArgs"
  - "thin wrapper"
  - "satisfies pattern"
output_types: [.rs, .json]
depends_on: [tax-lawyer-ufo-types, tax-lawyer-evidence-graph]
unlocks:
  - ledgerr_tax MCP tools
metadata:
  ufo_stereotype: Abstract
  legislation: []
  iso_types: []
---

## MCP Action Layer Design Constraint

Every MCP tool handler MUST be <=10 lines of Rust. ALL domain logic lives in
`Satisfies<C>` implementations. The handler's only jobs:

1. Deserialize `TaxArgs` from MCP input JSON
2. Construct the domain object
3. Call `.satisfies(constraint)`
4. Serialize result as JSON and return

### Canonical handler pattern

```rust
// contract.rs
pub struct AuRdRegisterArgs {
    pub lei: String,
    pub activity_name: String,
    pub start_year: u32,
    pub end_year: u32,
    pub domain: String,
}

// mcp_adapter.rs
pub async fn handle_au_rd_register(args: Value) -> Result<Value, anyhow::Error> {
    let args: AuRdRegisterArgs = serde_json::from_value(args)?;
    let activity = AuRdActivity::from_args(args)?;
    let evidence = activity.satisfies(&AuRdEligibility)?;
    Ok(serde_json::to_value(evidence)?)
}
```

## Tool Name Taxonomy

All ledgerr_tax tools follow the pattern:
`ledgerr_tax_<jurisdiction>_<domain>_<action>`

### AU R&D Tax Incentive

| Tool | Args | Returns |
|---|---|---|
| `ledgerr_tax_au_rd_register` | lei, activity_name, start_year, end_year, domain | Vec\<EvidenceNode\> |
| `ledgerr_tax_au_rd_add_expenditure` | activity_id, amount_aud, category, description | Vec\<EvidenceNode\> |
| `ledgerr_tax_au_rd_calculate_offset` | lei, fiscal_year | AuRdOffsetResult |
| `ledgerr_tax_au_rd_export_report` | lei, fiscal_year, format | String (JSONL|MD) |

### US R&D Tax Credit

| Tool | Args | Returns |
|---|---|---|
| `ledgerr_tax_us_rd_register_activity` | entity_ein, activity_name, tech_area | Vec\<EvidenceNode\> |
| `ledgerr_tax_us_rd_add_qre` | activity_id, qre_type, amount_usd | Vec\<EvidenceNode\> |
| `ledgerr_tax_us_rd_calculate_credit` | entity_ein, tax_year | UsRdcResult |
| `ledgerr_tax_us_rd_form_6765` | entity_ein, tax_year | Form6765Draft |

### Crypto — AU

| Tool | Args | Returns |
|---|---|---|
| `ledgerr_tax_au_crypto_record_tx` | wallet_id, tx_hash, tx_type, amount, asset, timestamp | Vec\<EvidenceNode\> |
| `ledgerr_tax_au_crypto_cgt_event` | disposal_id, method | CgtEventResult |
| `ledgerr_tax_au_crypto_personal_use` | wallet_id, tx_hash | PersonalUseAssessment |

### Crypto — US

| Tool | Args | Returns |
|---|---|---|
| `ledgerr_tax_us_crypto_record_tx` | wallet_id, tx_hash, tx_type, amount, asset, timestamp | Vec\<EvidenceNode\> |
| `ledgerr_tax_us_crypto_cost_basis` | wallet_id, method, tax_year | CostBasisResult |
| `ledgerr_tax_us_crypto_form_8949` | wallet_id, tax_year | Form8949Draft |

## TaxArgs Convention

```rust
// All args structs derive these — no exceptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SomeTaxArgs {
    // ...
}
```

- All monetary amounts as `String` (e.g. `"250000.00"`) — avoids float precision
- Parse to `rust_decimal::Decimal` inside the handler or domain constructor
- All LEIs validated before domain object construction
- All dates as ISO 8601 strings (`"2025-06-30"`)

## Error Semantics

```rust
// MCP handlers return JSON errors, not panics
pub struct TaxError {
    pub code: TaxErrorCode,
    pub message: String,
    pub evidence_so_far: Vec<EvidenceNode>,  // partial evidence if available
}

pub enum TaxErrorCode {
    InvalidLei,
    IneligibleActivity,
    InsufficientExpenditure,
    MissingDocumentation,
    JurisdictionConflict,
}
```

# b00t:map v1
# summary: MCP thin-wrapper pattern for ledgerr_tax tools — TaxArgs contract types
# tags: mcp, satisfies, ledgerr, tool-design, contract
# tier: ch0nky
# complexity: 5
