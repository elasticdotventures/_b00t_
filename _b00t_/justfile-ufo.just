# ── ufo-types crate (#511) — Tax-Lawyer UFO stereotypes + Satisfies<T> ──────
# 🤓 Part of Tax-Lawyer EPIC (#510). ufo-types is the ontological foundation:
#    UFO stereotypes (Kind/SubKind/Role/Relator/Mode) + Satisfies<T> constraint trait
#    + ISO wrappers (Lei, Iso4217, Ifrs9Classification) + DARED governance types.
# Extracted from root justfile for module isolation.

# Fast compile-check for ufo-types crate
ufo-check:
    cargo check -p ufo-types

# Run ufo-types tests (78 tests, <2s)
ufo-test:
    cargo test -p ufo-types

# Open ufo-types docs in browser
ufo-doc:
    cargo doc -p ufo-types --open

# Continuous watch: re-check ufo-types on file changes
ufo-watch:
    cargo watch -x 'check -p ufo-types'

# Full Tax-Lawyer test suite (ufo-types + ledgrrr)
tax-lawyer-test: ufo-test
    just ledgrrr test
