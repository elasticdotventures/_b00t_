#!/usr/bin/env bash
# b00t-lite: minimal bootstrap — reads install commands from _b00t_/b00t-lite.tomllm datum
#
# Usage:
#   ./b00t-lite.sh                    # detect OS, read datum, run install commands
#   ./b00t-lite.sh --dry-run          # print commands without executing
#   ./b00t-lite.sh --list             # list available OS keys in datum
#   B00T_DIR=/path/to/b00t ./b00t-lite.sh
#
# Design:
#   1. Detect OS/distro → OS_KEY (e.g. linux_debian, linux_arch, darwin)
#   2. Bootstrap a minimal TOML parser (python3.11+ tomllib, or toml-cli via cargo)
#   3. Read _b00t_/b00t-lite.tomllm — valid TOML, no stripping needed
#   4. Execute: sudo_cmds (with _prompt_timeout), then user_cmds unconditionally
#   5. OS_KEY falls back to linux_unknown for unsupported distros

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────
B00T_DIR="${B00T_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
DATUM="${B00T_DIR}/_b00t_/b00t-lite.tomllm"
DRY_RUN="${DRY_RUN:-false}"
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=true
[[ "${1:-}" == "--list"    ]] && LIST_ONLY=true || LIST_ONLY=false

# ── OS detection ──────────────────────────────────────────────────────────────
_detect_os_key() {
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "darwin"
    elif [[ -f /etc/os-release ]]; then
        # shellcheck source=/dev/null
        local id; id=$(. /etc/os-release && echo "${ID:-unknown}")
        echo "linux_${id}"
    else
        echo "linux_unknown"
    fi
}

# ── TOML parser bootstrap ─────────────────────────────────────────────────────
# 🤓 .tomllm IS valid TOML (# is standard TOML comment syntax); no stripping needed.
#    Preference: python3.11+ stdlib tomllib (zero deps); fallback: toml-cli (cargo).
_ensure_toml_parser() {
    if python3 -c "import tomllib" 2>/dev/null; then
        TOML_PARSER="python"
        return 0
    fi
    if command -v toml >/dev/null 2>&1; then
        TOML_PARSER="toml-cli"
        return 0
    fi
    echo "  ℹ️  bootstrapping toml-cli (requires cargo)..."
    if command -v cargo >/dev/null 2>&1; then
        cargo install toml-cli --quiet
        TOML_PARSER="toml-cli"
        return 0
    fi
    # Last resort: try to install rust via rustup
    echo "  ℹ️  cargo not found — installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    # shellcheck source=/dev/null
    source "${CARGO_HOME:-$HOME/.cargo}/env"
    cargo install toml-cli --quiet
    TOML_PARSER="toml-cli"
}

# ── Read TOML array from datum ────────────────────────────────────────────────
# Usage: _read_cmds "b00t.install.linux_debian.user_cmds"
# Returns: one command per line
_read_cmds() {
    local key="$1"
    if [[ "$TOML_PARSER" == "python" ]]; then
        python3 - "$DATUM" "$key" <<'PYEOF'
import sys, tomllib
datum_path, key = sys.argv[1], sys.argv[2]
with open(datum_path, "rb") as f:
    data = tomllib.load(f)
val = data
for k in key.split("."):
    val = val.get(k) if isinstance(val, dict) else None
    if val is None:
        break
if isinstance(val, list):
    for cmd in val:
        print(cmd)
elif isinstance(val, str) and val:
    print(val)
PYEOF
    elif [[ "$TOML_PARSER" == "toml-cli" ]]; then
        # toml get returns JSON array; parse with sed (avoids jq dep)
        local json; json=$(toml get "$DATUM" "$key" 2>/dev/null || echo "[]")
        # Strip JSON array brackets + quotes, one item per line
        echo "$json" | sed 's/^\[//;s/\]$//' | tr ',' '\n' \
            | sed 's/^[[:space:]]*"//;s/"[[:space:]]*$//' | grep -v '^$' || true
    fi
}

# ── Prompt subroutine (sudo-only steps) ───────────────────────────────────────
_prompt_timeout() {
    local desc="${1:-proceed}"
    local timeout=10
    for i in $(seq $timeout -1 1); do
        printf "\r  ⏱  [%2ds] %s — press any key or wait to skip..." "$i" "$desc"
        if read -r -s -n 1 -t 1 2>/dev/null; then
            printf "\n  ▶  %s\n" "$desc"
            return 0
        fi
    done
    printf "\n  ⏭  skipped: %s\n" "$desc"
    return 1
}

# ── Execute a list of commands ────────────────────────────────────────────────
_exec_cmds() {
    local label="$1"; shift
    local cmds=("$@")
    [[ ${#cmds[@]} -eq 0 ]] && return 0
    echo "  [$label]"
    for cmd in "${cmds[@]}"; do
        echo "    → $cmd"
        if [[ "$DRY_RUN" != "true" ]]; then
            eval "$cmd"
        fi
    done
}

# ── Main ──────────────────────────────────────────────────────────────────────
main() {
    if [[ ! -f "$DATUM" ]]; then
        echo "❌ datum not found: $DATUM" >&2
        echo "   Set B00T_DIR or run from the b00t repo root." >&2
        exit 1
    fi

    local OS_KEY; OS_KEY=$(_detect_os_key)
    _ensure_toml_parser

    if [[ "$LIST_ONLY" == "true" ]]; then
        echo "b00t-lite: available OS keys in $DATUM"
        _read_cmds "b00t.install" 2>/dev/null | grep -E '^\[' | sed 's/\[//;s/\]//' || \
            echo "  (use python3.11+ for key listing)"
        return 0
    fi

    echo "🥾 b00t-lite  os=${OS_KEY}  parser=${TOML_PARSER}  dry_run=${DRY_RUN}"
    echo "   datum: $DATUM"
    echo

    # Read OS-specific commands
    local base="b00t.install.${OS_KEY}"
    local fallback="b00t.install.linux_unknown"

    # sudo_cmds: apt-get / brew / pacman — prompt before each block
    local sudo_cmds=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && sudo_cmds+=("$line")
    done < <(_read_cmds "${base}.sudo_cmds" 2>/dev/null || _read_cmds "${fallback}.sudo_cmds" 2>/dev/null || true)

    if [[ ${#sudo_cmds[@]} -gt 0 ]]; then
        if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
            _prompt_timeout "run sudo_cmds for ${OS_KEY} (requires root)" && \
                _exec_cmds "sudo_cmds" "${sudo_cmds[@]}" || true
        else
            echo "  ⚠️  sudo_cmds skipped (not root)"
            echo "  💡 re-run as root: sudo B00T_DIR=${B00T_DIR} $0"
        fi
    fi

    # user_cmds: cargo/uv/pip — run unconditionally
    local user_cmds=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && user_cmds+=("$line")
    done < <(_read_cmds "${base}.user_cmds" 2>/dev/null || _read_cmds "${fallback}.user_cmds" 2>/dev/null || true)

    if [[ ${#user_cmds[@]} -gt 0 ]]; then
        _exec_cmds "user_cmds" "${user_cmds[@]}"
    fi

    # systemd_cmds: unit file install — prompt (sudo) or run directly (user)
    local systemd_cmds=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && systemd_cmds+=("$line")
    done < <(_read_cmds "${base}.systemd_cmds" 2>/dev/null || _read_cmds "b00t.install.systemd_cmds" 2>/dev/null || true)

    if [[ ${#systemd_cmds[@]} -gt 0 ]]; then
        if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
            _prompt_timeout "install systemd unit files (requires root)" && \
                _exec_cmds "systemd_cmds" "${systemd_cmds[@]}" || true
        else
            _exec_cmds "systemd_cmds (user)" "${systemd_cmds[@]}"
        fi
    fi

    echo
    echo "🥾 b00t-lite done"
    [[ "$DRY_RUN" == "true" ]] && echo "   (dry-run — no commands were executed)"
}

main "$@"
