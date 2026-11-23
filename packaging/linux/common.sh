#!/usr/bin/env bash
# Shared by the package scripts: finds the runner in an `nsh package --target linux` output directory and
# stages the install tree (/opt/rustsale, /usr/bin/rustsale, .desktop, icon).
# shellcheck shell=bash
set -euo pipefail

PKG_NAME=rustsale
# The GTK4 / WebKitGTK 6.0 floor the runner links against (docs/v3/perf.md section 4b).
GTK_MIN=4.14

here() { cd "$(dirname "${BASH_SOURCE[0]}")" && pwd; }
HERE="$(here)"
REPO="$(cd "$HERE/../.." && pwd)"

# The app version, from app/nsh.json (single source of truth).
app_version() {
  sed -n 's/^[[:space:]]*"version":[[:space:]]*"\([^"]*\)".*/\1/p' "$REPO/app/nsh.json" | head -n1
}

# The runner binary inside the nsh output directory: the one ELF file named like the app.
find_runner() {
  local dir="$1" f
  for f in "$dir"/RustSale "$dir"/rustsale; do
    if [[ -f "$f" && -x "$f" ]]; then printf '%s\n' "$f"; return 0; fi
  done
  echo "no RustSale runner in $dir (expected the output of: nsh package --target linux)" >&2
  return 1
}

# Copies the nsh output into $2/opt/rustsale and adds the launcher, desktop file and icon.
stage_tree() {
  local src="$1" root="$2" runner
  runner="$(find_runner "$src")"
  install -d "$root/opt/rustsale" "$root/usr/bin" "$root/usr/share/applications" \
    "$root/usr/share/icons/hicolor/512x512/apps"
  cp -a "$src"/. "$root/opt/rustsale/"
  # The nsh-made .desktop is replaced by ours (it names a path that only exists in the build folder).
  find "$root/opt/rustsale" -maxdepth 1 -name '*.desktop' -delete
  ln -s "/opt/rustsale/$(basename "$runner")" "$root/usr/bin/rustsale"
  install -m 0644 "$HERE/rustsale.desktop" "$root/usr/share/applications/com.rustsale.crm.v3.desktop"
  install -m 0644 "$HERE/rustsale.png" "$root/usr/share/icons/hicolor/512x512/apps/rustsale.png"
  chmod -R u+rwX,go+rX,go-w "$root"
}

# The newest glibc symbol version the runner needs, e.g. 2.39 (empty when it cannot be read).
min_glibc() {
  readelf -V "$1" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -n1
}

# Refuses a runner that carries the demo exports: a release has no Demo.seed.
assert_no_demo() {
  if grep -aq 'Demo\.seed\|Demo\.prepare' "$1"; then
    echo "refusing to package: the runner contains Demo.seed / Demo.prepare (built with RUSTSALE_DEMO or the demo feature)" >&2
    return 1
  fi
}
