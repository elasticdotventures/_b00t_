// autoexec.rhai — autonomous reviewer execution loop
//
// Runs: system-normal() → poll queue → risk-score → dispatch → post → metrics → loop
//
// 7-phase state machine backing: _b00t_/multi-framework-review.datum.toml
// Risk routing via: _b00t_/pr-risk-scorer.datum.toml
// Precriteria definitions: _b00t_/datums/SYSTEM-NORMAL.tomllmd
//
// Usage: b00t script run _b00t_/skills/reviewer/autoexec.rhai
//
// Output contract:
//   On success: structured status + findings summary printed to stdout
//   On gate fail: FAIL reason printed + execution blocked
//   On error: caught and reported; loop continues to next cycle

// Import precriteria gate utilities (shell, check_exact)
import "system-normal" as sys;

// ── Constants ──────────────────────────────────────────────────────────────────
// 🤓 MAX_ITERATIONS caps autonomous cycles to prevent runaway loops.
//    Each cycle's SLEEP_SECONDS means a full run takes ~MAX_ITERATIONS × (review_time + SLEEP_SECONDS).
const MAX_ITERATIONS = 10;
const SLEEP_SECONDS = 60;
const MIN_LINES_FOR_PARALLEL = 500;    // matches multi-framework-review.datum.toml [b00t.parallel].min_lines_for_parallel
const MAX_CONCURRENT_AGENTS = 3;       // matches multi-framework-review.datum.toml [b00t.parallel].max_concurrent_agents
const METRICS_FILE = ".b00t/reviewer-metrics.jsonl";
const STATE_FILE = ".b00t/multi-framework-review-state.json";

// ── Risk Score Thresholds (from pr-risk-scorer.datum.toml [b00t.thresholds]) ────
const SCORE_LOW_MAX = 29;
const SCORE_MEDIUM_MAX = 55;
const SCORE_HIGH_MAX = 75;
// >75 = CRITICAL (adversarial A/B/R bounce loop with operator oversight)

// ── Security-Sensitive File Patterns (from pr-risk-scorer.datum.toml [b00t.security_sensitive_patterns]) ──
const SEC_PATTERNS = [
    "auth", "authenticate", "login", "session", "token",
    "crypto", "encrypt", "decrypt", "hash", "sign", "key",
    "validate", "sanitize", "parser", "deserialize",
    ".env", "secret", "credential", "vault",
    "permission", "rbac", "acl", "authorize", "guard"
];

// ── Dependency Manifest Globs ──────────────────────────────────────────────────
const DEP_MANIFEST_PATTERN = "(Cargo\\.toml|package\\.json|pyproject\\.toml|go\\.mod|Gemfile|requirements\\.txt|Pipfile)";


// ═══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

/// Run a shell command, return trimmed stdout. Returns "" on failure.
fn shell(cmd) {
    try { run_cmd(cmd).trim() } catch(e) { "" }
}

/// ISO-8601 UTC timestamp for audit logging.
fn timestamp() {
    shell("date -u +%Y-%m-%dT%H:%M:%SZ")
}

/// Check file existence via test -f.
fn file_exists(path) {
    shell(`test -f "${path}" && echo yes || echo no`).contains("yes")
}

/// Check directory existence via test -d.
fn dir_exists(path) {
    shell(`test -d "${path}" && echo yes || echo no`).contains("yes")
}

/// Ensure the metrics log directory exists, creating it if needed.
fn ensure_metrics_dir() {
    if !dir_exists(".b00t") {
        shell("mkdir -p .b00t");
    }
}

/// Parse an integer from a string, returning 0 on failure.
fn parse_int(s) {
    if s == "" || s == () { return 0; }
    let t = s.trim();
    if t == "" { return 0; }
    try { t.to_int() } catch(e) { 0 }
}

/// Clamp value between min and max (inclusive).
fn clamp(val, min_val, max_val) {
    if val < min_val { min_val } else if val > max_val { max_val } else { val }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 1: SYSTEM-NORMAL PRECRITERIA
// ═══════════════════════════════════════════════════════════════════════════════
// Mirrors _b00t_/skills/reviewer/system-normal.rhai checks.
// Returns { allowed: bool, gate: { status, checks, summary, pass_count, fail_count } }
//   - FAIL on any error-severity check → blocks review
//   - WARN on warn-severity failures → proceeds with caution
//   - PASS when all checks pass

fn system_normal_gate() {
    // ── Individual checks ─────────────────────────────────────────────────────

    // Check 1: git stash must be empty (warn)
    let stash_count = parse_int(shell("git stash list 2>/dev/null | wc -l | tr -d ' '"));
    let c1 = #{
        id: "git-stash-reviewed",
        label: "git stash queue reviewed and empty",
        passed: stash_count == 0,
        severity: "warn",
        message: if stash_count == 0 { "" } else { "git stash has entries — REVIEW before dropping; stashes may contain valuable WIP" }
    };

    // Check 2: no merge/rebase conflict markers (error — blocks on fail)
    let conflict_count = parse_int(shell("git status --porcelain 2>/dev/null | grep -c '^UU\\|^AA\\|^DD' || echo 0"));
    let c2 = #{
        id: "git-no-merge-conflict",
        label: "no active merge/rebase conflict markers",
        passed: conflict_count == 0,
        severity: "error",
        message: if conflict_count == 0 { "" } else { "unresolved merge/rebase conflicts present — resolve before continuing" }
    };

    // Check 3: not in detached HEAD (warn)
    let head_state = shell("git symbolic-ref --short HEAD 2>/dev/null && echo ok || echo detached");
    let c3 = #{
        id: "git-not-detached-head",
        label: "not in detached HEAD state",
        passed: head_state.contains("ok"),
        severity: "warn",
        message: if head_state.contains("ok") { "" } else { "detached HEAD — checkout a branch: git checkout -b task/<slug>" }
    };

    // Check 4: not on main branch (warn)
    let branch = shell("git branch --show-current 2>/dev/null");
    let c4 = #{
        id: "no-uncommitted-on-main",
        label: "not on main branch",
        passed: branch != "main",
        severity: "warn",
        message: if branch != "main" { "" } else { "on main branch — always work on a task branch: git checkout -b task/<id>-<slug>" }
    };

    // Check 5: submodules in sync (warn)
    let submodule_dirty = parse_int(shell("git submodule status 2>/dev/null | grep -c '^[+M]' || echo 0"));
    let c5 = #{
        id: "submodules-in-sync",
        label: "all submodules in sync",
        passed: submodule_dirty == 0,
        severity: "warn",
        message: if submodule_dirty == 0 { "" } else { "submodule(s) have unpushed commits or dirty trees — sync before task operations" }
    };

    // ── Aggregate ─────────────────────────────────────────────────────────────

    let checks = [c1, c2, c3, c4, c5];
    let has_error = false;
    let has_warn = false;
    let mut pass_count = 0;
    let mut fail_count = 0;

    for c in checks {
        if c.passed {
            pass_count = pass_count + 1;
        } else {
            fail_count = fail_count + 1;
            if c.severity == "error" {
                has_error = true;
            } else if c.severity == "warn" {
                has_warn = true;
            }
        }
    }

    let status = if has_error { "FAIL" } else if has_warn { "WARN" } else { "PASS" };
    let summary = if status == "PASS" { "All checks passed — system is normal" }
                  else if status == "FAIL" { `${fail_count} check(s) failed — errors MUST be resolved` }
                  else { `${fail_count} warning(s) — review recommended` };

    let gate = #{
        status: status,
        checks: checks,
        summary: summary,
        pass_count: pass_count,
        fail_count: fail_count
    };

    // ── Blocking decision ─────────────────────────────────────────────────────
    let allowed = !has_error;  // error severity blocks; warn allows with caution

    // Compact inline status (full report already printed by the import of system-normal.rhai)
    if !allowed {
        print(`❌ system-normal FAILED [${fail_count} errors]: blocking review`);
        for c in checks {
            if !c.passed && c.severity == "error" {
                print(`   → [${c.severity}] ${c.id}: ${c.message}`);
            }
        }
    } else if has_warn {
        print(`⚠️  system-normal WARN [${fail_count} warnings, ${pass_count} passed]: proceeding with reduced confidence`);
    } else {
        print(`✅ system-normal PASS [${pass_count}/${checks.len()}]: all gates clear`);
    }

    #{ allowed: allowed, gate: gate }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 2: POLL REVIEW QUEUE
// ═══════════════════════════════════════════════════════════════════════════════
// Queries b00t task queue and gh CLI for open PRs needing review.
// Filters for PRs with review-related labels.
// Returns array of { number, title, author }.

fn poll_review_queue() {
    let tasks = [];

    // Strategy 1: b00t task queue (preferred — native task tracking)
    let b00t_tasks = shell("b00t-cli task list --status open 2>/dev/null");
    if b00t_tasks != "" {
        for line in b00t_tasks.split("\n") {
            let t = line.trim();
            if t.contains("review") && !t.starts_with("#") && !t.starts_with("─") {
                // Extract PR number: look for # followed by digits
                let hash_idx = t.index_of("#");
                if hash_idx >= 0 {
                    let rest = t.sub_string(hash_idx + 1);
                    let end_idx = rest.len();
                    for j in range(0, rest.len()) {
                        let ch = rest.sub_string(j, 1);
                        if ch < "0" || ch > "9" { end_idx = j; break; }
                    }
                    let pr_str = rest.sub_string(0, end_idx);
                    let pr_num = parse_int(pr_str);
                    if pr_num > 0 {
                        // Avoid duplicates
                        let exists = false;
                        for existing in tasks {
                            if existing.number == pr_num { exists = true; break; }
                        }
                        if !exists {
                            tasks = tasks + #{ number: pr_num, title: t, author: "b00t-queue" };
                        }
                    }
                }
            }
        }
    }

    // Strategy 2: gh CLI — PRs explicitly labeled for review
    let gh_prs = shell("gh pr list --label 'multi-framework-review' --state open --json number,title,author --jq '.[] | \"\\(.number)|\\(.title)|\\(.author.login)\"' 2>/dev/null");
    if gh_prs != "" {
        for line in gh_prs.split("\n") {
            let t = line.trim();
            if t == "" { continue; }
            let parts = [];
            let mut start = 0;
            for k in range(0, t.len()) {
                if t.sub_string(k, 1) == "|" {
                    parts = parts + t.sub_string(start, k - start);
                    start = k + 1;
                }
            }
            parts = parts + t.sub_string(start);
            if parts.len() >= 2 {
                let pr_num = parse_int(parts[0]);
                if pr_num > 0 {
                    let exists = false;
                    for existing in tasks {
                        if existing.number == pr_num { exists = true; break; }
                    }
                    if !exists {
                        tasks = tasks + #{ number: pr_num, title: parts[1], author: if parts.len() >= 3 { parts[2] } else { "unknown" } };
                    }
                }
            }
        }
    }

    // Strategy 3: broader — any open PR with "needs-review" label
    if tasks.len() == 0 {
        let broad_prs = shell("gh pr list --label 'needs-review' --state open --json number,title,author --jq '.[] | \"\\(.number)|\\(.title)|\\(.author.login)\"' 2>/dev/null");
        if broad_prs != "" {
            for line in broad_prs.split("\n") {
                let t = line.trim();
                if t == "" { continue; }
                let parts = [];
                let mut start = 0;
                for k in range(0, t.len()) {
                    if t.sub_string(k, 1) == "|" {
                        parts = parts + t.sub_string(start, k - start);
                        start = k + 1;
                    }
                }
                parts = parts + t.sub_string(start);
                if parts.len() >= 2 {
                    let pr_num = parse_int(parts[0]);
                    if pr_num > 0 {
                        let exists = false;
                        for existing in tasks {
                            if existing.number == pr_num { exists = true; break; }
                        }
                        if !exists {
                            tasks = tasks + #{ number: pr_num, title: parts[1], author: if parts.len() >= 3 { parts[2] } else { "unknown" } };
                        }
                    }
                }
            }
        }
    }

    tasks
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 3: RISK SCORE PR
// ═══════════════════════════════════════════════════════════════════════════════
// Scores a PR across 6 dimensions (author_experience, file_hotness, change_size,
// test_coverage_delta, security_surface_area, dependency_impact).
// Weights from pr-risk-scorer.datum.toml [b00t.scoring.weights].
// Maps total weighted score to tier: LOW | MEDIUM | HIGH | CRITICAL.
//
// Returns { tier, total_score, dimensions, total_lines, files_count, author }

fn risk_score_pr(pr_number) {
    print(`\n🔍 Risk-scoring PR #${pr_number}...`);

    // ── Fetch PR metadata ─────────────────────────────────────────────────────
    let author = shell(`gh pr view ${pr_number} --json author --jq '.author.login' 2>/dev/null`);
    let files_count = parse_int(shell(`gh pr view ${pr_number} --json files --jq '.files | length' 2>/dev/null`));
    let add_lines = parse_int(shell(`gh pr view ${pr_number} --json additions --jq '.additions' 2>/dev/null`));
    let del_lines = parse_int(shell(`gh pr view ${pr_number} --json deletions --jq '.deletions' 2>/dev/null`));
    let total_lines = add_lines + del_lines;

    let changed_files = shell(`gh pr diff ${pr_number} --name-only 2>/dev/null`);

    // ── D1: Author Experience (weight 1.0) ─────────────────────────────────────
    let author_commits = parse_int(shell(`git log --author="${author}" --oneline 2>/dev/null | wc -l`));
    // 100+ commits → 0, first-timer → 10
    let auth_score = if author_commits >= 100 { 0 }
                     else if author_commits >= 50 { 2 }
                     else if author_commits >= 20 { 4 }
                     else if author_commits >= 5 { 6 }
                     else if author_commits >= 1 { 8 }
                     else { 10 };

    // ── D2: File Hotness (weight 1.5) ─────────────────────────────────────────
    let hot_files = 0;
    let total_files = 0;
    for f in changed_files.split("\n") {
        let fn = f.trim();
        if fn == "" { continue; }
        total_files = total_files + 1;
        let churn = parse_int(shell(`git log --since='90 days ago' --oneline -- "${fn}" 2>/dev/null | wc -l`));
        if churn > 5 { hot_files = hot_files + 1; }
    }
    let hot_score = if total_files > 0 { clamp((hot_files * 10 / total_files), 0, 10) } else { 5 };

    // ── D3: Change Size (weight 2.0) ───────────────────────────────────────────
    let size_score = if total_lines < 50 && files_count <= 1 { 0 }
                     else if total_lines < 100 { 2 }
                     else if total_lines < 200 { 4 }
                     else if total_lines < 350 { 6 }
                     else if total_lines < 500 { 8 }
                     else { 10 };
    // Boost if many files touched
    if files_count > 15 && size_score < 9 { size_score = size_score + 1; }

    // ── D4: Test Coverage Delta (weight 1.5) ───────────────────────────────────
    let has_tests = false;
    for f in changed_files.split("\n") {
        let fn = f.trim().to_lower();
        if fn.contains("test") || fn.contains("spec") || fn.contains("__tests__") {
            has_tests = true;
            break;
        }
    }
    // No test files in a multi-file change → elevated risk
    let cov_score = if has_tests { 2 } else if files_count > 3 { 7 } else { 5 };

    // ── D5: Security Surface Area (weight 2.0) ─────────────────────────────────
    let mut sec_score = 0;
    for f in changed_files.split("\n") {
        let fn = f.trim().to_lower();
        for pat in SEC_PATTERNS {
            if fn.contains(pat) {
                sec_score = sec_score + 2;
                break;
            }
        }
        if sec_score >= 10 { sec_score = 10; break; }
    }

    // ── D6: Dependency Impact (weight 2.0) ─────────────────────────────────────
    let dep_changes = shell(`gh pr diff ${pr_number} --name-only 2>/dev/null | grep -c -E '${DEP_MANIFEST_PATTERN}' || echo 0`);
    let has_dep_changes = parse_int(dep_changes) > 0;
    let new_deps = parse_int(shell(`gh pr diff ${pr_number} 2>/dev/null | grep -c '^+.*[dD]ependenc' || echo 0`));
    let dep_score = if new_deps > 3 { 10 }
                    else if new_deps > 0 { clamp(6 + new_deps, 4, 10) }
                    else if has_dep_changes { 6 }
                    else { 2 };

    // ── Weighted Total ────────────────────────────────────────────────────────
    let weighted = auth_score * 1.0 + hot_score * 1.5 + size_score * 2.0 +
                   cov_score * 1.5 + sec_score * 2.0 + dep_score * 2.0;
    let total_score = clamp(weighted.to_int(), 0, 100);

    // ── Map to Tier ───────────────────────────────────────────────────────────
    let tier = if total_score <= SCORE_LOW_MAX { "LOW" }
               else if total_score <= SCORE_MEDIUM_MAX { "MEDIUM" }
               else if total_score <= SCORE_HIGH_MAX { "HIGH" }
               else { "CRITICAL" };

    // ── Output Contract (matches pr-risk-scorer.datum.toml [b00t.output_contract]) ──
    print(`   D1(auth)=${auth_score} D2(hot)=${hot_score} D3(size)=${size_score} D4(cov)=${cov_score} D5(sec)=${sec_score} D6(dep)=${dep_score}`);
    print(`   SCORE: ${tier}:${total_score}:auth=${auth_score},hot=${hot_score},size=${size_score},cov=${cov_score},sec=${sec_score},dep=${dep_score}`);

    #{
        tier: tier,
        total_score: total_score,
        dimensions: #{ auth: auth_score, hot: hot_score, size: size_score, cov: cov_score, sec: sec_score, dep: dep_score },
        total_lines: total_lines,
        files_count: files_count,
        author: author
    }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 4: DISPATCH REVIEW
// ═══════════════════════════════════════════════════════════════════════════════
// Routes PR to appropriate review depth based on risk tier and PR size.
// Tier mapping (from pr-risk-scorer.datum.toml [b00t.tiers]):
//   LOW:      standard-checklist (1 agent, advisory)
//   MEDIUM:   multi-framework-sequential (MECE → TRIZ → Eureka → Synthesize)
//   HIGH:     multi-framework-parallel (3 sub-agents concurrently + synthesis)
//   CRITICAL: adversarial-abr (3-round bounce with operator oversight)
//
// Parallel dispatch activates when total_lines >= MIN_LINES_FOR_PARALLEL (500)
// even for MEDIUM tier (override from multi-framework-review.datum.toml [b00t.parallel]).
//
// Returns { tier, findings: [{ phase, result|error }], status }

fn dispatch_review(pr_number, tier, risk) {
    print(`\n🚀 Dispatching review for PR #${pr_number} [${tier}]`);
    print(`   Lines: ${risk.total_lines} | Files: ${risk.files_count} | Author: ${risk.author}`);

    let findings = [];

    // Parallel override: large PRs at MEDIUM tier get parallel dispatch
    let use_parallel = tier == "HIGH" || tier == "CRITICAL" || risk.total_lines >= MIN_LINES_FOR_PARALLEL;

    if tier == "LOW" && !use_parallel {
        // ── LOW: Standard checklist review ────────────────────────────────────
        print("   → Standard checklist review (single agent, advisory)");
        try {
            let result = shell(`b00t-cli agent delegate github-code-review "PR ${pr_number} standard checklist review" 2>&1`);
            findings = findings + #{ phase: "checklist", result: result };
        } catch(e) {
            findings = findings + #{ phase: "checklist", error: `delegation failed: ${e}` };
        }

    } else if tier == "MEDIUM" && !use_parallel {
        // ── MEDIUM: Sequential multi-framework ────────────────────────────────
        print("   → Multi-framework sequential (MECE → TRIZ → Eureka → Synthesize)");

        let phases = ["setup", "mece", "triz", "eureka", "synthesize"];
        for phase in phases {
            print(`   [${phase}] running...`);
            try {
                let result = shell(`just review-${phase} PR=${pr_number} 2>&1`);
                findings = findings + #{ phase: phase, result: result };
                print(`   [${phase}] ✅ complete`);
            } catch(e) {
                findings = findings + #{ phase: phase, error: `execution failed: ${e}` };
                print(`   [${phase}] ❌ failed: ${e}`);
            }
        }

    } else if tier == "HIGH" || use_parallel {
        // ── HIGH / Parallel: 3 concurrent sub-agents + synthesis ──────────────
        print("   → Multi-framework parallel (3 concurrent agents + synthesis)");

        // Setup first (sequential prerequisite for all frameworks)
        try {
            let setup_result = shell(`just review-setup PR=${pr_number} 2>&1`);
            findings = findings + #{ phase: "setup", result: setup_result };
        } catch(e) {
            findings = findings + #{ phase: "setup", error: `setup failed: ${e}` };
        }

        // Delegate MECE, TRIZ, Eureka to sub-agents concurrently
        let delegate_ids = [];
        let frameworks = [
            #{ tag: "review-mece", label: "mece", desc: "PR ${pr_number} MECE analysis" },
            #{ tag: "review-triz", label: "triz", desc: "PR ${pr_number} TRIZ analysis" },
            #{ tag: "review-eureka", label: "eureka", desc: "PR ${pr_number} Eureka analysis" }
        ];

        for fw in frameworks {
            try {
                let agent_id = shell(`b00t-cli agent delegate ${fw.tag} "${fw.desc}" 2>&1`).trim();
                print(`   [${fw.label}] delegated → ${agent_id}`);
                delegate_ids = delegate_ids + #{ label: fw.label, id: agent_id };
            } catch(e) {
                findings = findings + #{ phase: fw.label, error: `delegation failed: ${e}` };
                print(`   [${fw.label}] ❌ delegation failed: ${e}`);
            }
        }

        // Poll for completion (max 30 cycles, 10s each = 5 min timeout)
        let mut all_done = false;
        let mut wait_cycles = 0;
        while !all_done && wait_cycles < 30 {
            let mut completed = 0;
            for d in delegate_ids {
                let status_json = shell(`b00t-cli task status ${d.id} --json 2>/dev/null`);
                if status_json.contains("\"status\":\"completed\"") || status_json.contains("\"status\":\"failed\"") {
                    completed = completed + 1;
                }
            }
            if completed >= delegate_ids.len() {
                all_done = true;
                break;
            }
            wait_cycles = wait_cycles + 1;
            print(`   ⏳ Waiting for agents (${completed}/${delegate_ids.len()} done, cycle ${wait_cycles}/30)...`);
            shell("sleep 10");
        }

        // Synthesize results from all three frameworks
        try {
            let synth_result = shell(`just review-synthesize PR=${pr_number} 2>&1`);
            findings = findings + #{ phase: "synthesize", result: synth_result };
            print("   [synthesize] ✅ complete");
        } catch(e) {
            findings = findings + #{ phase: "synthesize", error: `synthesis failed: ${e}` };
            print(`   [synthesize] ❌ failed: ${e}`);
        }

    } else {
        // ── CRITICAL: Adversarial A/B/R bounce loop ───────────────────────────
        print("   → CRITICAL: Adversarial A/B/R review (operator oversight required)");
        print("   🚩 Human-in-the-loop gate activated — cannot proceed autonomously");
        print("   🚩 Required: 3 review rounds with operator sign-off at each gate");

        // Log to audit trail
        ensure_metrics_dir();
        let audit_entry = `{"ts":"${timestamp()}","pr":${pr_number},"tier":"CRITICAL","action":"AUTOEXEC_BLOCKED","reason":"operator_oversight_required","risk_score":${risk.total_score}}`;
        try {
            shell(`echo '${audit_entry}' >> ${METRICS_FILE}`);
        } catch(e) {}

        return #{
            tier: tier,
            findings: findings,
            status: "BLOCKED",
            reason: "CRITICAL tier requires operator oversight — adversarial A/B/R bounce loop of 3 rounds with human-in-the-loop gates"
        };
    }

    // ── Return dispatch result ─────────────────────────────────────────────────
    #{
        tier: tier,
        findings: findings,
        status: if findings.len() > 0 { "DISPATCHED" } else { "FAILED" }
    }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 5: POST REVIEW RESULTS
// ═══════════════════════════════════════════════════════════════════════════════
// Submits compiled findings as a PR comment via gh CLI.
// Returns true on successful post, false on failure.

fn post_review_results(pr_number, findings) {
    print(`\n📝 Posting review results to PR #${pr_number}...`);

    // Compile findings into a markdown comment body
    let body_parts = [];
    body_parts = body_parts + "## 🤖 Multi-Framework Code Review";
    body_parts = body_parts + "";
    body_parts = body_parts + `**PR**: #${pr_number} | **Reviewer**: b00t-autoexec | **Timestamp**: ${timestamp()}`;
    body_parts = body_parts + "";

    if findings.len() == 0 {
        body_parts = body_parts + "⚠️ No findings available — review dispatch may have failed.";
    } else {
        body_parts = body_parts + "### Phase Results";
        for f in findings {
            if f.contains("error") {
                body_parts = body_parts + `- ❌ **${f.phase}**: Error — ${f.error}`;
            } else {
                body_parts = body_parts + `- ✅ **${f.phase}**: Completed`;
            }
        }
    }

    body_parts = body_parts + "";
    body_parts = body_parts + "> *Automated review by b00t reviewer autoexec loop. Gate: system-normal precriteria passed.*";

    // Join parts with escaped newlines for gh CLI --body
    let comment_body = "";
    for i in range(0, body_parts.len()) {
        if i > 0 { comment_body = comment_body + "\\n"; }
        comment_body = comment_body + body_parts[i];
    }

    // Post via gh CLI
    try {
        let result = shell(`gh pr comment ${pr_number} --body '${comment_body}' 2>&1`);
        print("   ✅ Review comment posted to PR");
        true
    } catch(e) {
        print(`   ❌ Failed to post comment: ${e}`);
        false
    }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 6: UPDATE QUALITY METRICS
// ═══════════════════════════════════════════════════════════════════════════════
// Appends a JSONL entry to .b00t/reviewer-metrics.jsonl with review statistics.
// Quality thresholds from multi-framework-review.datum.toml [b00t.thresholds]:
//   min_findings_per_100_lines: 1.5
//   min_cross_validated_findings: 1
//   max_false_contradictions: 0
//   min_framework_efficiency_ratio: 0.3

fn update_quality_metrics(findings) {
    print("\n📊 Updating quality metrics...");
    ensure_metrics_dir();

    // Count findings and errors
    let mut phase_count = 0;
    let mut error_count = 0;
    let mut phase_names = [];

    for f in findings {
        phase_count = phase_count + 1;
        if f.contains("phase") {
            phase_names = phase_names + f.phase;
        }
        if f.contains("error") {
            error_count = error_count + 1;
        }
    }

    let phases_json = "";
    for i in range(0, phase_names.len()) {
        if i > 0 { phases_json = phases_json + ","; }
        phases_json = phases_json + `"` + phase_names[i] + `"`;
    }

    let metrics = `{"ts":"${timestamp()}","phases_count":${phase_count},"errors":${error_count},"phases":[${phases_json}],"source":"autoexec"}`;

    try {
        shell(`echo '${metrics}' >> ${METRICS_FILE}`);
        print(`   ✅ Metrics logged to ${METRICS_FILE}`);
        print(`      Phases: ${phase_count} | Errors: ${error_count}`);
    } catch(e) {
        print(`   ❌ Failed to log metrics: ${e}`);
    }
}


// ═══════════════════════════════════════════════════════════════════════════════
// GATE 7: PULSE REPORT
// ═══════════════════════════════════════════════════════════════════════════════
// Reflects on review quality by reading recent metrics and checking against
// quality thresholds from the multi-framework-review datum.
// Part of the continuous-improvement feedback loop.

fn pulse_report() {
    print("\n💓 Pulse — review quality reflection");

    // Read recent metrics for trend analysis
    let recent = shell(`tail -5 ${METRICS_FILE} 2>/dev/null`);

    if recent == "" {
        print("   📭 No metrics data available yet — baseline not established");
        return;
    }

    print("   Last 5 review cycles:");
    let mut cycle_num = 0;
    for line in recent.split("\n") {
        let t = line.trim();
        if t == "" { continue; }
        cycle_num = cycle_num + 1;
        print(`   [${cycle_num}] ${t}`);
    }

    // Quality threshold reference (from multi-framework-review.datum.toml [b00t.thresholds])
    print("");
    print("   ── Quality Thresholds (targets) ──");
    print("   min_findings_per_100_lines:   1.5");
    print("   min_cross_validated_findings:  1");
    print("   max_false_contradictions:      0");
    print("   min_framework_efficiency:     30%");
    print("");
    print("   ✅ Pulse complete — reflect, adapt, improve for next cycle");
}


// ═══════════════════════════════════════════════════════════════════════════════
// MAIN AUTOEXEC LOOP
// ═══════════════════════════════════════════════════════════════════════════════
// Orchestrates the 7-gate review cycle in sequence:
//   1. system-normal precriteria
//   2. Poll review queue
//   3. Risk score each PR
//   4. Dispatch review (sequential or parallel based on tier + size)
//   5. Post results to PR
//   6. Update quality metrics
//   7. Pulse report (reflect)
//
// On gate fail (error severity) → block and log, continue to next cycle.
// On empty queue → sleep and retry.
// CRITICAL-tier PRs → block (requires operator).
//
// Returns array of cycle result objects for audit.

fn autoexec(max_iter) {
    print("╔══════════════════════════════════════════════════════════════╗");
    print("║  b00t REVIEWER AUTOEXEC — Autonomous Review Loop           ║");
    print("║  7-gate cycle: Gate → Poll → Score → Dispatch → Post →     ║");
    print("║  Metrics → Pulse                                           ║");
    print("╚══════════════════════════════════════════════════════════════╝");

    let mut cycle_results = [];

    for i in range(0, max_iter) {
        print(`\n${"=".repeat(60)}`);
        print(`  CYCLE ${i+1}/${max_iter}  |  ${timestamp()}`);
        print(`${"=".repeat(60)}`);

        let cycle = #{
            cycle_num: i + 1,
            timestamp: timestamp(),
            status: "STARTED",
            gate_fail: "",
            prs_reviewed: 0,
            prs_blocked: 0,
            errors: 0
        };

        // ── Gate 1: System-Normal Precriteria ──────────────────────────────────
        print("\n─── Gate 1: system-normal precriteria ───");
        let gate = system_normal_gate();
        if !gate.allowed {
            cycle.status = "BLOCKED";
            cycle.gate_fail = "system-normal";
            cycle_results = cycle_results + cycle;
            return cycle_results;  // Hard block — stop autoexec
        }

        // ── Gate 2: Poll Review Queue ──────────────────────────────────────────
        print("\n─── Gate 2: poll review queue ───");
        let tasks = poll_review_queue();

        if tasks.len() == 0 {
            print("📭 No review tasks found.");
            cycle.status = "IDLE";
            cycle.prs_reviewed = 0;
            cycle_results = cycle_results + cycle;

            print(`\n💤 Sleeping ${SLEEP_SECONDS}s before next cycle...`);
            shell(`sleep ${SLEEP_SECONDS}`);
            continue;
        }

        print(`📋 ${tasks.len()} review task(s) queued:`);
        for t in tasks {
            print(`   PR #${t.number}: ${t.title} (by ${t.author})`);
        }

        // ── Gate 3-6: Process each review task ─────────────────────────────────
        let mut reviewed = 0;
        let mut blocked = 0;
        let mut errors = 0;

        for t in tasks {
            let pr_number = t.number;
            print(`\n${"-".repeat(40)}`);
            print(`  Processing PR #${pr_number}`);
            print(`${"-".repeat(40)}`);

            // Gate 3: Risk Score
            print("\n─── Gate 3: risk-score ───");
            let risk;
            try {
                risk = risk_score_pr(pr_number);
            } catch(e) {
                print(`   ❌ Risk scoring failed: ${e}`);
                errors = errors + 1;
                continue;
            }

            // Gate 4: Dispatch Review
            print("\n─── Gate 4: dispatch review ───");
            let review_result;
            try {
                review_result = dispatch_review(pr_number, risk.tier, risk);
            } catch(e) {
                print(`   ❌ Review dispatch failed: ${e}`);
                errors = errors + 1;
                continue;
            }

            if review_result.status == "BLOCKED" {
                print(`   🚩 Review blocked: ${review_result.reason}`);
                blocked = blocked + 1;
                continue;
            }

            // Gate 5: Post Results
            print("\n─── Gate 5: post results ───");
            let posted = false;
            try {
                posted = post_review_results(pr_number, review_result.findings);
            } catch(e) {
                print(`   ❌ Post failed: ${e}`);
                errors = errors + 1;
            }
            if !posted {
                errors = errors + 1;
            }

            // Gate 6: Update Quality Metrics
            print("\n─── Gate 6: update quality metrics ───");
            try {
                update_quality_metrics(review_result.findings);
            } catch(e) {
                print(`   ❌ Metrics update failed: ${e}`);
                errors = errors + 1;
            }

            reviewed = reviewed + 1;
        }

        cycle.prs_reviewed = reviewed;
        cycle.prs_blocked = blocked;
        cycle.errors = errors;
        cycle.status = if reviewed > 0 { "COMPLETED" } else { "FAILED" };
        cycle_results = cycle_results + cycle;

        // ── Gate 7: Pulse Report ───────────────────────────────────────────────
        pulse_report();

        // ── Inter-cycle sleep ──────────────────────────────────────────────────
        if i < max_iter - 1 {
            print(`\n💤 Sleeping ${SLEEP_SECONDS}s before next cycle...`);
            shell(`sleep ${SLEEP_SECONDS}`);
        }
    }

    // ── Final Summary ──────────────────────────────────────────────────────────
    print("\n╔══════════════════════════════════════════════════════════════╗");
    print("║  AUTOEXEC COMPLETE — Summary                               ║");
    print("╠══════════════════════════════════════════════════════════════╣");
    let mut total_reviewed = 0;
    let mut total_blocked = 0;
    for r in cycle_results {
        total_reviewed = total_reviewed + r.prs_reviewed;
        total_blocked = total_blocked + r.prs_blocked;
        let status_icon = if r.status == "COMPLETED" { "✅" }
                          else if r.status == "IDLE" { "💤" }
                          else if r.status == "BLOCKED" { "🚫" }
                          else { "❌" };
        print(`║  ${status_icon} Cycle ${r.cycle_num}: ${r.status} | reviewed=${r.prs_reviewed} blocked=${r.prs_blocked} errors=${r.errors}`);
    }
    print("╠══════════════════════════════════════════════════════════════╣");
    print(`║  Total: ${cycle_results.len()} cycles | ${total_reviewed} reviewed | ${total_blocked} blocked`);
    print("╚══════════════════════════════════════════════════════════════╝");

    cycle_results
}


// ═══════════════════════════════════════════════════════════════════════════════
// ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════════

let results = autoexec(MAX_ITERATIONS);
print(`\n🏁 autoexec finished — ${results.len()} cycles completed`);
