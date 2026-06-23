---
name: tax-lawyer-us-crypto
description: |
  US crypto tax treatment under IRS guidance. Crypto as property (Notice 2014-21).
  Cost basis methods under Rev. Proc. 2024-28 safe harbor. Capital gains/losses,
  Form 8949/Schedule D, staking income, mining, and DeFi considerations.
version: 1.0.0
tags: [us, crypto, property, irs, cost-basis, form8949, rev-proc-2024-28, capital-gains]
tier: ch0nky
complexity: 7
applies_to:
  - "crypto tax US"
  - "IRS crypto"
  - "Bitcoin tax"
  - "cost basis method"
  - "Rev Proc 2024-28"
  - "Form 8949"
  - "crypto capital gains"
output_types: [.json, .md]
depends_on: [tax-lawyer-ufo-types, tax-lawyer-evidence-graph]
unlocks:
  - "ledgerr_tax_us_crypto_*"
metadata:
  ufo_stereotype: Perdurant
  satisfies_constraint: CryptoCostBasisRules
  legislation:
    - "IRS Notice 2014-21 (crypto as property)"
    - "IRS Rev. Proc. 2024-28 (cost basis safe harbor)"
    - "IRC Section 1001 (recognition of gains/losses)"
    - "IRC Section 1221 (capital asset definition)"
    - "IRC Section 1091 (wash sale — does NOT apply to crypto)"
    - "IRC Section 61 (gross income — staking/mining)"
    - "Form 8949 (Sales and Other Dispositions of Capital Assets)"
    - "Schedule D (Capital Gains and Losses)"
  iso_types: ["ISO 4217"]
---

## IRS Classification: Crypto as Property

:citation: IRS Notice 2014-21; IRC § 1221

The IRS treats virtual currency as **property** for US federal tax purposes.
This is NOT classified as:
- Currency (no IRC § 988 treatment)
- Security (no wash sale rule under IRC § 1091 — though proposed legislation may change this)
- Commodity

**Every disposition of crypto is a taxable event**:
- Sale for USD
- Trade of crypto-for-crypto (both legs are taxable)
- Use to purchase goods/services
- Gift above annual exclusion ($18,000 for 2024)

## Capital Gains/Losses

:citation: IRC §§ 1001, 1221, 1222

**Gain/Loss = Amount Realized − Adjusted Basis**

| Holding Period | Treatment | Rate (2024) |
|---|---|---|
| ≤ 12 months | **Short-term capital gain** | Ordinary income rates (10–37%) |
| > 12 months | **Long-term capital gain** | Preferential rates (0%, 15%, 20%) |
| Any period, net loss | **Capital loss** | Deductible up to $3,000/yr against ordinary income; excess carries forward |

**No wash sale rule**: Unlike stocks, you can sell crypto at a loss and immediately
repurchase the same asset and still claim the loss. (Proposed legislation in 2025
may end this — monitor legislative status.)

## Cost Basis Methods

:citation: Rev. Proc. 2024-28

### Available methods

| Method | How Basis Is Assigned | IRS Status |
|---|---|---|
| **FIFO** | First purchased = first sold | Safe harbor default if no specific ID |
| **HIFO** | Highest cost first sold | Available for specific identification |
| **LIFO** | Last purchased = first sold | Available but unusual |
| **SpecID** | Specifically identify units being sold | Gold standard; requires exchange support |

### Rev. Proc. 2024-28 Safe Harbor

:citation: Rev. Proc. 2024-28, Section 4

Released January 2024. Establishes a safe harbor for taxpayers to **allocate
unused basis** to wallets after the IRS clarified in 2022 that cost basis must
be tracked per wallet (not pooled across all wallets).

**Safe harbor**: Taxpayers who previously pooled basis may allocate to specific
wallets using any reasonable method — as long as allocation is made **by the end
of 2024** and applied consistently going forward.

**Platform implementation**: Per `CryptoWallet` — each wallet maintains its own
FIFO/HIFO/LIFO/SpecID cost basis lot queue.

## Form 8949 and Schedule D

:citation: Form 8949; Schedule D

### Form 8949 — per transaction
Each disposal reported with:
- Description (e.g., "0.5 BTC")
- Date acquired
- Date sold
- Proceeds (USD at sale)
- Cost basis (USD at acquisition)
- Gain/loss
- Adjustments (Box B/E codes for wash sales, etc.)

Groups:
- **Part I**: Short-term (≤ 12 months)
- **Part II**: Long-term (> 12 months)

### Schedule D
Aggregates Form 8949 totals. Net capital gain flows to Form 1040 Line 7.

## Staking and Mining Income

:citation: IRS Notice 2014-21; Rev. Rul. 2023-14

### Mining
- Mined crypto = **ordinary income** at fair market value at time of receipt
- Cost basis = FMV at receipt
- Self-employment tax may apply if mining constitutes a trade/business

### Staking Rewards
:citation: Rev. Rul. 2023-14 (July 2023)

IRS ruled: Staking rewards are **ordinary income** when received, at FMV.
- This settled the Jarrett vs United States dispute (previously unclear)
- Cost basis of received tokens = FMV at receipt
- Subsequent sale of staking rewards is capital gains/loss event

### DeFi / Liquidity Mining
- Providing liquidity: depositing crypto into LP → receipt of LP tokens
  may constitute a **taxable exchange** (gain/loss on deposited assets)
- Receiving LP rewards: ordinary income at FMV
- Withdrawing from LP: exchange of LP tokens back → taxable if basis differs

## Airdrops and Hard Forks

:citation: IRS Notice 2014-21 FAQ (2019)

- **Airdrop**: Ordinary income at FMV if received in exchange for services or as
  promotional distribution. Pure airdrops (no services): IRS position is ordinary income.
- **Hard fork**: If taxpayer receives new crypto as result of hard fork → ordinary income
  at FMV of new token when received and recorded in the new blockchain.

## Wash Sale Note (Important)

:citation: IRC § 1091

The wash sale rule (30-day window) does **NOT** currently apply to crypto.
A taxpayer may sell BTC at a loss on December 31 and repurchase on January 1
and claim the full loss. Congress has proposed applying wash sale rules to crypto
— monitor IRC changes.

## CryptoTx + CryptoCostBasisRules (Rust domain model)

```rust
pub struct CryptoTx {
    pub ufo: UfoCategory,               // Perdurant
    pub ufo_kind: PerdurantStereotype,  // Event
    pub tx_id: Uuid,
    pub wallet_id: Uuid,
    pub asset: CryptoAsset,
    pub tx_type: UsCryptoTxType,
    pub amount: rust_decimal::Decimal,
    pub usd_value: rust_decimal::Decimal, // at tx time
    pub timestamp: chrono::DateTime<Utc>,
    pub fee_usd: rust_decimal::Decimal,
}

pub enum UsCryptoTxType {
    Buy,              // acquire — establishes basis
    Sell,             // dispose — recognizes gain/loss
    Trade,            // crypto-to-crypto: both legs are taxable
    StakingReward,    // ordinary income at FMV
    MiningReward,     // ordinary income at FMV
    Airdrop,          // ordinary income (IRS position)
    HardFork,         // ordinary income at FMV
    DeFiDeposit,      // potentially taxable exchange
    DeFiWithdraw,     // potentially taxable exchange
    Transfer,         // wallet-to-wallet (same owner, NOT taxable)
}

pub struct CryptoCostBasisRules {
    pub method: CostBasisMethod,
    pub jurisdiction: Jurisdiction,
}

pub enum CostBasisMethod { Fifo, Hifo, Lifo, SpecId }
```

# b00t:map v1
# summary: IRS crypto-as-property — cost basis, Form 8949, staking income, Rev Proc 2024-28
# tags: us, crypto, irs, cost-basis, form8949, capital-gains, staking
# tier: ch0nky
# complexity: 7
