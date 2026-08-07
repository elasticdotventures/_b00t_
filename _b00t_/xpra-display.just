# _b00t_/xpra-display.just — general-purpose Xpra display service recipes.
# Invoked via `mod xpra-display '_b00t_/xpra-display.just'` in the root justfile,
# e.g.: just xpra-display::spawn "xterm"
#
# ─── general-purpose operator display — NOT owned by b00t-rpa ──────────────
# See _b00t_/xpra-display.hive.toml (this profile) and _b00t_/xpra-rpa.hive.toml
# (the RPA-scoped variant this generalizes). Distinct display :50 / port 10050
# so the two never collide if both happen to be active.

# Attach any GUI app to the running display — no server restart needed per app.
# 🤓 this is the whole point of the general-purpose profile: xpra-rpa launches
#    one browser tied to one session; this lets ANY process attach at ANY time.
spawn cmd:
	#!/usr/bin/env bash
	set -euo pipefail
	command -v xpra >/dev/null || { echo "xpra not installed — see: b00t-cli install xpra"; exit 1; }
	if ! DISPLAY=:50 xdotool getactivewindow >/dev/null 2>&1 && ! xpra info :50 >/dev/null 2>&1; then
		echo "⚠️  xpra-display service doesn't look active on :50 — start it first:"
		echo "   b00t-cli hive activate xpra-display"
		exit 1
	fi
	echo "🖥️  Spawning into xpra display :50: {{cmd}}"
	DISPLAY=:50 {{cmd}} &
	disown
	echo "✅ launched (pid $!) — attach with: just xpra-display::attach-html5"

# Zero-install operator attach: SSH tunnel + point any browser at the HTML5 client.
attach-html5 host=`hostname -I | awk '{print $1}'` user=`whoami`:
	@echo "Run on your operator machine (Windows/WSL2/wherever):"
	@echo "  ssh -L 10050:127.0.0.1:10050 {{user}}@{{host}}"
	@echo "Then open: http://127.0.0.1:10050"

# Power-user operator attach: native xpra client (reconnect-safe, clipboard, seamless windows)
# ⚠️ see xpra.cli.toml's @tribal notes for WSLg gotchas (GDK_BACKEND=x11, --tray=no)
#    before assuming a failed attach is this profile's fault — those are
#    documented, non-obvious client-side bugs, not specific to this service.
attach-native host=`hostname -I | awk '{print $1}'` user=`whoami`:
	GDK_BACKEND=x11 xpra attach "ssh://{{user}}@{{host}}/50" --tray=no

# Report whether the service is up and what's currently attached to it.
status:
	@systemctl --user status b00t-hive-xpra-display.service --no-pager || true
	@echo "---"
	@xpra info :50 2>&1 | head -20 || echo "(xpra info failed — service may not be running)"

# Stop the service.
stop:
	systemctl --user stop b00t-hive-xpra-display.service
