#!/usr/bin/env bash
# 🤓 Tauri v2 dependency detection — reports what's installed and what's missing
#    WITHOUT modifying /etc or running sudo. Read-only system inspection.
#    Source: DietrichGebert/ponytail rung 3 (stdlib: pkg-config)
#
# Usage: ./scripts/check-tauri-deps.sh
#        ./scripts/check-tauri-deps.sh --json   (machine-readable output)
#        ./scripts/check-tauri-deps.sh --install (print apt install command)
#        ./scripts/check-tauri-deps.sh --verify  (verify after install)

# 🤓 No set -e: detection scripts must survive missing packages gracefully
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

MODE="${1:-}"

# ── Tauri v2 required packages (Ubuntu 22.04+ names) ────────────────────────

declare -A DEPS
DEPS=(
  ["libwebkit2gtk-4.1-dev"]="Tauri WebView (critical)"
  ["libgtk-3-dev"]="GTK3 bindings for gdk-sys"
  ["libgdk-pixbuf-2.0-dev"]="Image buffer support"
  ["libpango1.0-dev"]="Text layout"
  ["libatk1.0-dev"]="Accessibility toolkit"
  ["libsoup-3.0-dev"]="HTTP networking (Tauri v2)"
  ["libjavascriptcoregtk-4.1-dev"]="JS engine for WebKit"
  ["libxdo-dev"]="X11 automation"
  ["libayatana-appindicator3-dev"]="Tray indicator"
  ["librsvg2-dev"]="SVG rendering"
  ["build-essential"]="Compilers and make"
)

# ── pkg-config check names (some differ from apt package names) ─────────────

declare -A PC_CHECKS
PC_CHECKS=(
  ["webkit2gtk-4.1"]="libwebkit2gtk-4.1-dev"
  ["gtk+-3.0"]="libgtk-3-dev"
  ["gdk-pixbuf-2.0"]="libgdk-pixbuf-2.0-dev"
  ["pango"]="libpango1.0-dev"
  ["atk"]="libatk1.0-dev"
  ["libsoup-3.0"]="libsoup-3.0-dev"
  ["javascriptcoregtk-4.1"]="libjavascriptcoregtk-4.1-dev"
  ["xdo"]="libxdo-dev"
  ["ayatana-appindicator3-0.1"]="libayatana-appindicator3-dev"
  ["librsvg-2.0"]="librsvg2-dev"
)

# ── Detection ────────────────────────────────────────────────────────────────

detect() {
  local os_id="" os_version="" arch=""
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    os_id="$ID"
    os_version="$VERSION_ID"
  fi
  arch="$(uname -m)"

  echo "🔍 Tauri v2 Build Dependency Detector"
  echo "   OS:       ${os_id} ${os_version} (${arch})"
  echo "   WSL:      $(grep -qi microsoft /proc/version 2>/dev/null && echo 'yes' || echo 'no')"
  echo "   DISPLAY:  ${DISPLAY:-unset}"
  echo "   Rust:     $(rustc --version 2>/dev/null || echo 'not installed')"
  echo "   Cargo:    $(cargo --version 2>/dev/null || echo 'not installed')"
  echo ""

  local pkg_found=0 pkg_missing=0
  local -a missing_pkgs=()
  local results=""

  for pkg in "${!DEPS[@]}"; do
    local desc="${DEPS[$pkg]}"
    local pc_name=""
    local found=false

    # Find matching pkg-config name
    for pc in "${!PC_CHECKS[@]}"; do
      if [ "${PC_CHECKS[$pc]}" = "$pkg" ]; then
        pc_name="$pc"
        break
      fi
    done

    # Check via pkg-config if we have a name for it
    if [ -n "$pc_name" ] && command -v pkg-config &>/dev/null; then
      if pkg-config --exists "$pc_name" 2>/dev/null; then
        found=true
      fi
    fi

    # Fallback: check dpkg directly
    if ! $found && command -v dpkg &>/dev/null; then
      if dpkg -s "$pkg" 2>/dev/null | grep -q 'Status: install ok installed'; then
        found=true
      fi
    fi

    # Fallback: check if .pc file exists directly
    if ! $found && [ -n "$pc_name" ]; then
      for dir in /usr/lib/pkgconfig /usr/lib/*/pkgconfig /usr/share/pkgconfig; do
        if [ -f "$dir/${pc_name}.pc" ]; then
          found=true
          break
        fi
      done
    fi

    if $found; then
      echo -e "  ${GREEN}✅${NC} $pkg  ($desc)"
      ((pkg_found++))
    else
      echo -e "  ${RED}❌${NC} $pkg  ($desc)"
      ((pkg_missing++))
      missing_pkgs+=("$pkg")
    fi
  done

  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo -e "  ${GREEN}${pkg_found} found${NC} · ${RED}${pkg_missing} missing${NC} · $((pkg_found + pkg_missing)) total"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

  # Return structured data
  if [ "$MODE" = "--json" ]; then
    echo "{"
    echo "  \"os\": \"${os_id} ${os_version}\","
    echo "  \"arch\": \"${arch}\","
    echo "  \"is_wsl\": $(grep -qi microsoft /proc/version 2>/dev/null && echo 'true' || echo 'false'),"
    echo "  \"pkg_found\": ${pkg_found},"
    echo "  \"pkg_missing\": ${pkg_missing},"
    echo "  \"missing_pkgs\": ["
    local i=0
    for p in "${missing_pkgs[@]}"; do
      if [ $i -gt 0 ]; then echo ","; fi
      echo -n "    \"${p}\""
      ((i++))
    done
    echo ""
    echo "  ]"
    echo "}"
  fi

  # Return exit code based on all critical deps
  local critical_missing=false
  if ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
    critical_missing=true
  fi

  if $critical_missing; then
    return 2  # critical missing
  elif [ $pkg_missing -gt 0 ]; then
    return 1  # non-critical missing
  else
    return 0  # all good
  fi
}

# ── Print install command ────────────────────────────────────────────────────

print_install_cmd() {
  echo "# To install all missing Tauri v2 dependencies, run:"
  echo ""
  echo "sudo apt-get update && sudo apt-get install -y \\"
  local i=0
  for pkg in "${!DEPS[@]}"; do
    local found=false
    local pc_name=""
    for pc in "${!PC_CHECKS[@]}"; do
      if [ "${PC_CHECKS[$pc]}" = "$pkg" ]; then
        pc_name="$pc"
        break
      fi
    done
    if [ -n "$pc_name" ] && pkg-config --exists "$pc_name" 2>/dev/null; then
      found=true
    fi
    if ! $found && dpkg -s "$pkg" 2>/dev/null | grep -q 'Status: install ok installed'; then
      found=true
    fi
    if ! $found; then
      if [ $i -gt 0 ]; then echo " \\"; fi
      echo -n "  $pkg"
      ((i++))
    fi
  done
  echo ""
}

# ── Verify after install ─────────────────────────────────────────────────────

verify_after_install() {
  echo "🔍 Verifying Tauri v2 deps after install..."
  echo ""
  local errors=0
  for pc in "${!PC_CHECKS[@]}"; do
    if pkg-config --exists "$pc" 2>/dev/null; then
      local ver
      ver=$(pkg-config --modversion "$pc" 2>/dev/null)
      echo -e "  ${GREEN}✅${NC} $pc  (v${ver})"
    else
      echo -e "  ${RED}❌${NC} $pc  (NOT FOUND)"
      ((errors++))
    fi
  done
  echo ""
  if [ $errors -eq 0 ]; then
    echo -e "${GREEN}✔ All Tauri v2 dependencies satisfied.${NC}"
    echo "  Run: cd vendor/ledgrrr/crates/ledgerr-tauri && cargo check"
  else
    echo -e "${RED}✖ ${errors} dependencies still missing.${NC}"
    return 1
  fi
}

# ── Main ─────────────────────────────────────────────────────────────────────

case "$MODE" in
  --json)
    detect
    ;;
  --install)
    print_install_cmd
    ;;
  --verify)
    verify_after_install
    ;;
  "")
    detect
    echo ""
    print_install_cmd
    echo ""
    echo "# After installing, run:  ./scripts/check-tauri-deps.sh --verify"
    ;;
  *)
    echo "Usage: $0 [--json | --install | --verify]"
    exit 1
    ;;
esac
