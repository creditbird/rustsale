#!/usr/bin/env bash
# Repeatable Linux .deb build for RustSale v3 (issue #59, release gate for #49/#57).
# Wraps packaging/linux/build-deb-container.sh: builds the committed HEAD inside a Debian 13 or Ubuntu 24.04
# container with `nsh package --target linux`, then reports the package (path, size, sha256, Depends).
#
# Usage:   scripts/package-linux.sh [debian:13|ubuntu:24.04] [out-dir]
#          (default distro debian:13, default out-dir <repo>/dist/linux-<distro>)
# Heavy:   run it through the shared queue, in the foreground, 20-30 min cold:
#          HEAVY_JOB_WAIT=5400 heavy-job <you> "RS: linux deb" -- scripts/package-linux.sh debian:13
# Needs:   podman or docker, `nsh` on PATH, committed tree (only HEAD is built).
# Settings: VITE_SUPABASE_URL / VITE_SUPABASE_ANON_KEY from the environment or .env (packaging/linux/README.md).
#          Without them the package is a dev build: sync stays off. Never set RUSTSALE_DEMO.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
BASE="${1:-debian:13}"
TAG="$(printf '%s' "$BASE" | tr ':/' '--')"
OUT="${2:-$REPO/dist/linux-$TAG}"

case "$BASE" in debian:13|ubuntu:24.04) ;; *) echo "distro must be debian:13 or ubuntu:24.04 (got $BASE)" >&2; exit 64;; esac
[[ "${RUSTSALE_DEMO:-}" == "1" ]] && { echo "unset RUSTSALE_DEMO: a release must not carry the demo exports" >&2; exit 1; }
command -v nsh >/dev/null || { echo "nsh not on PATH" >&2; exit 1; }

# Only HEAD is built (git archive), so refuse uncommitted edits to avoid a package that does not match its commit.
if [[ -n "$(git -C "$REPO" status --porcelain --untracked-files=no)" ]]; then
  echo "working tree has uncommitted changes; commit first (only HEAD is built)" >&2; exit 1
fi
COMMIT="$(git -C "$REPO" rev-parse --short HEAD)"
BRANCH="$(git -C "$REPO" rev-parse --abbrev-ref HEAD)"
echo "building $BRANCH @ $COMMIT in $BASE -> $OUT"
if [[ -z "${VITE_SUPABASE_URL:-}" && ! -f "$REPO/.env" ]]; then
  echo "warning: no VITE_SUPABASE_* settings: this is a dev build (Supabase sync off)" >&2
fi

df -h / | tail -n1
"$HERE/../packaging/linux/build-deb-container.sh" "$BASE" "$OUT"

DEB="$(find "$OUT" -maxdepth 1 -name 'rustsale_*_amd64.deb' | sort | tail -n1)"
[[ -n "$DEB" ]] || { echo "no .deb produced in $OUT" >&2; exit 1; }
echo "---- package"
echo "path:   $DEB"
echo "size:   $(stat -c %s "$DEB") bytes ($(du -h "$DEB" | cut -f1))"
echo "sha256: $(sha256sum "$DEB" | cut -d' ' -f1)"
echo "commit: $BRANCH @ $COMMIT, base $BASE"
# Depends and demo-export check straight from the archive (works without dpkg: ar + tar).
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
(cd "$TMP" && ar x "$DEB" && tar -xf control.tar.* ./control 2>/dev/null && grep -E '^(Version|Depends):' control)
(cd "$TMP" && mkdir data && tar -xf data.tar.* -C data ./opt/rustsale 2>/dev/null
 RUNNER="$(find data/opt/rustsale -maxdepth 1 -type f -iname rustsale | head -n1)"
 if grep -aq 'Demo\.seed\|Demo\.prepare' "$RUNNER"; then echo "FAIL: runner carries Demo exports" >&2; exit 1; fi
 echo "demo exports: none")
df -h / | tail -n1
