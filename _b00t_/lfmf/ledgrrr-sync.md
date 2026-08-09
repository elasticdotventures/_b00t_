# ledgrrr-sync

A vendored submodule's checked-out HEAD matching its recorded `.gitmodules`
pin proves nothing about whether that pin is still reachable from the
branch `.gitmodules` declares (`branch = ...`). If the declared branch gets
force-pushed/rebased upstream after the pin was set, the pin silently
strands on an orphan lineage — `check-submodule-drift.sh` reported
"ok" the whole time because it only ever compares checked-out HEAD against
the recorded pin, never the pin against the declared branch's current tip.

This is exactly what happened to `vendor/ledgrrr`: a series of "bump
vendor/ledgrrr for X fix" commits pinned it straight onto a feature branch
(`fix/tray-native-windows-rs-062`) instead of the declared `b00t-patches`.
`b00t-patches` was later force-pushed, and the two lineages diverged for
weeks — 34 commits on one side, 3 on the other, including two independent
fixes for the same bug (`windows_registry::*` shadowing `std::result::Result`)
written on each side without either author knowing about the other's fix.
Caught only by a manual `git cherry`/`merge-base --is-ancestor` audit, not
by any existing check.

Fix-forward, don't fast-forward: when a pin and its declared branch have
diverged, merge both lineages together upstream (resolve conflicts keeping
the more complete/correct side — see PromptExecution/ledgrrr#167 for the
worked example), verify it builds, *then* re-bump the pin. Never just move
the pin to the declared branch's current tip — that silently drops whatever
commits the old pin had that never made it onto that branch.

`check-submodule-drift.sh` (and `b00t doctor check`'s `submodule-drift`
entry) now check this directly: every submodule with a `branch=` in
`.gitmodules` gets a `branch_status` field — `ok` (pin is an ancestor of
`origin/<branch>`), `stale` (pin unreachable from it — the failure mode
above, counted as a doctor-check failure), `unknown` (declared branch not
fetched locally yet — inconclusive, not a failure), or `n/a` (no branch
declared). It's local-only by design (never fetches over the network at
check time — a network call per submodule on every `doctor check` would be
slow, and per the same day's WSL2 + Cloudflare WARP MTU-blackhole incident,
can hang indefinitely) — run `git -C <path> fetch origin <branch>`
periodically to keep it informative rather than `unknown`.
