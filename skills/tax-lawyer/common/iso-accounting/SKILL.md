---
name: tax-lawyer-iso-accounting
description: |
  ISO and IFRS accounting standards used in the Tax-Lawyer Platform: ISO 20022
  (financial messaging), IFRS 9 (financial instruments), LEI (ISO 17442),
  and ISO 4217 (currency codes). Grounds monetary domain objects in formal standards.
version: 1.0.0
tags: [iso, ifrs, accounting, lei, currency, iso20022, ifrs9, financial]
tier: ch0nky
complexity: 5
applies_to:
  - "ISO 20022"
  - "IFRS 9"
  - "LEI"
  - "financial instruments"
  - "currency codes"
output_types: [.json, .rs]
depends_on: [tax-lawyer-ufo-types]
unlocks: []
metadata:
  ufo_stereotype: Abstract
  legislation: []
  iso_types: ["ISO 20022", "ISO 17442", "ISO 4217", "IFRS 9"]
---

## ISO 20022 — Financial Services Messaging

ISO 20022 defines a common platform for financial services messaging.

### Relevance to Tax-Lawyer Platform

- `Camt.053` (Bank Account Statement): maps to `CryptoWallet` statement events
- `Pain.001` (Credit Transfer Initiation): maps to outbound crypto transfers
- Amount type: `ActiveCurrencyAndAmount { currency: Iso4217, value: Decimal }`

```rust
pub struct ActiveCurrencyAndAmount {
    pub currency: Iso4217,   // "AUD" | "USD" | "ETH" | "BTC"
    pub value: rust_decimal::Decimal,
}
```

## ISO 17442 — Legal Entity Identifier (LEI)

LEI is a 20-character alphanumeric code identifying legal entities that engage
in financial transactions globally. Required for ATO R&D registration.

### Structure
```
XXXXXXXXXX  (4-char LOU prefix)
XXXXXXXXXX  (14-char entity-specific)
XX          (2-char check digits, MOD 97-10)
```

### Examples
- `213800WSGIIZCXF1P572` — example valid LEI (check: always verify via GLEIF)
- Australian entities: register via GLEIF-accredited Local Operating Units

### Rust validation
```rust
pub fn validate_lei(lei: &str) -> Result<(), anyhow::Error> {
    anyhow::ensure!(lei.len() == 20, "LEI must be 20 characters");
    anyhow::ensure!(lei.chars().all(|c| c.is_alphanumeric()), "LEI alphanumeric only");
    // MOD 97-10 check omitted for brevity — use `lei` crate
    Ok(())
}
```

## ISO 4217 — Currency Codes

3-letter alphabetic codes. Crypto assets use non-standard codes (ISO 4217 only
covers fiat) — use `XBT` (BTC) and `XET` (ETH) as per emerging conventions, or
use custom `CryptoAsset` enum parallel to `Iso4217`.

```rust
pub enum Iso4217 {
    Aud,  // Australian Dollar
    Usd,  // US Dollar
    Gbp,  // Pound Sterling
}

pub enum CryptoAsset {
    Btc,
    Eth,
    Usdc,
    // ... platform-supported assets
}
```

## IFRS 9 — Financial Instruments

### Classification Relevant to Crypto
IFRS 9 classifies financial assets into three categories based on business model
and contractual cash flow characteristics:

| Category | IFRS 9 Class | Crypto applicability |
|---|---|---|
| Hold-to-collect (FCF) | Amortised Cost | Stablecoins as treasury |
| Hold-to-collect-and-sell | FVOCI | Long-term BTC/ETH holdings |
| Trading / other | FVTPL | Active trading wallets |

### ATO vs IFRS 9 Tension
The ATO treats crypto as **CGT assets** (not IFRS 9 financial instruments) for
income tax purposes (QC 53725). IFRS 9 governs financial *reporting* — these are
separate frameworks. The platform tracks both:

```rust
pub struct CryptoWallet {
    pub ato_treatment: AtoCryptoTreatment,   // CgtAsset | PersonalUseAsset | TradingStock
    pub ifrs9_class: Option<Ifrs9Class>,     // for financial reporting if applicable
}
```

# b00t:map v1
# summary: ISO 20022, IFRS 9, LEI, ISO 4217 — accounting standards for Tax-Lawyer Platform
# tags: iso, ifrs, accounting, lei, currency, financial
# tier: ch0nky
# complexity: 5
