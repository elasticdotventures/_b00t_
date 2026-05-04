// Systems thinking analysis of b00t WOW principles.
// Frameworks: TRIZ, MECE, INCOSE V, IDEO.
// Import: b00t learn wow --search "systems"

// ═══════════════════════════════════════════════════════════════════════════════
// MECE — Mutually Exclusive, Collectively Exhaustive
// ═══════════════════════════════════════════════════════════════════════════════
//
// The 13 WOW rules partition into 6 MECE categories:

// ── Category A: Build Integrity ─────────────────────────────────────────────
// Rules 1,2,3,5 — all about ensuring the build is healthy before proceeding.
//   A1: Fix First, Features Second   [rule 1]  — temporal ordering
//   A2: Feature Gates Must Compile   [rule 2]  — verification gate
//   A3: Dependency Staleness Is Bug  [rule 3]  — preventative maintenance
//   A5: Fixes Before Departure       [rule 5]  — session boundary
//   ANTI-PATTERN: "I'll fix it later" — later never comes. Enforce with CI.
//   CONTEXT EXPLOSION: Each unfixed feature gate doubles debugging surface.
//   REDUCTION: One CI command: cargo build --all-features. Fail fast.

// ── Category B: Type Invariants ─────────────────────────────────────────────
// Rule 7 — type safety at system boundaries.
//   B7: Factories Are the Last Line  [rule 7]
//   ANTI-PATTERN: Runtime checks disguised as type invariants.
//     RoleRef<Worker> with inner string "executive" IS a bug, not a feature.
//   CONTEXT EXPLOSION: Every call site must remember to check the string.
//   REDUCTION: Sealed sum type (KnownRole) makes illegal states unrepresentable.
//     Match exhaustiveness catches new variants at compile time.

// ── Category C: Boundary Discipline ─────────────────────────────────────────
// Rules 8,9 — where does the file/system boundary lie?
//   C8: User Owns The Filesystem     [rule 8]
//   C9: Vendor IS Upstream           [rule 9]
//   ANTI-PATTERN: Assuming ownership without verification.
//     "This file looks unused, I'll delete it" → user needed it.
//     "I'll put the Dockerfile here" → vendor should have it.
//   CONTEXT EXPLOSION: Files scattered across owner boundaries create
//     merge conflicts, stale copies, and permission confusion.
//   REDUCTION: Explicit boundary contracts. Every file asks: "who owns me?"
//     Datums declare provenance. Vendor dirs declare upstream URL.

// ── Category D: Deployment Topology ─────────────────────────────────────────
// Rules 6,10 — how the system maps to physical/logical infrastructure.
//   D6: Naming Is Deployment         [rule 6]  — polyseme = platform map
//   D10: Dual Runtime Design         [rule 10] — environment detection
//   ANTI-PATTERN: Hardcoding infrastructure assumptions.
//     "systemctl --user" fails silently in WSL. Assumed Linux desktop.
//   CONTEXT EXPLOSION: Every platform-specific workaround multiplies
//     the state space. WSL + systemd + docker + native = 4x test matrix.
//   REDUCTION: Datum-declared runtime detection (detect_order).
//     One code path dispatches to systemd/docker/podman based on env.
//     The hive profile chooses; the implementation just calls the active backend.

// ── Category E: Design Heuristics ───────────────────────────────────────────
// Rules 11,12,13 — empirical observations about what scales.
//   E11: ICL > Fine-Tuning           [rule 11] — FSL context over framework
//   E12: Just Modules Over Flat      [rule 12] — grouping for discoverability
//   E13: 7±2 Per Group               [rule 13] — Miller's Law applied to CLIs
//   ANTI-PATTERN: Solving scale problems with new frameworks.
//     "Too many commands" → add a categorizer. "Model too weak" → fine-tune.
//     Each new framework doubles the dependency graph.
//   CONTEXT EXPLOSION: Every new abstraction layer compounds learning curve.
//   REDUCTION: Miller's Law — 7±2 items per grouping. If a module exceeds 20:
//     split into sub-modules. If FSL < 500: don't fine-tune.

// ── Category F: Session Hygiene ─────────────────────────────────────────────
// Rule 4 — the meta-rule about rules themselves.
//   F4: No Workaround Proposals      [rule 4]
//   This is distinct from build integrity (A) because it governs DESIGN
//   DECISIONS, not build state. A workaround is a design choice that
//   routes around known-broken infrastructure.

// ═══════════════════════════════════════════════════════════════════════════════
// TRIZ — Contradiction Resolution
// ═══════════════════════════════════════════════════════════════════════════════
//
// TRIZ teaches that innovation requires resolving contradictions.
// Here are the contradictions embedded in the WOW rules and their resolution:

// ── Contradiction 1: Speed vs Quality ───────────────────────────────────────
// Parameter: "I want to ship features fast" vs "I need the build to work"
// TRIZ Principle: Separation in TIME
//   Resolution: Fix first (build integrity), THEN add features (innovation).
//   The "fix first" rule is temporal separation — two distinct phases.
//   Applied: candle 0.4→0.10 before any model training features.

// ── Contradiction 2: Expressiveness vs Safety ──────────────────────────────
// Parameter: "I want runtime flexibility" vs "I need compile-time guarantees"
// TRIZ Principle: Separation in STRUCTURE
//   Resolution: Sealed sum types (KnownRole) for known cases,
//   dynamic dispatch (String) only at the system boundary.
//   The type invariant is maintained inside the boundary;
//   the boundary factory converts external strings to internal types.
//   Applied: resolve_role() returns KnownRole. RoleRef<Worker> never holds "executive".

// ── Contradiction 3: Discoverability vs Brevity ────────────────────────────
// Parameter: "I want few top-level commands" vs "I need many distinct actions"
// TRIZ Principle: Separation in SPACE
//   Resolution: Group commands into modules (space separation).
//   The "just module action" pattern creates a namespace hierarchy.
//   Miller's Law (7±2) governs module size. Modules > 20 split.
//   Applied: justfile from 65 flat recipes → 16 modules × ~5 recipes each.

// ── Contradiction 4: Portability vs Specialization ─────────────────────────
// Parameter: "I want one codebase" vs "I need platform-specific behavior"
// TRIZ Principle: Local Quality + Merging
//   Resolution: Polyseme naming (same code, different entry points).
//   The platform-specific behavior is in the datum (detect_order),
//   not in the code. One binary, multiple personalities.
//   Applied: ledgrrr/ledgerr-mcp/ledg3rr/l3dg3rr = same crate, different targets.

// ── Contradiction 5: Richness vs Maintenance ───────────────────────────────
// Parameter: "I want powerful model adaptation" vs "I need zero dependencies"
// TRIZ Principle: Preliminary Action + Universality
//   Resolution: In-context learning (FSL) before fine-tuning.
//   The "preliminary action" is collecting validation failures as examples.
//   The "universality" is using the same sm0l model for both inference and
//   adaptation — no second system needed.
//   Applied: validate --auto-train saves FSL examples. model train only when > threshold.

// ═══════════════════════════════════════════════════════════════════════════════
// INCOSE V-Model — Bridging Requirements to Verification
// ═══════════════════════════════════════════════════════════════════════════════
//
// The V model connects requirements (left leg) to verification (right leg)
// through architecture and detailed design (bottom).
//
// ── Left Leg (Decomposition) ────────────────────────────────────────────────
//
// Level 1 — Stakeholder Requirements (Operator)
//   "Commands must be discoverable"  →  just modules (E12)
//   "The system must not lie about types"  →  sealed sum types (B7)
//   "Build must not be broken"  →  feature gates compile (A2)
//   "Must run on WSL and Linux"  →  dual runtime (D10)
//
// Level 2 — System Requirements (Architect)
//   Module organization → 7±2 per group (E13)
//   Type safety → factory functions validate (B7)
//   Platform portability → datum-declared detect_order (D10)
//   Model adaptation → FSL first, fine-tune later (E11)
//
// Level 3 — Component Requirements (Engineer)
//   KnownRole enum with exhaustive match
//   justfile mod statements
//   b00t.providers.runtime.detect_order in datum
//   validate --auto-train saves to .b00t/fsl/
//
// ── Bottom (Implementation) ─────────────────────────────────────────────────
//   agentic_role.rs → KnownRole enum
//   justfile → mod b00t, mod ledgrrr, mod irontology
//   ledgrrr.cli.toml → [b00t.providers.runtime]
//   validate.rs → FSL store + auto-train trigger
//   experiment.rs → model_endpoint + adapter fields
//
// ── Right Leg (Integration & Verification) ──────────────────────────────────
//
// Level 3 — Unit Test
//   cargo test --lib -- agentic_role::tests  (KnownRole exhaustiveness)
//   cargo test --lib -- datum_schema::tests   (FocusSchema load)
//   cargo test --lib -- commands::validate::tests  (FSL, auto-train)
//
// Level 2 — Integration Test
//   cargo build --features candle  (feature gate compiles)
//   just --list  (module discovery works)
//   just b00t pi-status  (module invocation)
//
// Level 1 — Acceptance Test
//   System runs on WSL (docker runtime selected)
//   System runs on Linux desktop (systemd selected)
//   just test-schema-drift  (CI gate catches drift)
//
// ── V-Model Gap Analysis ────────────────────────────────────────────────────
//
// MISSING on right leg: Acceptance tests for:
//   - Dual runtime selection (WSL vs Linux desktop)
//   - FSL example store growth (validate → file exists)
//   - polyseme build targets (ledgrrr vs ledgerr-mcp)
// These should be added as integration tests.

// ═══════════════════════════════════════════════════════════════════════════════
// IDEO — Human-Centered Design Process
// ═══════════════════════════════════════════════════════════════════════════════
//
// The session itself followed the IDEO process organically:

// ── EMPATHIZE ───────────────────────────────────────────────────────────────
// Observed pain points:
//   "this file contains an unclosed delimiter"  →  Rust compiler frustration
//   "you better take all that stuff you just removed"  →  ownership violation
//   "the number of top level b00t commands proposed is WAYYYY to many"  →  cognitive overload
//   "systemctl --user restart failed"  →  platform mismatch
//   "Candle build still failing after all that"  →  broken trust in tooling
//
// ── DEFINE ──────────────────────────────────────────────────────────────────
// Point of view statements:
//   "The operator needs confidence that the build isn't broken BEFORE adding features."
//   "The operator needs commands grouped by function, not a flat list of 65."
//   "The operator needs the same workflow on WSL and Linux without thinking about it."
//   "The operator needs type safety at API boundaries, not runtime checks disguised as types."
//
// ── IDEATE ──────────────────────────────────────────────────────────────────
// Generated solutions (all implemented):
//   - Fix candle first (bump 0.4→0.10)
//   - Just modules (mod b00t, mod ledgrrr, mod irontology)
//   - Dual runtime datum (b00t.providers.runtime.detect_order)
//   - KnownRole sealed enum
//   - FSL in-context learning
//
// ── PROTOTYPE ───────────────────────────────────────────────────────────────
// Rapid iteration cycle (< 5 min per prototype):
//   1. Edit file → cargo check
//   2. just --list → verify grouping
//   3. b00t-cli validate --help → verify CLI
//   4. cargo test --lib → verify behavior
//
// ── TEST ────────────────────────────────────────────────────────────────────
// Verification gates:
//   cargo check --features candle  →  build integrity
//   just --list  →  module discoverability
//   just b00t pi-status  →  module invocation
//   cargo test --lib -- datum_schema::tests  →  schema loading
//   cargo test --lib -- experiment::tests  →  experiment scoring

// ═══════════════════════════════════════════════════════════════════════════════
// Context Reduction Map
// ═══════════════════════════════════════════════════════════════════════════════
//
// Each anti-pattern directly maps to a context explosion source
// and the WOW rule that curbs it:

// ── Context Explosion Source 1: Broken Infrastructure ──────────────────────
//   Every unfixed build issue lives in working memory.
//     "Don't use --features candle, it doesn't compile"
//     "Use the workaround, not the real path"
//   REDUCTION: Fix first (rule A1). If it compiles, it's one less thing to remember.
//   MEASURE: Unfixed feature gates → count of "don't use" comments in code.

// ── Context Explosion Source 2: Flat Namespaces ─────────────────────────────
//   65 recipes in one help output → mental scrolling.
//     "I know it's here somewhere..."
//   REDUCTION: Module grouping (rule E12). `just b00t` shows 46 related commands.
//   MEASURE: Commands per module. 7±2 is ideal. >20 needs sub-modules.

// ── Context Explosion Source 3: Platform Assumptions ────────────────────────
//   "systemctl --user works everywhere" → N hours debugging WSL.
//     "Oh right, this doesn't have systemd"
//   REDUCTION: Dual runtime (rule D10). The datum encodes the detection order.
//   MEASURE: Platform-specific #ifdefs → count of runtime-detection calls.

// ── Context Explosion Source 4: Runtime Type Checks ─────────────────────────
//   "RoleRef<Worker> might hold 'executive', check before using"
//     Every call site needs the same guard.
//   REDUCTION: Sealed sum types (rule B7). The compiler enforces exhaustiveness.
//   MEASURE: match arms vs if-let chains. Match = safe. If-let = possible hole.

// ── Context Explosion Source 5: Framework Churn ─────────────────────────────
//   "We need unsloth for training. No wait, Burn. No wait, candle."
//     Each framework evaluated means reading docs, resolving deps, debugging builds.
//   REDUCTION: FSL first (rule E11). < 500 examples? Don't train. Use context.
//   MEASURE: FSL example count. Training flag only activates above threshold.

// b00t:map v1
// summary: Systems thinking analysis of 13 WOW rules — MECE categories, TRIZ contradictions, INCOSE V verification gaps, IDEO process map, context reduction sources
// tags: wow, triz, mece, incose, ideo, systems-thinking, context-reduction, anti-patterns
// tier: frontier
// cmds: b00t learn wow, b00t grok ask "context reduction" --topic wow
// complexity: 9
