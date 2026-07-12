// reviewer-system-normal.rhai — precriteria gate for reviewer operations
//
// Mirrors _b00t_/datums/SYSTEM-NORMAL.tomllmd checks in RHAI syntax.
// Called by reviewer-autoexec.rhai before the review loop starts.
// Also callable via: just reviewer system-normal
//
// Returns: { status: "PASS"|"WARN"|"FAIL", checks: [...], summary, pass_count, fail_count }
// Each check: { id, label, passed: bool, severity: "error"|"warn", message }

// ── Helper: run shell command and return trimmed output ───────────────────────

fn shell(cmd) {
    try {
        run_cmd(cmd).trim()
    } catch(e) {
        ""
    }
}

// ── Helper: simple exact-match check ──────────────────────────────────────────

fn check_exact(id, label, cmd, expected, severity, fail_msg) {
    let output = shell(cmd);
    let passed = output == expected;
    #{
        id: id, label: label, passed: passed, severity: severity,
        message: if passed { "" } else { fail_msg }
    }
}

// ── PRECRITERIA CHECKS ────────────────────────────────────────────────────────

// Check 1: git stash must be empty (warn)
let c1 = check_exact(
    "git-stash-reviewed",
    "git stash queue reviewed and empty",
    "git stash list 2>/dev/null | wc -l | tr -d ' '",
    "0",
    "warn",
    "git stash has entries — REVIEW before dropping; stashes may contain valuable WIP"
);

// Check 2: no merge/rebase conflict markers (error — blocks on fail)
let c2_cmd = "git status --porcelain 2>/dev/null | grep -c '^UU\\|^AA\\|^DD' || echo 0";
let c2_output = shell(c2_cmd);
let c2 = #{
    id: "git-no-merge-conflict",
    label: "no active merge/rebase conflict markers",
    passed: c2_output == "0",
    severity: "error",
    message: if c2_output == "0" { "" } else { "unresolved merge/rebase conflicts present — resolve before continuing" }
};

// Check 3: not in detached HEAD (warn — uses expected_contains: "ok")
let c3_cmd = "git symbolic-ref --short HEAD 2>/dev/null && echo ok || echo detached";
let c3_output = shell(c3_cmd);
let c3 = #{
    id: "git-not-detached-head",
    label: "not in detached HEAD state",
    passed: c3_output.contains("ok"),
    severity: "warn",
    message: if c3_output.contains("ok") { "" } else { "detached HEAD — checkout a branch: git checkout -b task/<slug>" }
};

// Check 4: not on main branch (warn — uses expected_not: "main")
let c4_output = shell("git branch --show-current 2>/dev/null");
let c4 = #{
    id: "no-uncommitted-on-main",
    label: "not on main branch",
    passed: c4_output != "main",
    severity: "warn",
    message: if c4_output != "main" { "" } else { "on main branch — always work on a task branch: git checkout -b task/<id>-<slug>" }
};

// Check 5: submodules in sync (warn)
let c5 = check_exact(
    "submodules-in-sync",
    "all submodules in sync",
    "git submodule status 2>/dev/null | grep -c '^[+M]' || echo 0",
    "0",
    "warn",
    "submodule(s) have unpushed commits or dirty trees — sync before task operations"
);

// ── AGGREGATE RESULTS ─────────────────────────────────────────────────────────

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

// ── FORMATTED REPORT ──────────────────────────────────────────────────────────

print("╔══════════════════════════════════════════════════════════════╗");
print("║  SYSTEM-NORMAL PRECRITERIA — Reviewer Gate                  ║");
print("╠══════════════════════════════════════════════════════════════╣");

for c in checks {
    let icon = if c.passed { "✅" } else { "❌" };
    let sev = if c.passed { "" } else { " [" + c.severity + "]" };
    print(`║ ${icon} ${c.label}${sev}`);
}

print("╠══════════════════════════════════════════════════════════════╣");
print(`║  RESULT: ${status} — ${summary}`);
print("╚══════════════════════════════════════════════════════════════╝");

// Print failure details if any checks failed
if has_error || has_warn {
    print("");
    print("─── Details ───");
    for c in checks {
        if !c.passed {
            print(`[${c.severity}] ${c.id}: ${c.message}`);
        }
    }
}

// ── RETURN STRUCTURED RESULT ──────────────────────────────────────────────────

#{
    status: status,
    checks: checks,
    summary: summary,
    pass_count: pass_count,
    fail_count: fail_count
}
