---
workspace dependency pattern: When adding a new crate to a workspace, it needs three things: (1) added to [workspace].members in root Cargo.toml, (2) a [workspace.dependencies] entry for version sharing, and (3) the consuming crate uses k0mmand3r = { workspace = true }. Don't use path = \"../k0mmand3r\" directly — workspace deps keep version pinning centralized.
