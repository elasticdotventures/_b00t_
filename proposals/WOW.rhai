// WOW — Way of Working (b00t hive standard)
// Import: b00t learn wow
//
// These are non-negotiable rules. Breaking them is an alignment failure.
//
// 1. FIX FIRST, FEATURES SECOND
//    Never add features on broken infrastructure. If `cargo build --features X`
//    fails, fix it before adding any feature that depends on X.
//    Working around a problem is NOT fixing it — it compounds debt.
//
// 2. BUILD VERIFICATION BEFORE MERGE
//    Every feature-gated build MUST compile before merge:
//      cargo build --features candle
//      cargo build --features llamacpp-fallback
//      cargo build --all-features
//    A feature that doesn't compile is a blocker, not a TODO.
//
// 3. DEPENDENCY VERSION STALENESS IS A BUG
//    If a pinned version (`0.4`) is 6+ major versions behind latest (`0.10`),
//    that IS a bug. Outdated deps accumulate incompatibilities.
//    Check: cargo search <crate>  →  compare with Cargo.toml
//    Update: bump to latest, fix breakage, verify build.
//
// 4. NO WORKAROUND PROPOSALS
//    A proposal that routes around broken infrastructure is rejected.
//    Rejected: "use llamacpp because candle doesn't build"
//    Accepted: "fix candle build, then add feature"
//
// 5. FIXES BEFORE DEPARTURE
//    Before ending a session, ensure all feature-gated builds compile.
//    If not, leave a clear LFMF + TODO + the exact error.
//
// 6. NAMING IS DEPLOYMENT
//    A polyseme (l3dg3rr/ledgerr-mcp/ledg3rr/ledgrrr) that maps proto→linux→cloud→windows
//    prevents fork drift. One codebase, platform-suffixed builds, trait-abstracted
//    platform differences (systemd vs docker for WSL, stdio vs gRPC for cloud).
//    Never name a crate just the proto name — encode the target.
//
// 7. TYPE INVARIANTS END AT FACTORY FUNCTIONS
//    A RoleRef<Worker> containing "executive" is a lie. The sealed trait + const NAME
//    pattern is only as strong as its public constructors. If resolve_role() bypasses
//    RoleRef::new(), the invariant is already broken. Use sealed sum types (KnownRole)
//    with exhaustive match dispatch instead of trying to polymorph a single type.
//
// 8. THE USER OWNS THE FILE SYSTEM
//    Before deleting a file, ask. Even if the user questions its value, the decision
//    to remove is theirs. "Is this necessary?" is an invitation to justify, not to delete.
//    Restore first, explain second, remove never without consent.
//
// 9. VENDOR IS UPSTREAM
//    A vendored crate at vendor/X/ IS the upstream for build purposes. Dockerfiles,
//    CI configs, and module justfiles belong inside the vendor tree, not in the
//    parent project. The parent justfile references vendor/X/ via mod + path.
//    This keeps the packaging boundary at the crate level.
//
// 10. DUAL RUNTIME DESIGN
//     Services must support both systemd AND docker (possibly others) as runtimes,
//     selected by environment detection (WSL → docker, Linux desktop → systemd,
//     container → podman). Store the preference in the datum, not hardcoded in scripts.
//     The runtime selection is a hive profile concern, not a justfile concern.
//
// 11. IN-CONTEXT LEARNING > FINE-TUNING
//     For small teams with constrained hardware (4GB VRAM), FSL examples + growing
//     context window beats framework churn. Burn/unsloth/candle training adds
//     dependency risk for marginal gain. Validate → save failures → include as
//     few-shot context on next call. The sm0l model improves passively without a
//     training pipeline. Only invest in fine-tuning when FSL exceeds ~500 examples.
//
// 12. JUST MODULES OVER FLAT RECIPES
//     A justfile with 65+ flat recipes is a discovery problem. Group commands into
//     modules (mod b00t, mod ledgrrr, mod irontology) with 3-8 related recipes each.
//     Invocation becomes `just module action` — self-documenting and scoped.
//     The module file lives in the vendor tree, not the parent justfile directory.
//
// 13. SEVEN IS THE MAGIC NUMBER
//     Recipe groups, enum variants, module exports — keep to 7±2 items per grouping.
//     The b00t module hit 46 recipes and became unwieldy. Sub-group within the module
//     file with clear section headers and consistent naming prefixes (pi-*, opencode-*).
//     If a module exceeds ~20 recipes, it needs sub-modules.

}
