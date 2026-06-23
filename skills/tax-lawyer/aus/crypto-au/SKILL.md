---
name: tax-lawyer-au-crypto
description: |
  Australian crypto tax treatment per ATO guidance QC 53725. Crypto assets
  are CGT assets under ITAA 1997 s 108-5. Each disposal triggers CGT event A1.
  Covers personal-use exception, trading stock test, and the 50% CGT discount.
version: 1.0.0
tags: [au, crypto, cgt, ato, tax, bitcoin, ethereum, defi, personal-use]
tier: ch0nky
complexity: 7
applies_to:
  - "crypto tax Australia"
  - "capital gains crypto"
  - "ATO crypto"
  - "Bitcoin CGT"
  - "DeFi tax"
  - "personal use asset"
  - "crypto CGT discount"
output_types: [.json, .md]
depends_on: [tax-lawyer-ufo-types, tax-lawyer-evidence-graph]
unlocks:
  - "ledgerr_tax_au_crypto_*"
metadata:
  ufo_stereotype: Perdurant
  satisfies_constraint: AuCryptoCgtRules
  legislation:
    - "Income Tax Assessment Act 1997 (ITAA 1997) s 108-5 (CGT asset)"
    - "ITAA 1997 s 100-5 (CGT event A1 — disposal)"
    - "ITAA 1997 s 118-5 (personal use asset exception)"
    - "ITAA 1997 s 115-25 (50% CGT discount)"
    - "ATO QC 53725 (Crypto assets — tax treatment guidance, 2023)"
  iso_types: ["ISO 4217"]
---

## ATO Crypto Asset Classification

:citation: ATO QC 53725; ITAA 1997 s 108-5

The ATO treats **crypto assets as CGT assets** — NOT:
- Foreign currency (ITAA 1997 Div 775 does NOT apply)
- Money or currency

**Each disposal of a crypto asset is a CGT Event A1** (ITAA 1997 s 104-10):
- Disposal = sale, trade, gift, use to purchase goods/services
- CGT event time = when contract for disposal entered (or when disposal occurs)

## CGT Calculation

### Capital Gain = Proceeds − Cost Base

**Proceeds**: AUD value at time of disposal (use market rate from reputable exchange)

**Cost Base**: Includes:
1. Initial AUD cost (what you paid including exchange fees)
2. Incidental costs (transfer fees, gas fees at acquisition)
3. Enhancement costs (nil for most crypto)
4. Costs of preserving title (nil for most crypto)
5. Costs of disposal (exchange fees, gas fees at disposal)

:citation: ITAA 1997 s 110-25

### Cost Base Tracking

The ATO does NOT mandate FIFO but requires **consistent identification**:
- Specific identification preferred for large holdings
- If not specifically identified: any reasonable method applied consistently
- FIFO is commonly used and accepted

:citation: ATO QC 53725 (Keeping records)

### 50% CGT Discount

:citation: ITAA 1997 s 115-25

If held for **≥ 12 months** before disposal:
- Australian resident individuals and trusts may apply 50% discount
- Companies: NO discount (0%)
- Net capital gain after discount included in assessable income

### CGT Event timing for DeFi

| Activity | CGT Event | Notes |
|---|---|---|
| Sell crypto for AUD | A1 (disposal) | Proceeds = AUD received |
| Trade BTC→ETH | A1 | Proceeds = AUD value of ETH received |
| Provide liquidity (DeFi) | A1 (potentially) | Receiving LP tokens may be disposal of input assets |
| Receive staking rewards | Ordinary income | Value at receipt; CGT on subsequent disposal |
| Receive airdrop | Ordinary income (if value), else nil | ATO QC 53725 s 'Airdrops' |
| NFT sale | A1 | CGT asset disposal |

## Personal Use Asset Exception

:citation: ITAA 1997 s 118-5; ATO QC 53725 s 'Personal use assets'

A CGT asset is a **personal use asset** if it is:
- Kept or used mainly for personal use/enjoyment (NOT to make profit)
- AND cost ≤ AUD 10,000

If personal use asset:
- Capital gains are **disregarded**
- Capital losses are **NOT allowed** (cannot offset other gains)

### Personal use test — fact patterns:
- **PASSES**: Used crypto wallet to buy coffee, games, minor online purchases; total crypto cost < AUD 10,000
- **FAILS**: Held BTC for 18 months then sold; this is investment
- **FAILS**: Crypto cost > AUD 10,000 even if used personally

### Satisfies<PersonalUseTest> checklist:
- [ ] Asset acquired and used mainly for personal use (NOT profit-making)
- [ ] Cost ≤ AUD 10,000
- [ ] Not held as part of trading stock

## Trading Stock Alternative

:citation: ITAA 1997 Div 70; ATO QC 53725 s 'Trading stock'

If crypto is **trading stock** (carrying on a business of trading crypto):
- CGT does NOT apply (trading stock rules apply instead)
- Opening/closing stock difference assessable as ordinary income
- Losses deductible as ordinary loss

Very few individuals qualify — must genuinely carry on a business.

## ATO Record-Keeping Requirements

:citation: ATO QC 53725; ITAA 1997 s 121-25

Must keep for **5 years** after disposal:
- Date of each transaction
- Value in AUD at time of transaction (with source/exchange)
- Nature of transaction
- Exchange/wallet addresses involved
- Receipts/transaction IDs

## CryptoTx Struct (Rust domain model)

```rust
pub struct CryptoTx {
    pub ufo: UfoCategory,               // Perdurant
    pub ufo_kind: PerdurantStereotype,  // Event
    pub tx_id: Uuid,
    pub tx_hash: String,                // on-chain tx hash
    pub wallet_id: Uuid,
    pub asset: CryptoAsset,
    pub tx_type: CryptoTxType,
    pub amount: rust_decimal::Decimal,
    pub aud_value: rust_decimal::Decimal, // at tx time
    pub timestamp: chrono::DateTime<Utc>,
    pub fee_aud: rust_decimal::Decimal,
}

pub enum CryptoTxType {
    Buy,          // acquire with AUD/fiat
    Sell,         // dispose for AUD/fiat — CGT A1
    Trade,        // crypto-to-crypto — CGT A1 on outgoing
    Transfer,     // wallet-to-wallet (same owner, NOT disposal)
    StakingReward,// ordinary income at receipt
    Airdrop,      // potentially ordinary income
    Defi,         // wrapped in DeFiEvent for full analysis
}
```

# b00t:map v1
# summary: ATO crypto CGT treatment — QC 53725, personal use, 50% discount, record keeping
# tags: au, crypto, cgt, ato, personal-use, staking, defi
# tier: ch0nky
# complexity: 7
