# t00n — TOON format serialization skill

b00t's implementation of the TOON (Token-Oriented Object Notation) format.
Polyseme: `toon|t00n` — both refer to the same format.

Canonical spec: https://github.com/toon-format/spec (v3.0)

## Use
- `t00n encode <json>` — serialize JSON data to t00n format
- `t00n decode <t00n>` — parse t00n back to JSON
- `t00n validate <t00n>` — validate [N] row counts and structure

## Datum integration
If a `.tomllmd` has a `[schema]` section with `format = "t00n"`,
the datum serializes data in t00n for token-efficient LLM consumption.

```toml
# example tomllmd with t00n schema
[b00t.schema]
format = "t00n"
fields = ["BillingAccountId", "ServiceName", "BilledCost"]
spec = "https://github.com/toon-format/spec"
```

## reqif.yaml attribution
Every t00n output from the FOCUS ledger starts with:
```
# reqif.yaml: <schema-name>
```
This attributes the requirements schema used for contract validation.

## Validation
A sm0l model verifies that t00n-serialized FOCUS records fulfill
the requirements declared in the `# reqif.yaml` schema header.

<!-- b00t:map v1
summary: t00n — TOON format serializer/validator for FOCUS records, reqif.yaml attribution, sm0l validation gate
tags: toon, t00n, serialization, focus, reqif, validation, contract
tier: ch0nky
cmds: t00n encode, t00n validate
complexity: 4
-->
