# ── ufo-types consumer compatibility — UFO stereotypes + Satisfies<T> ───────
# 🤓 ufo-types now lives at github.com/PromptExecution/ufo-types. Its own CI
#    runs the standalone test suite; these recipes validate _b00t_'s consumers.
# Part of Tax-Lawyer EPIC (#510). ufo-types is the ontological foundation:
#    UFO stereotypes (Kind/SubKind/Role/Relator/Mode) + Satisfies<T> constraint trait
#    + ISO wrappers (Lei, Iso4217, Ifrs9Classification) + DARED governance types.
# Extracted from root justfile for module isolation.

# Fast compile-check for both in-repo ufo-types consumers
ufo-check:
    cargo check -p b00t-c0re-lib -p b00t-chat

# Run consumer tests against the pinned external ufo-types revision
ufo-test:
    cargo test -p b00t-c0re-lib --lib ai_artifact::tests
    cargo test -p b00t-chat --lib gossip::tests
    cargo test -p b00t-chat --lib ipc_transport::tests
    cargo test -p b00t-chat --lib mesh::tests

# Build and open consumer docs with links to ufo-types APIs
ufo-doc:
    cargo doc --no-deps -p b00t-c0re-lib -p b00t-chat --open

# Continuous watch: re-check both consumers on file changes
ufo-watch:
    cargo watch -x 'check -p b00t-c0re-lib -p b00t-chat'

# Full Tax-Lawyer test suite (ufo-types consumers + ledgrrr)
tax-lawyer-test: ufo-test
    just ledgrrr test
