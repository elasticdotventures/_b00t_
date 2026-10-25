#!/usr/bin/env bash
# Prune stale Cargo build artifacts (target/ directories) with cargo-sweep, run inside a container.
#
#   scripts/cargo-sweep/sweep.sh                 dry run: report what --days 14 would delete (default)
#   scripts/cargo-sweep/sweep.sh --days 30       dry run with a different age threshold
#   scripts/cargo-sweep/sweep.sh --maxsize 20GB  dry run: trim oldest artifacts until each target/ <= 20GB
#   scripts/cargo-sweep/sweep.sh --apply         actually delete (repo mounted read-write)
#   --root DIR   sweep DIR instead of the repository root
#
# Age is judged from file access/modification times, so it works on target/ dirs built by any toolchain
# or OS (including Windows-built ones). It deliberately does not offer cargo-sweep's --installed mode:
# inside the container that compares against the container's Rust toolchain, not the host's.
# Needs podman or docker (override with CONTAINER_ENGINE=...). Dry runs mount the repo read-only.
set -euo pipefail

sweep_version="0.8.0"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
days=14
maxsize=""
apply=0

die() { echo "sweep.sh: $*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --days) [ $# -ge 2 ] || die "--days needs a value"; days="$2"; shift ;;
    --maxsize) [ $# -ge 2 ] || die "--maxsize needs a value"; maxsize="$2"; shift ;;
    --root) [ $# -ge 2 ] || die "--root needs a value"; root="$(cd "$2" && pwd)"; shift ;;
    --apply) apply=1 ;;
    -h | --help) sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1 (try --help)" ;;
  esac
  shift
done
case "$days" in '' | *[!0-9]*) die "--days must be a whole number" ;; esac

engine="${CONTAINER_ENGINE:-$(command -v podman || command -v docker || true)}"
[ -n "$engine" ] || die "neither podman nor docker found (set CONTAINER_ENGINE)"

# A Windows-side engine (podman.exe, or any engine under Git Bash) needs Windows-style paths.
hostpath() {
  case "$engine" in
    *.exe) if command -v wslpath >/dev/null 2>&1; then wslpath -w "$1"; else cygpath -w "$1"; fi ;;
    *) if [ "${OSTYPE:-}" = msys ] && command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi ;;
  esac
}

image="localhost/cargo-sweep:${sweep_version}"
if ! "$engine" image inspect "$image" >/dev/null 2>&1; then
  echo "building $image (one-time) ..."
  "$engine" build -t "$image" --build-arg "SWEEP_VERSION=${sweep_version}" "$(hostpath "$here")"
fi

mode=(--time "$days")
[ -z "$maxsize" ] || mode=(--maxsize "$maxsize")
dry=(--dry-run)
mount=ro
if [ "$apply" = 1 ]; then dry=(); mount=rw; fi

echo "cargo-sweep ${sweep_version} on ${root}: ${mode[*]} $([ "$apply" = 1 ] && echo '(APPLY: deleting)' || echo '(dry run; pass --apply to delete)')"
out="$(mktemp)"
trap 'rm -f "$out"' EXIT
rc=0
"$engine" run --rm --security-opt label=disable \
  -v "$(hostpath "$root"):/work:${mount}" "$image" \
  --recursive "${mode[@]}" ${dry[@]+"${dry[@]}"} /work 2>&1 | tee "$out" || rc="${PIPESTATUS[0]}"
# cargo-sweep silently skips a workspace that `cargo metadata` cannot resolve (for example when a path
# dependency or git submodule is not checked out). Say so instead of leaving an empty result.
if ! grep -qE 'Would clean|Cleaned' "$out"; then
  echo "sweep.sh: note: cargo-sweep reported no target directories under ${root}." >&2
  echo "          If you expect one, check that the workspace resolves (e.g. 'git submodule update --init')." >&2
fi
exit "$rc"
