// bouncer-review.rhai - state machine for Agent A dev + Agent B review
// Usage: b00t script run bouncer-review [context.md]
let ctx_file = if ARGS.len() > 0 { ARGS[0] } else { "context.md" };
let sf = ".bouncer-state.toml";

// Assert: we're in the expected workdir
let expected_prefix = "/home/brianh/.b00t";
let cwd = get_env("PWD");
if !cwd.starts_with(expected_prefix) {
    log_warn(`Workdir drift detected! Expected ${expected_prefix}*, got ${cwd}. Files may land in wrong location.`);
}

fn toml_val(s, sec, key) {
    let in_sec = false;
    for line in s.split("\n") {
        let t = line.trim();
        if t == "[" + sec + "]" { in_sec = true; continue; }
        if in_sec && t.starts_with("[") && t != "[" + sec + "]" { in_sec = false; }
        if in_sec && t.starts_with(key + " = ") { return t.sub_string(key.len() + 3).trim_matches("\""); }
    }
    return "";
}
fn save(phase, turn, max_t, a_rdy, b_app, fixes, rnd) {
    write_file(sf, `[meta]\ncreated = "${get_env("TIMESTAMP")}"\ncontext = "${ctx_file}"\n\n[state]\nphase = "${phase}"\nturn = ${turn}\nmax = ${max_t}\n\n[results]\na_ready = ${a_rdy}\nb_approved = ${b_app}\nfixes = ${fixes}\nround = ${rnd}\n`);
    log_info(`CHECKPOINT: ${sf} written — phase=${phase} turn=${turn} round=${rnd}. Resume: b00t script run bouncer-review ${ctx_file}`);
}

let phase = "research"; let turn = 0; let max_t = 5;
let a_rdy = false; let b_app = false; let fixes = 0; let rnd = 0;
if file_exists(sf) {
    let s = read_file(sf);
    phase = toml_val(s, "state", "phase");
    turn = parse_int(toml_val(s, "state", "turn"));
    max_t = parse_int(toml_val(s, "state", "max"));
    a_rdy = toml_val(s, "results", "a_ready") == "true";
    b_app = toml_val(s, "results", "b_approved") == "true";
    fixes = parse_int(toml_val(s, "results", "fixes"));
    rnd = parse_int(toml_val(s, "results", "round"));
}
log_info(`Bouncer-Review | ${phase} | turn ${turn}/${max_t} | round ${rnd}`);

if phase == "research" {
    log_info("Phase A-1: Agent A research + tests + stubs");
    log_info(`Context: ${ctx_file}`);
    save("development", turn, max_t, a_rdy, b_app, fixes, rnd);
} else if phase == "development" {
    if a_rdy {
        log_success("Agent A ready -> review");
        save("review", turn, max_t, a_rdy, b_app, fixes, rnd);
    } else if turn < max_t {
        turn += 1;
        log_info(`Ralph iter ${turn}/${max_t}: small step, pass >=1 test`);
        save("development", turn, max_t, a_rdy, b_app, fixes, rnd);
    } else {
        log_warn("max turns -> forcing review");
        save("review", turn, max_t, true, b_app, fixes, rnd);
    }
} else if phase == "review" {
    if b_app {
        log_success("APPROVED");
        save("approved", turn, max_t, a_rdy, b_app, fixes, rnd);
    } else if rnd >= 3 {
        log_warn("max review rounds -> escalate");
        save("escalated", turn, max_t, a_rdy, b_app, fixes, rnd);
    } else {
        rnd += 1;
        log_info(`B review round ${rnd}: critical review + ecosystem check`);
        fixes += 1;
        log_info("B rejected -> A fixes -> loop");
        save("development", turn, max_t, false, b_app, fixes, rnd);
    }
} else if phase == "approved" {
    log_success("=== DONE ===");
    log_info(`${turn} turns, ${rnd} reviews, ${fixes} fixes`);
} else if phase == "escalated" {
    log_warn("=== ESCALATED: operator must decide ===");
}
log_info(`State: cat ${sf}`);
return phase;
