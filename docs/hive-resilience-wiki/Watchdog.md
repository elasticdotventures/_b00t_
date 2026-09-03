# A · Watchdog — the Sentinel  ✅

Shipped on branch `ralph/hive-watchdog` (commit `feat: hive Sentinel …`).

## What it is

`scripts/b00t-hive-watchdog.sh` — a poll loop (`WATCHDOG_INTERVAL_SECS`, default 15):

1. **Census** — every `b00t-hive-*.service` + `b00t@*.service`: `NRestarts`,
   `ActiveState`, `SubState`, `ExecMainStatus` → one JSON line/unit into
   `.b00t/hive-watchdog.jsonl`.
2. **Loop-break** — per-unit `NRestarts` delta tracked in
   `$XDG_RUNTIME_DIR/b00t-watchdog.state`. Delta ≥ `WATCHDOG_MAX_RESTARTS` (5)
   within `WATCHDOG_WINDOW_SECS` (120) → `systemctl --user stop <unit>` + a
   `{"event":"crashloop",…,"action":"stopped"}` line. Own unit is exempt.
3. **Herald** — best-effort `nats pub` to `b00t.hive.mesh.health.crashloop`
   (on trip) and `b00t.hive.mesh.health.snapshot` (every cycle). NATS down ≠ loop fail.

`WATCHDOG_ONESHOT=1` → one cycle, exit 0 (used by tests).
`WATCHDOG_EXTRA_UNITS="u.service …"` → extra units to watch (test hook).

## Standing orders

`_b00t_/hive-watchdog.hive.toml` → `[b00t.hive.service]` `Restart=always` →
`b00t-hive-hive-watchdog.service`. Activate: `just hive-watchdog-ensure`
(`b00t-cli hive activate hive-watchdog`). Cross-ref note in
`_b00t_/hive-guards.hive.toml`.

## Verified

- oneshot census: 12 units, all lines valid JSON
- canary crash-loop: stopped at `delta=3`, `crashloop` event logged
- NATS subscriber received `b00t.hive.mesh.health.snapshot`

## Open

- `shellcheck` not installed on the host — only `bash -n` + functional test ran. [[Agent-Doctor]] story could add it.
- profile name `hive-watchdog` → doubled unit `b00t-hive-hive-watchdog.service`. Rename profile to `watchdog` if it grates.
- not yet `hive activate`d in production — [[Repair-Chain]] or operator does that.

See also: [[Health-Metrics]] consumes the `b00t.hive.mesh.health.*` subjects this page produces.
