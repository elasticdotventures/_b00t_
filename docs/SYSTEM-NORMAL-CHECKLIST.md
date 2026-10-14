# Read-only baseline health checklist

`just system-normal-test` tests bounded probe evaluation, real isolated Git
repositories and the actual CLI's Satisfied/Violated/Unknown exit contract.
`b00t -p _b00t_ is system-normal --json` evaluates the checked-in baseline.
Install the datum and its `scripts/system-normal-probe.py` companion together
when using a different datum root.

The baseline checks Git, Just, Rust, Cargo, uv and Python availability, then
observes unmerged entries, branch identity, retained stashes and submodule pin
state. It performs no repairs or stash deletion. Uninitialized submodules are
Unknown; mismatched pins and unresolved Git state are Violated. Command
presence establishes dependency availability only. Project checklists should
declare their own live service probes rather than treating command presence
as daemon, authentication or model-server health.

An explicit `[b00t.check.probe]` declares `argv` and optional `timeout_ms`.
It executes directly, without an implicit shell, with the datum directory as
its working directory and no inherited input or captured output. Exit 0 means
the observed condition is Satisfied, 1 means it is Violated, and every other
exit, signal, spawn failure or deadline failure means Unknown. A missing
deadline defaults to 3000 milliseconds; values outside 1..=60000 are Unknown.
The evaluator kills and reaps its direct child on timeout. Probe authors must
also bound any child processes they create; the supplied Git observer uses
two-second subprocess deadlines. Probe output is suppressed to keep credentials
and diagnostics out of checklist responses.

Known violations dominate unavailable evidence in the existing implicit-AND
aggregate. The CLI exits 0/1/2 respectively; it never collapses Unknown into a
successful baseline.

The datum's `probe_gates_supported()` Rhai check is a compatibility guard.
Older executables ignore the new `probe` field, but cannot evaluate this
capability check and therefore return Unknown instead of an incorrect
Satisfied aggregate. Upgrade the executable before relying on probe results.
