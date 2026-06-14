# Hive Guard Test Logic Bug Analysis

## Problem Identified

The test `test_guard_expr_coverage_all_shipped_datums` has **broken command generation logic** for complex Rhai expressions.

### Root Cause

The test extracts quoted strings from Rhai expressions and tries to map them to test commands, but the mapping logic is incomplete:

**For guard:** `cmd.contains("git checkout -b") && !cmd.contains("/")`

**What test does:**
1. Extract keywords: `["git checkout -b", "/"]`  
2. Iterate keywords looking for matches
3. `"git checkout -b"` doesn't match simple keyword `"git"`
4. Falls through to default: `"trigger-command-match install"`

**Expected:** Generate `"git checkout -b branchname"`  
**Actual:** Generates `"trigger-command-match install"`  
**Result:** Guard returns Allow (correct!) but test expects Warn

### Test Logic Bug Flow

```rust
// Line 1465-1496: Broken keyword mapping
for keyword in &keywords {
    match keyword.as_str() {
        "pip" | "pip3" => format!("{} install somepackage"),
        "docker" => "docker run nginx",
        "git" => "git push --force origin main",  // <-- NEVER REACHED
        // ...
        _ => "trigger-command-match".to_string(), // <-- ALWAYS EXECUTED
    }
}
```

Since `"git checkout -b"` != `"git"`, the match never succeeds and falls through to default.

### Failing Guards (All Test Bugs, Not Guard Bugs)

1. **Line 13:** `K0mmand3rStage { stage: "pre_parse" }`  
2. **Line 18:** `cmd.contains("git checkout -b") && !cmd.contains("/")`  
3. **Line 19:** `cmd.contains("git commit") && cmd.contains("-m") && !cmd.contains(":")`

All guards work correctly; the test just doesn't generate proper trigger commands.

## Fix Strategy

### Option 1: Improve Command Generation (Preferred)
Enhance the test's keyword extraction and mapping logic to handle complex expressions:

```rust
// Better keyword extraction and mapping
GuardPattern::RhaiExpr(expr) => {
    // Parse the expression structure more intelligently
    if expr.contains("git checkout -b") {
        "git checkout -b feature-branch"
    } else if expr.contains("git commit") && expr.contains("-m") && expr.contains(":") {
        "git commit -m feat: add feature"
    } else if expr.contains("git") {
        "git push origin main"
    } else {
        // fallback
        extract_and_map_keywords(expr)
    }
}
```

### Option 2: Skip Complex Guards in Test
Mark complex guards as "validated manually" and skip auto-generation:

```rust
// Skip complex multi-condition guards
if expr.contains("&&") {
    continue; // Manual validation only
}
```

### Option 3: Add Test Command Override
Allow tests to specify expected trigger commands directly in TOML:

```toml
[[b00t.hive.guards]]
pattern = { rhai = "cmd.contains(\"git checkout -b\") && !cmd.contains(\"/\")" }
action = "warn"
test_command = "git checkout -b simple-branch"  # New field
```

## Recommendation

**Fix Option 1** - Improve the command generation logic to properly parse and map complex Rhai expressions. This will make the test suite more robust and catch actual guard logic bugs.

## Sources

- [arXiv: Systematic Approach for LLM Debugging](https://arxiv.org/html/2604.23027v1) - Observable systems methodology
- [Test code analysis](b00t-cli/src/hive.rs:1465-1496) - Broken keyword mapping logic
- [Guard definitions](/_b00t_/hive-guards.hive.toml) - Actual working guards

---

**Agent:** First-mate systematic debugging  
**Branch:** `fix/hive-guard-test-logic`  
**Finding:** Test implementation bug, not guard logic bug  
**Next Step:** Implement improved command generation logic