// DRAFT-git-merge.rhai — b00t hive governance merge protocol
//
// Stage-gated merge process with role-based approval, FOCUS cost accounting,
// and ledgerr audit trail. Each stage emits an AbDataFrame that feeds the next.
//
// Usage: b00t script run DRAFT-git-merge.rhai [--source <branch>] [--target <branch>]
//
// Convention: ALL_CAPS names are b00t context globals or k0mmand3r protocols.
// snake_case names are local RHAI variables or helper functions.

// ─── Stage 0: Pre-flight — resolve source/target ────────────────────────────

let SOURCE = ctx_arg("source", "HEAD");
let TARGET = ctx_arg("target", "main");
let MERGE_ID = run_cmd(`git rev-parse --short ${SOURCE}`).trim();

log_info("🏁 b00t governance merge protocol");
log_info(`  source: ${SOURCE}`);
log_info(`  target: ${TARGET}`);
log_info(`  merge_id: ${MERGE_ID}`);

// ─── Stage 1: Fitness gate — test, lint, build ──────────────────────────────

log_info("─── Stage 1/5: Fitness gate ───");

let fitness_pass = true;

// 1a. Check that the workspace compiles
log_info("  checking compilation...");
let build_result = run_cmd("cargo check --workspace 2>&1");
if build_result.status != 0 {
    log_error(`  BUILD FAILED: ${build_result.stderr}`);
    fitness_pass = false;
}

// 1b. Run unit tests (lib only — integration tests are expensive)
log_info("  running unit tests...");
let test_result = run_cmd("cargo test --lib 2>&1");
if test_result.status != 0 {
    log_error(`  TESTS FAILED: ${test_result.stderr}`);
    fitness_pass = false;
}

// 1c. Run experiments — ensure A/B system still produces valid FOCUS records
log_info("  validating experiment system...");
let exp_result = run_cmd("b00t-cli experiment run --id governance-check --control 'noop' --treatment 'noop' 2>&1");
if exp_result.status != 0 {
    log_error(`  EXPERIMENT SYSTEM FAILED: ${exp_result.stderr}`);
    fitness_pass = false;
}

// 1d. Record fitness frame to audit log
if fitness_pass {
    log_info("  ✅ fitness gate passed");
    run_cmd(`echo '{"ts":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","stage":"fitness","merge_id":"${MERGE_ID}","result":"pass"}' >> .b00t/audit.jsonl`);
} else {
    log_error("  ❌ fitness gate FAILED — aborting merge protocol");
    run_cmd(`echo '{"ts":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","stage":"fitness","merge_id":"${MERGE_ID}","result":"fail"}' >> .b00t/audit.jsonl`);
    throw("fitness gate failed — fix build/test errors before retrying");
}

// ─── Stage 2: k0mmand3r negotiation — blessing check ───────────────────────

log_info("─── Stage 2/5: k0mmand3r negotiation ───");

// The /negotiate protocol checks if the agent holds required blessings.
// If not, it signals /vote to request them.
let blessing_check = run_cmd(`b00t-cli k0mmand3r /negotiate blessing:merge-to-${TARGET} 2>&1`);
if blessing_check.status != 0 {
    log_warn("  blessing 'merge-to-main' not held — requesting via /vote...");
    let vote_result = run_cmd(`b00t-cli k0mmand3r /vote on blessing:merge-to-${TARGET} 2>&1`);
    if vote_result.status != 0 {
        log_error("  ❌ blessing vote FAILED — insufficient quorum");
        throw("merge blocked: /vote did not reach quorum");
    }
    log_info("  ✅ blessing granted via vote");
} else {
    log_info("  ✅ blessing held — proceeding");
}

// ─── Stage 3: Role-based approval gate ─────────────────────────────────────

log_info("─── Stage 3/5: Approval gate ───");

// Resolve current role from KnownRole enum (worker/executive/operator/provider)
let ROLE = run_cmd("b00t-cli whoami 2>&1 | grep '🎭 Role:' | awk '{print $3}'").trim().to_lowercase();
log_info(`  current role: ${ROLE}`);

// Governance: merge to main requires executive or above
let approved = false;
if ROLE == "executive" || ROLE == "provider" {
    approved = true;
    log_info("  ✅ role has merge authority");
} else {
    log_warn("  ⚠️  current role lacks merge authority — escalating to executive");
    let escalate = run_cmd(`b00t-cli k0mmand3r /delegate step:approve-merge to agent:executive 2>&1`);
    if escalate.status != 0 || ROLE != "operator" {
        log_warn("  escalation sent — waiting for /handshake acknowledgement");
        let handshake = run_cmd("b00t-cli k0mmand3r /handshake from agent:executive 2>&1");
        if handshake.status == 0 {
            log_info("  ✅ executive acknowledged — approval assumed");
            approved = true;
        } else {
            log_error("  ❌ no executive handshake — merge blocked");
            throw("merge requires executive approval — no handshake received");
        }
    } else {
        approved = true;
    }
}

// ─── Stage 4: FOCUS cost accounting ─────────────────────────────────────────

log_info("─── Stage 4/5: FOCUS cost accounting ───");

// Record the merge as a FOCUS cost/usage event via ledgerr-mcp.
// The focus_tool handler validates against FocusSchema before persisting.
let focus_record = run_cmd(`b00t-cli experiment run --id merge-${MERGE_ID} --control 'pre-merge ${TARGET}' --treatment 'post-merge ${SOURCE}' 2>&1`);
if focus_record.status == 0 {
    log_info("  ✅ FOCUS record appended to focus_records.jsonl");
    // The [ledgrrr] lines emitted by the experiment command are consumed
    // by ledgerr-mcp's ledgerr_focus/append_focus_record tool when the
    // MCP server is running.
} else {
    log_warn("  ⚠️  FOCUS recording skipped (ledgerr-mcp not running)");
}

// ─── Stage 5: Merge execution ───────────────────────────────────────────────

log_info("─── Stage 5/5: Merge ───");

let merge_result = run_cmd(`git merge ${SOURCE} --no-ff -m "chore(merge): hive-governed merge of ${SOURCE} into ${TARGET}

Merge-ID: ${MERGE_ID}
Governance: fitness=pass role=${ROLE} approved=${approved}
FOCUS: merge-${MERGE_ID}" 2>&1`);

if merge_result.status != 0 {
    log_error(`  ❌ merge CONFLICT: ${merge_result.stderr}`);
    log_info("  run: git merge --abort   # to reset");
    log_info("  run: git mergetool        # to resolve");
    throw("merge conflict — resolve manually, then retry governance protocol");
}

log_info("  ✅ merge committed");

// ─── Post-merge: tag and push proposal ──────────────────────────────────────

let TAG = `merge-${MERGE_ID}`;
run_cmd(`git tag -a ${TAG} -m 'hive-governed merge ${SOURCE}→${TARGET}' 2>&1`);

log_info("");
log_info("🏁 GOVERNANCE MERGE COMPLETE");
log_info(`  merge_id: ${MERGE_ID}`);
log_info(`  source:   ${SOURCE}`);
log_info(`  target:   ${TARGET}`);
log_info(`  tag:      ${TAG}`);
log_info(`  role:     ${ROLE}`);
log_info(`  approved: ${approved}`);
log_info("");
log_info("To push:   git push origin ${TARGET} --tags");
log_info("To revert: git revert $(git rev-parse ${TAG})");
log_info("");
log_info("FOCUS records in:  focus_records.jsonl");
log_info("Audit trail in:    .b00t/audit.jsonl");

// Append final audit frame
run_cmd(`echo '{"ts":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","stage":"merge","merge_id":"${MERGE_ID}","result":"ok","tag":"${TAG}","role":"${ROLE}"}' >> .b00t/audit.jsonl`);
