#!/usr/bin/env bash
# b00tyverse vendor: podman static .deb builder
#
# Produces a signed, checksum-verified .deb from the official static binary.
# Same input + same key = identical artifact across machines.
#
# Usage:
#   ./build-deb.sh                    # build latest version
#   ./build-deb.sh 6.0.1             # build specific version
#   B00T_GPG_KEY=0xDEADBEEF ./build-deb.sh 6.0.1  # sign with specific key
#
# Output: podman-static_6.0.1_amd64.deb + .deb.sha256 + .deb.asc
set -euo pipefail

VERSION="${1:-6.0.1}"
TAG="v${VERSION}"
REPO="containers/podman"
ASSET="podman-remote-static-linux_amd64.tar.gz"
WORKDIR="$(mktemp -d)"
OUTDIR="${PWD}"
GPG_KEY="${B00T_GPG_KEY:-}"

cleanup() { rm -rf "$WORKDIR"; }
trap cleanup EXIT

echo "==> b00tyverse podman .deb builder — v${VERSION}"

# ── 1. Download release + checksums ─────────────────────────────
echo "[1/5] Downloading ${TAG}..."
gh release download "$TAG" \
  --repo "$REPO" \
  --pattern "$ASSET" \
  --pattern "shasums" \
  --dir "$WORKDIR" \
  --clobber

# ── 2. Verify SHA256 checksum ───────────────────────────────────
echo "[2/5] Verifying checksum..."
cd "$WORKDIR"
# GitHub publishes shasums as shasums.txt or shasums — find whichever
if [ -f shasums.txt ]; then
  SHASUM_FILE="shasums.txt"
elif [ -f shasums ]; then
  SHASUM_FILE="shasums"
else
  echo "ERROR: no shasums file found in release"
  ls -la "$WORKDIR"
  exit 1
fi
grep "$ASSET" "$SHASUM_FILE" | sha256sum -c --strict || {
  echo "ERROR: SHA256 mismatch — artifact may be tampered"
  exit 1
}
echo "  checksum verified"

# ── 3. Extract binary ────────────────────────────────────────────
echo "[3/5] Extracting binary..."
tar xzf "$WORKDIR/$ASSET" -C "$WORKDIR"
BIN=$(find "$WORKDIR" -type f -name 'podman*' ! -name '*.tar.gz' | head -1)
if [ ! -x "$BIN" ]; then
  echo "ERROR: no podman binary found in tarball"
  exit 1
fi
BIN_HASH=$(sha256sum "$BIN" | awk '{print $1}')
echo "  binary: $(basename "$BIN") (sha256: ${BIN_HASH:0:16}...)"

# ── 4. Build .deb ─────────────────────────────────────────────────
echo "[4/5] Building .deb..."
DEB_ROOT="$WORKDIR/deb-root"
mkdir -p "$DEB_ROOT/usr/local/bin"
mkdir -p "$DEB_ROOT/DEBIAN"

install -m755 "$BIN" "$DEB_ROOT/usr/local/bin/podman"
install -m755 "$BIN" "$DEB_ROOT/usr/local/bin/podman-remote"

cat > "$DEB_ROOT/DEBIAN/control" <<CONTROL
Package: podman-static
Version: ${VERSION}
Architecture: amd64
Maintainer: b00tyverse <b00t@promptexecution.com>
Description: Podman ${VERSION} — static binary (containers/podman)
 Rootless container engine with pod management and kube play.
 Built from official GitHub release, SHA256-verified.
Homepage: https://github.com/containers/podman
CONTROL

DEB_FILE="${OUTDIR}/podman-static_${VERSION}_amd64.deb"
dpkg-deb --build "$DEB_ROOT" "$DEB_FILE"
DEB_HASH=$(sha256sum "$DEB_FILE" | awk '{print $1}')

# ── 5. Checksums + signature ──────────────────────────────────────
echo "[5/5] Generating checksums + signature..."
echo "${DEB_HASH}  podman-static_${VERSION}_amd64.deb" > "${DEB_FILE}.sha256"

if [ -n "$GPG_KEY" ]; then
  gpg --detach-sign --local-user "$GPG_KEY" --armor "$DEB_FILE"
  echo "  signed with ${GPG_KEY} → ${DEB_FILE}.asc"
else
  echo "  ⚠ B00T_GPG_KEY not set — .deb not signed"
  echo "  Set B00T_GPG_KEY=0xYOURKEYID to GPG-sign the package"
fi

echo ""
echo "==> Done: ${DEB_FILE}"
echo "    sha256: ${DEB_HASH}"
ls -lh "${DEB_FILE}"*
