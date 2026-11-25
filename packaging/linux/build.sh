#!/usr/bin/env bash
# Builds the Linux release packages: nsh package --target linux, then .deb, .tar.gz (for the PKGBUILD) and,
# when rpmbuild exists, .rpm.
# Usage: build.sh [out-dir]            (default: <repo>/dist/linux, git-ignored)
# Build settings (not committed, see README.md): VITE_SUPABASE_URL, VITE_SUPABASE_ANON_KEY from the
# environment or an uncommitted .env at the repo root. Never set RUSTSALE_DEMO here: a release has no Demo.seed.
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

OUT="${1:-$REPO/dist/linux}"
if [[ "${RUSTSALE_DEMO:-}" == "1" ]]; then
  echo "RUSTSALE_DEMO=1 is set: a release must not carry the demo exports; unset it" >&2
  exit 1
fi
mkdir -p "$OUT"
NSH_OUT="$OUT/nsh"
rm -rf "$NSH_OUT"
(cd "$REPO/app" && nsh package --target linux -o "$NSH_OUT")

# nsh writes the app directory (runner + bundle + .desktop) next to a .tar.gz of it.
APP_DIR="$(find "$NSH_OUT" -mindepth 1 -maxdepth 1 -type d | head -n1)"
[[ -n "$APP_DIR" ]] || { echo "nsh package produced no app directory in $NSH_OUT" >&2; exit 1; }

"$HERE/build-deb.sh" "$APP_DIR" "$OUT"
"$HERE/build-tarball.sh" "$APP_DIR" "$OUT"
if command -v rpmbuild >/dev/null 2>&1; then
  "$HERE/build-rpm.sh" "$APP_DIR" "$OUT"
else
  echo "skip .rpm: rpmbuild not found" >&2
fi
echo "done: $OUT"
