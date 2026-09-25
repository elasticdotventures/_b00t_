# ── pi-agent — pi coding agent ⇄ b00t integration ───────────────────────────
# 🤓 SINGLE SOURCE OF TRUTH for the commands pi.agent needs to reach b00t.
#    Datums describe; this file executes. `b00t install pi` shells into here so
#    the datum graph and the real command surface cannot drift apart.
#
# ⚠️需 DIVISION OF LABOUR — provisioning is NOT duplicated here:
#    • PiAdapter (b00t-cli/src/install/runtimes/pi.rs, RuntimeId::Pi) owns all
#      PROVISIONING: pi packages via `pi install`, settings.json merge, mcp.json
#      merge (b00t-mcp + codebase-memory), the --stdio invariant, and
#      b00t-manifest.json. Fragments: _b00t_/runtimes/pi/*.json
#    • This justfile owns the pi BINARY (npm), VERIFICATION, service lifecycle,
#      and delegation. It delegates provisioning to the adapter rather than
#      reimplementing it.
#
# Canonical datum chain (each fact stated exactly once — see pi.polyseme.toml):
#   pi.agent              → agent-harness facet; install delegates to PiAdapter
#   pi-coding-agent.cli   → the `pi` npm binary (ONLY place install/version live)
#   pi-mcp-adapter.cli    → the MCP client extension (ONLY place)
#   pi-agent.justfile     → THIS file, registered as a b00t justfile datum
#
# Invocation:  just pi-agent::install | just pi-agent::check | just pi-agent::probe
# Via MCP:     b00t_just({ just: "pi-agent::check" })
#
# ⚠️需 b00t-mcp MUST be launched with --stdio. With args [] it takes the HTTP
#    path: syncs the official MCP registry over the network, spawns `npx`
#    bridges for every stdio entry (recursively bridging pi itself), prints
#    usage, and exits. PiAdapter::enforce_stdio() repairs violations on install.

set shell := ["bash", "-cu"]

# pi npm package — canonical name, stated HERE only.
# ⚠️需 @mariozechner/pi-coding-agent is STALE (frozen at 0.73.1). The maintained
#    package is @earendil-works/pi-coding-agent (0.85.1 verified 2026-09-11).
PI_PKG := "@earendil-works/pi-coding-agent"
PI_DIR := env_var_or_default("PI_CODING_AGENT_DIR", env("HOME") + "/.pi/agent")
PI_MCP_JSON := PI_DIR + "/mcp.json"
PI_SETTINGS := PI_DIR + "/settings.json"

# Full install: pi binary → PiAdapter provisioning → verification.
# 🤓 This is what `b00t install pi` resolves to (via pi.agent + pi-coding-agent.cli).
install: install-pi install-runtime check
    @echo "✅ pi ⇄ b00t integration complete"
    @echo "   next: pi   (then /mcp status, or call b00t_discover)"

# Install or update the pi binary. The ONLY place the npm package is named.
install-pi:
    @echo "📦 installing {{PI_PKG}}@latest ..."
    npm install -g {{PI_PKG}}@latest
    @echo "✅ pi $(pi --version)"

# Provision pi's runtime through the registered PiAdapter handler set:
# pi packages (`pi install npm:pi-mcp-adapter`), settings.json, mcp.json
# (b00t-mcp + codebase-memory), --stdio enforcement, install manifest.
# 🤓 Additive merge — user packages, servers, and settings are preserved.
install-runtime:
    @echo "🧩 provisioning pi runtime via PiAdapter ..."
    b00t-cli install --runtimes pi --scope global -y
    @echo "✅ pi runtime provisioned (manifest: {{PI_DIR}}/b00t-manifest.json)"

# Thin alias — the adapter installs the MCP client extension. Kept as a named
# entry point because pi-mcp-adapter.cli.toml references it.
install-adapter: install-runtime

# Verify the whole chain and print a PASS/FAIL evidence line.
# 🤓 Verification only — provisions nothing, so it never races the adapter.
check:
    #!/usr/bin/env bash
    set -uo pipefail
    fail=0
    ok()  { echo "  ✅ $1"; }
    bad() { echo "  ❌ $1"; fail=1; }
    echo "🩺 pi-agent integration check"

    command -v pi >/dev/null 2>&1 \
      && ok "pi $(pi --version 2>/dev/null | head -1)" \
      || bad "pi not on PATH — just pi-agent::install-pi"

    case "$(npm ls -g --depth=0 2>/dev/null)" in
      *@earendil-works/pi-coding-agent*) ok "npm pkg @earendil-works/pi-coding-agent" ;;
      *@mariozechner/pi-coding-agent*)   bad "STALE pkg @mariozechner — just pi-agent::install-pi" ;;
      *)                                  bad "pi npm package not found globally" ;;
    esac

    # PiAdapter must be compiled into the b00t-cli on PATH. The unknown-runtime
    # error prints the valid token list, so it doubles as a capability probe.
    # ⚠️需 capture-then-grep, NOT `cmd | grep`: that command exits 1 by design,
    #    and with `set -o pipefail` the pipeline status would be 1 even on a
    #    successful match — a silent false negative.
    rt_probe=$(b00t-cli install --runtimes __probe__ 2>&1 || true)
    if grep -q '\bpi\b' <<<"$rt_probe"; then
      ok "b00t-cli has the pi runtime adapter (RuntimeId::Pi)"
    else
      bad "b00t-cli lacks RuntimeId::Pi — rebuild: cargo build --release -p b00t-cli"
    fi

    pkg_list=$(pi list 2>/dev/null || true)
    if grep -q pi-mcp-adapter <<<"$pkg_list"; then
      ok "pi-mcp-adapter package registered"
    else
      bad "pi-mcp-adapter missing — just pi-agent::install-runtime"
    fi

    if [ -f "{{PI_MCP_JSON}}" ]; then
      ok "{{PI_MCP_JSON}} exists"
      python3 - "{{PI_MCP_JSON}}" <<'PY'
    import json, sys
    try:
        cfg = json.load(open(sys.argv[1]))
    except json.JSONDecodeError as e:
        print(f"  ❌ invalid JSON: {e}"); sys.exit(1)
    srv = cfg.get("mcpServers", {})
    rc = 0
    b = srv.get("b00t-mcp")
    if not b:
        print("  ❌ no b00t-mcp server entry"); rc = 1
    elif b.get("args") != ["--stdio"]:
        print(f"  ❌ b00t-mcp args={b.get('args')} — must be ['--stdio']"); rc = 1
    else:
        print(f"  ✅ b00t-mcp stdio, directTools={b.get('directTools')}")
    if "codebase-memory" in srv:
        print(f"  ✅ codebase-memory wired (directTools={srv['codebase-memory'].get('directTools')})")
    else:
        print("  ⚠️ codebase-memory not wired (optional)")
    leaked = [k for k in cfg if k.startswith("_") and k != "_note"]
    if leaked:
        print(f"  ❌ fragment comment keys leaked into config: {leaked}"); rc = 1
    sys.exit(rc)
    PY
      [ $? -eq 0 ] || fail=1
    else
      bad "{{PI_MCP_JSON}} missing — just pi-agent::install-runtime"
    fi

    command -v b00t-mcp >/dev/null 2>&1 \
      && ok "b00t-mcp binary" || bad "b00t-mcp missing — b00t install b00t-mcp"
    command -v b00t-cli >/dev/null 2>&1 \
      && ok "b00t-cli binary" || bad "b00t-cli missing"

    [ $fail -eq 0 ] && echo "PASS pi-agent check" || echo "FAIL pi-agent check"
    exit $fail

# Raw MCP stdio handshake — proves b00t-mcp serves tools/list without pi.
# 🤓 Emits a trace-worthy evidence line: "PASS b00t-mcp stdio: N tools".
probe server="b00t-mcp":
    #!/usr/bin/env bash
    set -uo pipefail
    case "{{server}}" in
      b00t-mcp)         cmd=(b00t-mcp --stdio) ;;
      codebase-memory)  cmd=(codebase-memory-mcp) ;;   # 🤓 no args = stdio here
      *) echo "unknown server '{{server}}' (b00t-mcp|codebase-memory)"; exit 2 ;;
    esac
    out=$( { printf '%s\n' \
      '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"b00t-probe","version":"1"}}}' \
      '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
      '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
      sleep 6; } | timeout 60 "${cmd[@]}" 2>/dev/null )
    n=$(printf '%s' "$out" | python3 -c '
    import json,sys
    for line in sys.stdin:
        try: o=json.loads(line)
        except Exception: continue
        if o.get("id")==2: print(len(o.get("result",{}).get("tools",[]))); break
    else: print(0)')
    if [ "${n:-0}" -gt 0 ]; then
      echo "PASS {{server}} stdio: $n tools"
      printf '%s' "$out" | python3 -c '
    import json,sys
    for line in sys.stdin:
        try: o=json.loads(line)
        except Exception: continue
        if o.get("id")==2:
            for t in o["result"]["tools"]: print("   -", t["name"])
            break'
    else
      echo "FAIL {{server}} stdio: no tools/list response"
      exit 1
    fi

# Probe every wired MCP server.
probe-all: (probe "b00t-mcp") (probe "codebase-memory")
    @echo "PASS all wired MCP servers respond to tools/list"

# End-to-end: pi → adapter → b00t-mcp → b00t-cli. Costs one cheap LLM call.
e2e model="gemini-3.5-flash-lite" provider="google":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🧪 e2e via {{provider}}/{{model}}"
    pi -p --no-session --provider {{provider}} --model {{model}} \
      "Call the b00t_discover tool with query 'task' and limit 3. Reply with only the tool names returned." \
      2>&1 | grep -v "GOOGLE_API_KEY" | tail -12

# Show pi's view of the wiring (read-only).
status:
    @echo "── pi list ──";         pi list 2>&1 | head -10
    @echo "── {{PI_SETTINGS}} ──"; cat {{PI_SETTINGS}} 2>/dev/null || echo "(missing)"
    @echo "── {{PI_MCP_JSON}} ──"; cat {{PI_MCP_JSON}} 2>/dev/null || echo "(missing)"

# Update pi itself and all installed pi packages/extensions.
update: install-pi
    pi update --extensions || echo "⚠️ extension update skipped"
    @echo "✅ updated: pi $(pi --version)"

# Reverse the pi-side wiring.
# 🤓 Delegates to the manifest-aware runtime uninstall — the exact inverse of
#    install-runtime. PiAdapter::uninstall() strips only WIRED_MCP_SERVERS and
#    b00t-injected settings keys, deletes files whose SHA256 still matches the
#    manifest, and backs up anything the operator modified. No logic duplicated
#    here, so the two directions cannot drift.
uninstall:
    b00t-cli uninstall --runtimes pi --scope global -y
    -pi remove npm:pi-mcp-adapter
    @echo "✅ pi runtime unwired (b00t-managed files removed, user content preserved)"

# Load the pi learn blessing.
learn:
    b00t learn pi-coding-agent

# Show the canonical datum for a pi facet (default: the polyseme branch point).
datum name="pi":
    b00t datum show {{name}}

# ── hive service lifecycle (b00t@pi-agent.service) ──────────────────────────
service-start:
    systemctl --user start b00t@pi-agent.service && systemctl --user --no-pager status b00t@pi-agent.service | head -8

service-stop:
    systemctl --user stop b00t@pi-agent.service

service-status:
    systemctl --user is-active b00t@pi-agent.service || true

# One-shot delegation to pi (non-interactive). Swap provider/model for local ch0nky.
delegate task="" provider="google" model="gemini-3.5-flash-lite":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -z "{{task}}" ] && { echo "usage: just pi-agent::delegate task='<description>'"; exit 1; }
    pi -p --provider {{provider}} --model {{model}} "{{task}}"

# b00t:map v1
# summary: pi-agent justfile — verification + binary + service surface for pi; provisioning delegated to PiAdapter (RuntimeId::Pi)
# tags: pi, pi-agent, agent-harness, b00t-mcp, codebase-memory, mcp, justfile, runtime-adapter, install
# tier: sm0l
# cmds: just pi-agent::install, just pi-agent::check, just pi-agent::probe, just pi-agent::probe-all
# complexity: 5
