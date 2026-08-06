# b00t-rpa module — browser RPA recipes.
# Invoked from b00t-cli/justfile via `mod rpa`, e.g.: just rpa::xpra-start
#
# ─── xpra: operator display for headed RPA browser sessions ────────────────
# See _b00t_/xpra-rpa.hive.toml and _b00t_/xpra.cli.toml. b00t-rpa/browser-use
# launches a *visible* Chromium so the operator can watch/intervene; this
# wraps that window in an Xpra seamless session instead of raw X11Forwarding.

# Start an Xpra-backed headed browser session. browser_cmd is the actual
# browser-use/Playwright headed invocation; defaults to a bare chromium launch.
# 🤓 `xpra start` is seamless-by-default (per-window, not a full desktop) —
#    there's no separate `xpra seamless` subcommand, that naming is docs-only.
# 🤓 requires the `xpra-x11` subpackage on 6.x (xpra.org repo): the base
#    `xpra` meta-package alone fails with "you must install `xpra-x11` to use
#    'seamless'". See _b00t_/xpra.cli.toml [install] for the full bootstrap
#    (xpra-server xpra-x11 xpra-html5 xpra-codecs) — confirmed 2026-08-04.
xpra-start browser_cmd="chromium --no-sandbox":
	#!/usr/bin/env bash
	set -euo pipefail
	command -v xpra >/dev/null || { echo "xpra not installed — see: b00t-cli install xpra"; exit 1; }
	echo "🖥️  Starting xpra :100 with: {{browser_cmd}}"
	xpra start :100 \
		--start-child="{{browser_cmd}}" \
		--exit-with-children=yes \
		--bind-tcp=127.0.0.1:10000 \
		--html=on \
		--daemon=no

# Zero-install operator attach: SSH tunnel + point any browser at the HTML5 client.
# host defaults to this node's primary LAN IP (not the mDNS hostname — WSL2
# clients often can't resolve .local names); user defaults to `whoami` — both
# resolve to real, copy-pasteable values, nothing hardcoded in the datum.
xpra-attach-html5 host=`hostname -I | awk '{print $1}'` user=`whoami`:
	@echo "Run on your operator machine (Windows/WSL2/wherever):"
	@echo "  ssh -L 10000:127.0.0.1:10000 {{user}}@{{host}}"
	@echo "Then open: http://127.0.0.1:10000"

# Power-user operator attach: native xpra client (reconnect-safe, clipboard, seamless windows)
# ⚠️ from WSL2 this still needs somewhere to render (WSLg, or a Windows X
#    server) — it is NOT a fix for "no X server on Windows"; use xpra-attach-html5
#    for that case. This is for operators with a real local display already.
# 🤓 --tray=no is load-bearing, not cosmetic: on the Ubuntu-packaged xpra 3.1.5
#    client, tray setup under an environment with no dbus/systemd (e.g. WSLg)
#    hits a real bug — os_util.py's get_saved_env_var() references an
#    undefined `_saved_env` (should be `saved_env`) inside the AppIndicator
#    fallback path, and the resulting NameError kills the whole connection
#    ("Connection lost"), not just the tray icon. Fixed by upgrading to the
#    xpra.org repo build (6.5.2), which doesn't hit this bug.
# 🤓 GDK_BACKEND=x11 is ALSO load-bearing under WSLg: the 6.x GTK3 client
#    auto-detects WSLg's Weston Wayland compositor and renders natively, but
#    its keyboard XKB-layout query unconditionally uses X11 root-window
#    bindings and crashes ("cannot load X11 bindings on non-X11 platform")
#    instead of falling back gracefully. WSLg bundles XWayland alongside
#    Weston for exactly this case — forcing GTK onto it sidesteps the crash.
#    Confirmed to fix the crash 2026-08-04; the overall flow is still
#    unconfirmed past "window renders" — see xpra.cli.toml's RESUME HERE
#    note. This work is paused/tabled for now.
xpra-attach-native host=`hostname -I | awk '{print $1}'` user=`whoami`:
	GDK_BACKEND=x11 xpra attach "ssh://{{user}}@{{host}}/100" --tray=no

xpra-stop:
	xpra stop :100
