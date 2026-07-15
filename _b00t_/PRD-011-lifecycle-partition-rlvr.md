# PRD-011: Lifecycle Partition & Deterministic Reward Channel (RLVR Gap Closure)

**Status:** In progress — P2-G5 running, W1 shipped-on-branch pending operator review
**Date:** 2026-07-15
**Priority:** P0 (foundation for the training loop)
**Task queue:** #123–#146 (`b00t task list -t prd-011`) · Branch: `task/prd-011-lifecycle-partition`

## 1. Problem Statement

The fine-tune corpus is generated from the entire `_b00t_` store — four file formats,
live config, dead PRDs, aspirational gospel, and backup files at equal trust — so the
adapter trains on b00t's own contradictions. The A/B experiment scoring that would feed
any reinforcement loop is heuristic (hash-derived accuracy, length-ratio ROI): training
on it optimizes noise. The role→skill graph has dangling refs (9 of 10 skills declared
by `worker`/`ai-wizard` have no datum), so blessing-based authorization is unenforceable.
And the wake boilerplate spends ~200 lines on every agent regardless of tier, while not
requiring the one behavior that makes transcripts valuable: verified execution.

**Reward doctrine (non-negotiable):** a reinforcement reward MUST be deterministically
linked to a script whose outcome is independently observable and objectively measurable
without an LLM — exit code + evidence regex. Where a task is irreducibly semantic, the
fallback is handoff to a reviewer in an independent context (fresh agent, structured
forced-choice rubric), recorded as grade-B evidence, never mixed with grade-A script
evidence.

**Corpus doctrine (this PRD's second law):** trace-grounded rows over templated rows.
A row generated because a command actually ran and its contract PASSed is worth fifty
rows of "How do I load a b00t skill?" boilerplate. lfmf entries and service-contract
evidence strings are the highest-value rows in the store; templated Q&A is filler that
teaches phrasing, not behavior.

## 2. End State (work backwards from here)

Invariant loop: batch processor pulls contract-bearing tasks from `b00t task` →
dispatches variants → the contract's handler script runs → PASS/FAIL from exit code +
evidence regex (no LLM in the reward path) → EvidenceRecord appended to the unified
evidence spine → PASS transcripts become SFT rows, PASS/FAIL pairs on the same task
become DPO pairs → fine-tune → held-out probe gate → promoted adapter serves the sm0l
tier → which scores/routes the next generation. Terminal DoD rolls up to mission **#114**
(native-b00t ch0nky).

Backward chain of preconditions:

```
loop (P5) ⇐ trace-grounded dataset (P4) ⇐ deterministic reward channel (P3)
         ⇐ clean corpus: lifecycle-partitioned + single-format (P2)
         ⇐ validated datum graph (P1) ⇐ measured baseline (P0)
   wake workstream (W) runs parallel: agents must EMIT traces for P4 to harvest
```

Dogfood-forward rule: every phase ships a runnable artifact that b00t itself uses the
next day, and every phase's acceptance is itself a deterministic script (each gap
becomes a `[[service_contract]]` once P3-G10 lands).

## 3. Handoff Review Protocol

Applies at every phase boundary and to every gap marked **[review]**.

1. **Implementer never reviews their own gap.**
2. Reviewer runs in an **independent context**: fresh agent session
   (`b00t whoami --role=reviewer`), receiving ONLY: the gap list for the phase, each
   gap's DoD command, the captured evidence lines, and the diff. No implementation
   conversation history.
3. Verdict is **forced-choice**: `SHIP` or `BLOCK:<gap>:<reason ≤2 lines>`. No essays.
4. Verdict is recorded as **grade-B evidence** (task note + evidence spine once G10
   exists). Grade-A (script) evidence and grade-B (reviewer) evidence never aggregate
   into one number.
5. **Operator approval is additionally required** for: G6 (labels folding into datums —
   mass mechanical edit), every W gap (wake changes touch every session's context), and
   G17 promotion (first adapter generation).

## 4. Phases, Gaps, Definitions of Done

Format per gap: rationale → required → **DoD** (deterministic command where possible) →
task#.

### Phase 0 — Baseline & guardrails

**G0: soul substrate integrity** (#125, P1, [review])
*Rationale:* `b00t soul set` rewrites `SOUL.tomllm` and silently drops all DataFramerr
tables + cursors ([#830](https://github.com/elasticdotventures/_b00t_/issues/830)) —
data loss in the flashtable substrate everything in P2 sits on. Interim doctrine
(lfmf: soul-dataframerr): all K/V writes ordered BEFORE `table-create`.
*Required:* `soul set` round-trips the full document (or K/V and tables split files).
**DoD:** `b00t-cli soul table-create t a:text && b00t-cli soul set k v && b00t-cli soul
table-list | grep -q '^t '` exits 0; regression test in b00t-cli.

**G0b: node soul orientation** (#126, [shipped: `lifecycle.just soul-node-refresh`])
*Rationale:* the whoami node preamble (`compose_node_summary`, anchor `node.board`) was
coded but starving — no `node.*` keys existed. Operators and agents need one-line
orientation to their node; `b00t-server-soul.tomllm` drift (records llama.cpp:8080 /
Qwen3.5-0.8B; reality ch0nky:8001 / qwen3.6-35B-A3B) shows why hand-recorded state rots.
*Required:* node soul derived from observation (hostname, DMI, free, nvidia-smi,
podman ps; k8s later), re-runnable; stale `soul.services` sections regenerated or marked.
**DoD:** on a node with empty soul, `just -f _b00t_/lifecycle.just soul-node-refresh &&
b00t whoami | grep -q '🥾 Node:'` exits 0.

**G1: metrics snapshot** (#123)
*Rationale:* can't claim any phase improved anything without a before/after number.
*Required:* `just b00t-metrics` → JSON: datum counts by extension, dangling graph refs,
`train.jsonl` rows by provenance, adapter probe score.
**DoD:** `just b00t-metrics | jq -e '.datums and .train_rows'` exits 0; snapshot
committed per phase close.

**G2: whoami hard-error on unknown role** (#124)
*Rationale:* silent fallback to `worker` puts agents under the wrong compliance set —
observed live (`--role=agentic-efficiency` → worker). Same defect family as red wow
check `B:TypeInvariant` (KnownRole exhaustive) and MCP whoami gap #94.
*Required:* unknown `--role` exits nonzero listing valid roles.
**DoD:** `! b00t whoami --role=nonexistent-role-xyz` succeeds AND
`b00t wow check` B:TypeInvariant passes.

### Phase 1 — Graph integrity

**G3: `b00t datum validate --graph`** (#127, P1)
*Rationale:* an authorization graph you can't validate is documentation; blessing-gated
tool auth ("no learning = no auth") is unenforceable with dangling edges. Also the
prerequisite for ever moving files (path refs become enumerable).
*Required:* resolve every `skills[]`, `depends_on[]`, `entangled_*[]` ref across all
datums; nonzero exit on dangling; wired to CI + pre-commit. Composes with existing
`b00t-lsp --check` (84 datum errors already known, #115) rather than duplicating it.
**DoD:** seeded dangling ref → exit 1 naming it; clean store → exit 0; CI job green.

**G4: close the 9 dangling role skills** (#128, blocked by #127)
*Rationale:* `worker`/`ai-wizard` declare skills with no datum (worker-execution,
ab-experiment-dispatch, governance-safety-gate, phygital-status-reporting,
stateless-scoring, cognitive-tier-routing, model-architecture, lora-hyperparams,
hf-ecosystem). Fresh agents are told to learn skills that don't exist.
*Required:* per skill: author a stub datum with `status = "aspirational"` OR delete the
ref — decided per skill with the operator.
**DoD:** G3 validator exits 0 on the real store.

### Phase 2 — Lifecycle partition + single format  ← STARTED

**G5: lifecycle flashtable** (#129, in-progress, [review: operator])
*Rationale:* the store mixes four lifecycles (runtime config, docs, scratch, corpus) in
one flat directory; nothing downstream (corpus filter, `b00t learn`, graph validation)
can improve until every file has a lifecycle label. Output doubles as the **first
flashtable**: soul DataFramerr table `lifecycle` (typed columns; cursors = restartable
scans; alarms watch `unresolved`) — b00tyverse pattern #140.
*Required:* every top-level `_b00t_` file labeled
`active | partial | aspirational | deprecated | archive`. Deterministic pre-rules
(`*~`/`.bak` → archive, `DEPRECATED*` → deprecated); ch0nky (qwen3.6-35B-A3B, thinking
ON, temp 0, endpoint+key discovered from the hive datum) for the rest; enum-validated;
failures → `unresolved`, never a guess. Driver: `_b00t_/lifecycle.just` (no one-off
scripts).
**DoD:** `just -f _b00t_/lifecycle.just gate` prints `PASS: <n>/<inventory> labeled, <u>
unresolved`. **Handoff review:** operator reviews all `unresolved` rows + a spot-check
sample (≥5 per label) before G6 may start.

**G6: status field + fold labels** (#130, blocked by #129, [review: operator])
*Rationale:* labels are only useful where consumers look — in the datums. Tag-in-place
first; moving 750 files breaks unenumerated path refs (deferred until G3).
*Required:* `status` added to datum schema; mechanical fold of flashtable labels into
datums as one reviewable diff.
**DoD:** `b00t schema validate` accepts `status`; diff applied after operator SHIP;
G3 still exits 0.

**G7: one canonical skill format** (#131)
*Rationale:* 20 `.skill.toml` vs 64 `.skill.tomllm` — two serializations of one type is
a standing contradiction generator in the corpus.
*Required:* pick one (with operator), mechanical migration, validator rejects the other.
**DoD:** `ls _b00t_/*.skill.<legacy-ext> | wc -l` = 0; validator exits 1 on a seeded
legacy file.

**G8: corpus filter by status** (#132, blocked by #130)
*Rationale:* this is where partition pays: deprecated docs, gospel, PRDs and backups
exit the training corpus.
*Required:* `generate_dataset.py` consumes only `status="active"` (`partial` behind a
flag); G1 records the row delta.
**DoD:** rebuilt `train.jsonl` contains zero rows sourced from files labeled
deprecated/archive/aspirational (assert by provenance metadata); metrics delta committed.

### Phase 3 — Deterministic reward channel

**G9: EvidenceRecord schema** (#133 part 1)
*Rationale:* the reward must be a record a script wrote, not a number a model felt.
*Required:* `{contract_id, handler_cmd, exit_code, evidence_match, duration_ms, tokens,
git_sha, ts, grade}` — rides the **unified evidence spine** of #104 (satisfies /
lfmf-telemetry / spotlight / exec-audit → one); MUST NOT become a fifth channel.
**DoD:** schema datum exists; sample record validates.

**G10: `b00t contract run <capability>`** (#133, P1, [review])
*Rationale:* the single reward function for everything downstream. b00t already has the
right primitive — `[[service_contract]]` with handler + evidence string + verifiable —
it just has no executor.
*Required:* look up contract, execute handler, regex-match declared `evidence` against
stdout, append EvidenceRecord. Exit code mirrors PASS/FAIL. **No LLM in this code path.**
**DoD:** `b00t contract run adapter-smoke-test` on a passing handler → record with
`evidence_match=true`, exit 0; on a failing handler → `false`, exit 1. `grep -rn
'chat/completions\|/v1/' src/commands/contract*.rs` returns nothing.

**G11: real experiment scores** (#134, blocked by #133)
*Rationale:* current scores are theater — accuracy = hash of response bytes in
[0.70,0.95], roi = length ratio (rewards verbosity), utility derived, risk = elapsed
time, fallback chain ends in `sin()`; dispatch is sequential despite the role datum
mandating parallel. Any RL on this converges on longer, hash-luckier outputs.
*Required:* delete roi/utility/risk/accuracy heuristics from
`b00t-cli/src/commands/experiment.rs`; scores = `{pass: from contract, cost_tokens:
measured, time_ms: measured}`; genuinely parallel dispatch. Rewrite the worker role
supplement's "Stateless Scoring Contract" section to match (W3).
**DoD:** `grep -n 'content_hash\|sin()\|accuracy_factor' src/commands/experiment.rs`
returns nothing; parallel dispatch covered by a test; supplement shows the new contract.

**G12: independent-context reviewer — grade-B evidence** (#135)
*Rationale:* some tasks are irreducibly semantic; the fallback must still be
uncontaminated — fresh context, forced choice, separate grade.
*Required:* `b00t experiment review <id>` packages both transcripts + rubric to a fresh
reviewer agent; verdict stored `grade="B"`. Reuses §3 protocol.
**DoD:** review of a fixture experiment produces a grade-B record; aggregation code
rejects mixing grades (unit test).

### Phase 4 — Trace-grounded dataset

**G13: traces → rows** (#142, blocked by #133, [review])
*Rationale:* see corpus doctrine (§1). Behavior is taught by verified executions;
phrasing is taught by templates — the corpus is currently ~100% phrasing.
*Required — row design:*
- **Provenance enum** on every row: `trace | lfmf | contract | templated`, plus
  `evidence_ref` (spine offset/hash + git_sha) for trace rows.
- **Row shapes:**
  1. *contract-trace SFT*: instruction = task intent (title + minimal context);
     response = exact handler invocation + verbatim evidence line.
  2. *DPO pair*: same task id, chosen = PASS transcript, rejected = FAIL transcript.
  3. *lfmf row*: failure context → lesson (harvest `[[lfmf]]` blocks + `learn/*.md`).
  4. *ops-trace*: soul OPS.jsonl / exec-audit command + observed outcome.
- **Quotas:** trace + lfmf + contract rows unlimited (gold); `templated` capped at
  ≤20% of total rows — phrasing seasoning, never the meal.
- **Verifiability gate (deterministic):** every `trace` row's `evidence_ref` must
  re-resolve against the evidence spine (hash lookup); build fails otherwise.
**DoD:** `generate_dataset.py --traces` emits rows with provenance metadata;
`jq '[.[]|select(.provenance=="templated")]|length / total <= 0.2'` style check passes;
verifiability gate exits nonzero on a tampered ref (test fixture).

**G13a: harvest v0 from existing channels** (#144 — before G10 exists)
*Rationale:* don't wait for the spine — gold already exists: `[[lfmf]]` blocks in role
datums, `learn/*.md` lessons, `.b00t/ralph/scores.jsonl` (skill-improve loop),
`spotlight.jsonl`, soul `OPS.jsonl`. Converges with harvest #96.
*Required:* v0 harvester emitting `lfmf` and `ops-trace` rows with provenance metadata.
**DoD:** ≥200 non-templated rows in `train.jsonl` tagged with real source refs; G1
metrics show rows-by-provenance.

**G14: dedup + contradiction pass** (existing scope)
*Rationale:* near-duplicates waste steps; same-instruction/different-response pairs are
the corpus's internal disagreements — surface them, don't average them.
*Required:* b00t-embed clustering; conflict report for human resolution.
**DoD:** dedup report generated; conflicts file reviewed by operator.

**G15: held-out probes ≥50** (#136)
*Rationale:* 5-of-5 on 5 corpus-derived probes is a memorization check, not a gate.
*Required:* ≥50 probes stored outside every corpus scan path; zero train/probe overlap
asserted by content hash; threshold gate (default ≥90%); rotates per generation.
Converges with benchmark #120 (mechanically scored b00t-native tasks).
**DoD:** overlap assertion is a deterministic script in the dataset build; gate
threshold configurable; probe count ≥50.

### Phase W — Wake & context reduction (parallel workstream)

**W1: trace-or-filler doctrine in the wake** (shipped-on-branch, [review: operator])
*Rationale:* P4 harvests traces only if agents emit them; the wake is where behavior is
REQUIRED. Every session must end tasks with a verified evidence line — this converts
every agent session into corpus.
*Shipped:* Core Law "Trace-or-filler" + MUST ALWAYS evidence-line bullet in
`AGENTS.md` (= `_b00t_/AGENT.md` symlink) and `CLAUDE.optimized.md`. Net +7 lines to the
full wake — paid for many times over by W2.
**DoD:** operator SHIP on the wording; `b00t whoami | grep -q 'Trace-or-filler'`.

**W2: sm0l worker wake** (#145, [review: operator])
*Rationale:* workers pay ~200 wake lines for a constitution they don't need (hive CMDB,
.tomllm spec, cognitive-tier table, guards already runtime-enforced). Baseline context
for workers should be orders + laws + output contract. Template authored:
`_b00t_/AGENT.sm0l.md` (~40 lines, ~80% reduction).
*Required:* whoami selects template via role datum field `wake = "sm0l"` (default:
full wake — zero behavior change for existing roles); `worker.role.toml` opts in.
Aligns with #97 (b00t init hooks / `--suffix-only`).
**DoD:** `b00t whoami --role=worker | wc -l` < 60 while `b00t whoami` (operator) is
unchanged; role datums without `wake` field render identically to today (snapshot test).

**W3: worker supplement de-theater** (#146, with #134)
*Rationale:* `AGENTS/--role=worker.md` currently teaches the fake scoring dimensions
(roi/utility/risk) G11 deletes — the supplement trains the exact behavior the reward
doctrine forbids.
*Required:* rewrite scoring section to `{pass, cost_tokens, time_ms}` + evidence line;
A/B dispatch section points at the real (parallel) dispatcher.
**DoD:** `grep -n 'ROI\|UTILITY\|RISK' AGENTS/--role=worker.md` returns nothing; ships
in the same PR as #134.

**W4: skill→datum briefing in the wake** (#147)
*Rationale:* whoami already knows the role's skills; every datum already carries a
machine-readable tail-map (`summary:` + `cmds:`). A few commands convey a lot of context
when the wake renders one briefing line per skill FROM THE TAIL-MAPS — no LLM, no full
datum load. Agents must read it or they do not know: every capability named ships with a
copy-paste invocation. Example rendering (2 lines per skill, budget-capped):
```
🧠 ai-finetune — QLoRA pipeline: dataset gen, local k8s, HF Jobs cloud
   ↳ just ai-finetune::dataset · just ai-finetune::cloud-coder · just ai-finetune::test
```
*Required:* whoami step between role supplement and datum summary: for each role skill,
parse the datum tail-map, emit `summary` line + `cmds` line. Missing tail-map → one ⚠️
line (feeds #115 cleanup). Also: remove or implement the wake's dangling
`b00t compile-agent` claim (subcommand does not exist — same family as #117).
**DoD:** `b00t whoami --role=ai-worker` shows a briefing line per skill sourced from
tail-maps (snapshot test); total wake growth ≤2 lines/skill; `compile-agent` either
exists or is gone from AGENTS.md.

**W5: example-or-silence lint** (#148)
*Rationale:* laconic form only works when every surfaced command is real and runnable —
a named-but-wrong command is worse than none (agents copy it verbatim).
*Required:* wow check: every `cmds:` entry in every active datum tail-map must resolve —
`just -n <recipe>` or `b00t-cli <sub> --help` exits 0. Wake line budget enforced by the
same check: full wake ≤220 lines, sm0l ≤50.
**DoD:** `b00t wow check` gains `E:CmdsResolvable` + `E:WakeBudget`; seeded bogus
`cmds:` line turns it red; current store passes after cleanup.

**W6: graph auto-discovery of blessings** (#149)
*Rationale:* `--skills=auto` interview exists but is shallow. The datum graph
(tags, depends_on, entangled_*) + grok/qdrant embeddings can rank candidate blessings
against the task context deterministically-first (tag/keyword match on tail-maps; no
LLM), embeddings as tie-breaker. Personality/trait fields on agent datums act as a
ranking prior. Output stays laconic — max 5 candidates, each with match evidence:
```
## Candidate blessings (task: "fix hf jobs checkpoint resume")
1. ai-finetune  match: tags[finetune,hf-jobs]  → b00t learn ai-finetune
2. hf-cli       match: cmds[hf jobs logs]      → b00t learn hf-cli
```
*Required:* `b00t whoami --skills=auto` reads `B00T_TASK_CONTEXT` (or stdin), ranks via
tail-map tag/keyword match, optional `--semantic` flag adds b00t-embed similarity;
prints ranked list with WHY. Never auto-loads — discovery proposes, agent disposes.
**DoD:** fixture task context yields deterministic top-3 (tag-match only, no network);
`--semantic` path covered by one integration test against local qdrant.

**W7: authorization grants from the credential vault** (#150, P1, [review: operator])
*Rationale:* the highest-value context whoami can convey is WHAT YOU MAY DO: which
credentials this role+blessing combination grants, and how to invoke them — without
ever placing a secret value in context. The substrate exists: `CredentialDatum`
(encrypt/decrypt, `key_env_for(provider)`) + OS keyring backend (`CredentialBackend`)
+ `b00t exec` guard/audit + `blessing --manifest`. Missing: the grant edge and the wake
rendering. Example rendering:
```
## Authorizations (keyring-mediated — values NEVER enter context)
🔐 hf      → HF_TOKEN         use: b00t exec --grant hf -- hf auth whoami
🔐 openai  → OPENAI_API_KEY   use: b00t exec --grant openai -- <cmd>   (local-b00t proxy)
⛔ aws     — not granted to worker (requires: b00t learn aws-cli + operator approval)
```
*Required:*
- skill datums gain `grants = ["<provider>", ...]` alongside `unlocks`; blessing
  manifest resolves (role, learned-skills) → grant set.
- `b00t exec --grant <provider> -- <cmd>` injects the secret from keyring into the
  child env ONLY (never stdout), appends to the exec audit log per use.
- wake renders grants as name + env var + invocation shape; denials are informative
  (name the path to authorization).
- Security invariants (non-negotiable): secret values never rendered by whoami or any
  wake path; grants are deny-by-default; every injection audited; grant edges validated
  by G3 like any other graph ref.
**DoD:** `b00t whoami --role=worker` shows grant lines with zero secret bytes (test
asserts no keyring value substring appears in output); `b00t exec --grant hf -- env`
child sees `HF_TOKEN`, parent transcript does not; audit line written per injection;
ungranted provider → exit 1 + informative denial.

### Phase X — Dogfood execution & verification economy (parallel workstream)

**X1: exec → artifact → ledgrrr** (#151, mock shipped: `_b00t_/cake.just`)
*Rationale:* commands run through b00t so they are logged; the log entry becomes a
verification artifact; the artifact is submitted to enterprise ledgrrr as proof, with a
🍰 allocation. Cake attaches to ARTIFACTS (verified execution), never to prose — the
social layer of the same reward doctrine (§1). Mock ledger:
`.b00t/ledgrrr-mock.jsonl`, FOCUS-shaped (`earned`/`consumed`), same record family as
`experiment.rs::focus_record_to_ledgrrr`.
*Required (real):* `b00t contract run` (G10) and `b00t sh` submissions flow to
ledgrrr-mcp via the existing emit path; allocation policy set by operator.
**DoD:** `just -f _b00t_/cake.just verify <task> <cmd>` mints artifact + ledger entry
(mock: PASSING today); real path replaces file append with ledgrrr-mcp call; no LLM.

**X2: `b00t sh` — audited shell alias** (#152, shipped-on-branch)
*Rationale:* ergonomics decide adoption: `b00t sh -- <cmd>` must be as cheap to type as
raw bash, or agents route around the audit log.
*Shipped:* `visible_alias = "sh"` on Exec (main.rs); long_about documents the
artifact/ledgrrr linkage. Build initially failed on the #101 `core.bare` rewriter
(cargo gix fingerprinting) — fixed per that task's documented workaround.
**DoD:** `b00t sh -- echo pong` prints pong, exit 0, and the run appears in
`~/.b00t/exec-log.jsonl` (verified on target/debug; `cargo install` pending).

**X3: execution ladder doctrine** (shipped, reviews with #143)
*Rationale:* `just` is HIGHER VALUE than ad-hoc shell: a recipe is a memoized,
REGISTERED action space entry (registry-gated per VISION-just-datum-executor),
self-documenting via tail-map, reusable by every agent, and is the `[[service_contract]]`
handler surface — an ad-hoc command benefits one session, a recipe compounds. `b00t sh`
is the audited middle rung; raw bash is invisible to the hive and earns nothing.
*Shipped:* ladder block in AGENTS.md + CLAUDE.optimized.md + AGENT.sm0l.md.
**DoD:** operator SHIP (#143); `b00t whoami | grep -q 'Execution ladder'`.

**X4: recipe-anchored justfile patching — never sed** (#153)
*Rationale:* sed on justfiles is line-number roulette. Serena's pattern is the model:
anchor edits on SYMBOLS (`replace_symbol_body`, `insert_after_symbol`) with
diff-before-write. For justfiles the symbol is the recipe. `b00t patch` already gives
whole-file diff-before-write (`patch apply <file> -` reads proposed content on stdin);
what's missing is the recipe-level anchor.
*Required:* `b00t patch apply <justfile> --recipe <name> -` replaces one recipe body
(tree-sitter-just via b00t-ast; comments and neighbors untouched); `--after-recipe` for
insertion; `just -n` parse-check gates the write.
**DoD:** fixture justfile: replace one recipe body → diff shows only that recipe;
`just -n` exits 0; tail-map preserved byte-identical.

**X5: guard hygiene** (#154)
*Rationale:* dogfooding found session guards `a`, `b`, `c` — single-character patterns
matching nearly every command (31 violations), making `b00t exec` unusable until
`guard remove`. Guards that cry wolf train agents to bypass the audit path.
*Required:* `guard add` validation (min pattern length ≥3, reject bare catch-alls,
warn on >50% match-rate in first 10 execs); `guard list` shows provenance (who/when).
**DoD:** `b00t guard add a` exits 1 with explanation; provenance column in list.

### Phase 5 — Close the loop

**G16: contract-bearing batch processor** (#137, blocked by #133)
*Rationale:* the repeatable task processor — semantic (MCP-dispatched) or scripted
(batch) — with the action space constrained to registered justfile recipes
(`VISION-just-datum-executor` registry gate): deterministic action space + verifiable
evidence is the only configuration that doesn't reward-hack.
*Required:* `just worker-batch` pulls tasks from `b00t task` where each declares
`contract_id`; dispatch → handler → EvidenceRecord per task.
**DoD:** batch of 3 fixture tasks yields 3 EvidenceRecords with correct pass/fail.

**G17: one full training generation** (#138, blocked by #132 + #136, [review: operator])
*Rationale:* prove the pipe end-to-end at sm0l scale before spending A100 money.
*Required:* dataset → `train-smol` → G15 probe gate → promote to sm0l tier ONLY on gate
PASS.
**DoD:** a gate-FAIL run demonstrably blocks promotion (fixture); a gate-PASS run
promotes; both leave evidence records.

**G18: adapter A/B on contract pass-rate** (#139, blocked by #138)
*Rationale:* eval on real work, not vibes: promoted adapter vs base on the batch tasks
themselves. Reward channel and eval channel become the same deterministic script.
This is the A/B control loop for mission #114, scored by benchmark #120.
**DoD:** A/B result derived exclusively from EvidenceRecords (audit: no other inputs).

### Phase 6 — Ratchet (continuous)

G1 metrics trend per generation; `b00t lfmf` on every failure; probe set grows every
generation; no phase regresses a prior phase's contract. Each closed gap gets a
`[[service_contract]]` so its DoD stays continuously enforced, not once-checked.

## 5. Convergence with the existing queue

This PRD feeds mission **#114** (native-b00t ch0nky):
- **G10 EvidenceRecord** rides #104's unified evidence spine — never a fifth channel.
- **G13/G13a/G15** are the dataset half of #120 (benchmark) and #96/#595
  (harvest, evidence→train).
- **G2** closes the same defect family as red wow check `B:TypeInvariant` and #94.
- **W2** aligns with #97 (init hooks / suffix-only wake); **G3** composes with #115
  (b00t-lsp datum errors) and #121 (single-file --check).
- The `core.bare=true` rewriter (#101) bit this work too — branch created via plumbing.
- ch0nky-in-k0s + gh-runner CrashLoopBackOff ×3037: #141.

## 6. Sequencing & Pace

One gap at a time, each independently shippable and immediately dogfooded; rome was not
built in a day. Grunt work runs on local silicon (ch0nky, thinking ON); frontier tokens
are for review and novel design. P0/P1 are small Rust PRs. P2-G5 is running now. P3 is
the heart — P4/P5 do not start until G10 exists, because every later acceptance test is
expressed as a service contract. W runs parallel because P4's harvest depends on agents
emitting traces starting today.

<!-- b00t:map v1
summary: RLVR gap-closure plan — lifecycle flashtable, graph validation, deterministic script reward (no LLM), trace-grounded corpus over templated, sm0l worker wake, handoff review protocol, closed training loop feeding mission #114
tags: prd, rlvr, lifecycle, flashtable, service-contract, reward, trace, corpus, wake, context-reduction, gap-closure
tier: frontier
cmds: just -f _b00t_/lifecycle.just gate, b00t task list, just b00t-metrics
complexity: 9
-->
