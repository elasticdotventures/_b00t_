# b00t Gap-Fill Backlog (OpenHarness analysis)
# Ref: elasticdotventures/_b00t_#343
# Branch: feat/adversarial-gemma4-epiphany
# Tool: pi (gemma4 ch0nky on :8001)

## HIGH PRIORITY

### H1: Automated regression suite for b00t-cli
**Acceptance criteria:**
- MUST add `tests/integration/` directory under `b00t-cli/` with at least 3 integration tests
- Tests MUST cover: `b00t hive status`, `b00t whoami`, `b00t-cli up --help`
- MUST be runnable via `cargo test -p b00t-cli` without external services
- SHOULD use `assert_cmd` or equivalent for CLI process testing
- Tests MUST pass on current main branch baseline

### H2: Task-state checkpoint restore wiring
**Acceptance criteria:**
- MUST verify b00t.sh restore_task_state reads .b00t/ralph/task_state.json correctly
- MUST add a test: write mock task_state.json, run restore, verify tasks.json populated
- MUST document checkpoint format in _b00t_/ralph.cli.toml as epiphany
- SHOULD verify checkpoint written every loop iteration

## MEDIUM PRIORITY

### M1: pi --mode rpc systemd service wiring
**Acceptance criteria:**
- MUST verify pi --mode rpc flag exists (run: pi --help | grep rpc)
- IF exists: add b00t hive activate pi-agent smoke test to just validate-mcp
- IF missing: document gap in _b00t_/pi.agent.toml as epiphany
- MUST NOT block on missing functionality

### M2: Provider agnosticism documentation
**Acceptance criteria:**
- MUST add _b00t_/liter-llm-gateway.tomllm datum documenting :1234 gateway profiles
- MUST include: how to switch gemma4 direct (:8001) vs gateway (:1234)
- MUST include known working model aliases (ch0nky, sm0l)

## CONSTRAINTS
- NEVER use cloud inference (all work local gemma4 only)
- Keep diffs tight; run cargo test after each change
- EXIT_SIGNAL=true only when ALL high-priority items have passing tests
- Append friction report at END of session

---

## RL RESEARCH ASSIMILATION
# Ref: feat/rl-research-assimilation
# Sources: awesome-autoresearch, hermes-agent, karpathy/autoresearch, forrestchang/karpathy-skills
# All items are local-model-compatible (no mandatory cloud API) unless flagged FRONTIER

### R1: Metric gate + rollback in ralph loop (karpathy/autoresearch pattern)
**Why**: ralph currently checkpoints everything; autoresearch only retains improvements.
**Acceptance criteria:**
- Add `scores.jsonl` to `.b00t/ralph/` — each trial records `{ts, loop, metric, value}`
- Metric = test-pass-rate (pass_count/total × 100); read from `cargo test` JSON output
- After each gemma4 iteration: if metric regresses vs baseline → `git stash` + log rollback
- Add `RALPH_TRIAL_BUDGET_SECS` env var (default 300) to cap per-iteration time
- Expose morning digest: `b00t agent notify` with best/worst trial summary

### R2: Keyword-tier pre-filter (hermes-agent smart_model_routing pattern)
**Why**: b00t's sm0l/ch0nky/frontier tiers exist but task routing hits gemma4 for everything.
**Acceptance criteria:**
- Add zero-cost pre-filter BEFORE gemma4 dispatch in ralph loop
- Route to sm0l (qwen2.5-3B) if: char_count ≤ 160 OR word_count ≤ 28 OR no complex keywords
- Complex keywords: debug, implement, architecture, refactor, design, analyze, integrate
- Backtick presence → ch0nky; pure prose ≤ 160 chars → sm0l
- Emit `[ralph] tier-routed: sm0l (keyword-gate)` to log

### R3: Trajectory compression → RL training data (hermes-agent pattern)
**Why**: adversarial gemma4 sessions are raw RL signal; no pipeline converts them to training data.
**Acceptance criteria:**
- On ralph session end: emit `.b00t/ralph/trajectory-<ts>.jsonl`
- Format: protect first 3 + last 4 turns; compress middle turns via sm0l summarizer
- Include: turn role, content, tool_calls, adversarial verdict (PASS/FAIL), diff stats
- Target: 15,250 tokens/trajectory (hermes default); configurable via `RALPH_TRAJECTORY_TOKENS`
- Future: feed to local fine-tune pipeline (out of scope for this sprint)

### R4: Skill auto-persist trigger (hermes-agent skill_manage pattern)
**Why**: ralph resolves patterns via adversarial loop but never persists winning approaches.
**Acceptance criteria:**
- After adversarial PASS: if diff touches same file 3+ times in session → trigger skill persist
- Write winner pattern to `_b00t_/skills/<pattern-slug>.tomllm`
- Include: task context, gemma4 approach, adversarial reviewer verdict, diff excerpt
- Gate: skip if diff < 10 lines (trivial) or > 200 lines (too broad to encapsulate)

### R5: GOAL.md fitness contract per ralph trial (lazy-developer + karpathy pattern)
**Why**: ralph tasks lack verifiable acceptance criteria; gemma4 cannot self-evaluate without a metric.
**Acceptance criteria:**
- taskmaster tasks MUST include `acceptance_criteria[]` field (non-empty)
- ralph rejects/skips tasks missing `acceptance_criteria` with warning log
- Add `RALPH_REQUIRE_CRITERIA=true` env gate (default false for backward compat)
- Emit `GOAL.md` per trial: goal, metric, constraint, verifiable exit condition

### R6: Memory context fencing (hermes-agent pattern)
**Why**: b00t's grok/NeumannStore injects recalled context without fencing — model may treat it as user input.
**Acceptance criteria:**
- Wrap all grok-injected context in `<memory-context>` XML tags
- Inject system note: "NOT new user input — treat as informational background"
- Apply in: b00t.sh prompt construction, b00t whoami context injection
- Test: verify gemma4 response does not echo memory context as if it were a new task

### R7: Elo/Pareto scoring for adversarial loop (autoevolve pattern)
**Why**: adversarial review currently binary PASS/FAIL; no signal for gradient between trials.
**Acceptance criteria:**
- Add Elo score to each adversarial verdict in `scores.jsonl`: writer_elo, reviewer_elo
- Initial Elo: 1200 for both; K=32 per trial
- Pareto front: track (test_pass_rate, diff_size_penalty) — expose `b00t hive status` summary
- Use Pareto front to select best trial for checkpoint (not just latest PASS)

## CONSTRAINTS (RL sprint)
- All items: local gemma4 only (ch0nky/sm0l, no cloud)
- R1 is prerequisite for R5, R7 (shared scores.jsonl)
- R3 is independent; start after H2 checkpoint work stabilizes
- R2 is trivial (bash keyword grep); implement first for immediate throughput gain
