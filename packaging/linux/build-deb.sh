#!/usr/bin/env bash
# Builds rustsale_<version>_amd64.deb from an `nsh package --target linux` output directory.
# Usage: build-deb.sh <nsh-output-dir> <out-dir>
# Uses dpkg-deb when present, otherwise assembles the archive with ar and tar (no Debian host needed).
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

SRC="${1:?usage: build-deb.sh <nsh-output-dir> <out-dir>}"
OUT="${2:?usage: build-deb.sh <nsh-output-dir> <out-dir>}"
VERSION="${PKG_VERSION:-$(app_version)}"
MAINTAINER="${MAINTAINER:-RustSale Packagers <packaging@rustsale.invalid>}"
ARCH=amd64

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ROOT="$WORK/root"
mkdir -p "$ROOT" "$OUT"
stage_tree "$SRC" "$ROOT"
assert_no_demo "$(find_runner "$SRC")"

GLIBC="$(min_glibc "$(find_runner "$SRC")")"
DEPENDS="libwebkitgtk-6.0-4, libgtk-4-1 (>= $GTK_MIN)"
[[ -n "$GLIBC" ]] && DEPENDS="libc6 (>= $GLIBC), $DEPENDS"

INSTALLED_KB="$(du -sk "$ROOT" | cut -f1)"
mkdir -p "$ROOT/DEBIAN"
cat > "$ROOT/DEBIAN/control" <<CONTROL
Package: $PKG_NAME
Version: $VERSION
Section: web
Priority: optional
Architecture: $ARCH
Depends: $DEPENDS
Installed-Size: $INSTALLED_KB
Maintainer: $MAINTAINER
Description: RustSale, sales CRM for customer conversations and outreach
 Native (GTK 4) desktop app. The runner links WebKitGTK 6.0 at load time,
 so libwebkitgtk-6.0-4 and libgtk-4-1 must be installed even though the
 app opens no web view.
CONTROL
(cd "$ROOT" && find . -path ./DEBIAN -prune -o -type f -print0 | sort -z | xargs -0 md5sum | sed 's# \./# #' > DEBIAN/md5sums)

DEB="$OUT/${PKG_NAME}_${VERSION}_${ARCH}.deb"
rm -f "$DEB"
if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --root-owner-group -Zgzip --build "$ROOT" "$DEB" >/dev/null
else
  OWN=(--owner=0 --group=0 --numeric-owner)
  (cd "$ROOT/DEBIAN" && tar "${OWN[@]}" -czf "$WORK/control.tar.gz" ./control ./md5sums)
  (cd "$ROOT" && tar "${OWN[@]}" --exclude=./DEBIAN -czf "$WORK/data.tar.gz" .)
  printf '2.0\n' > "$WORK/debian-binary"
  (cd "$WORK" && ar rc "$DEB" debian-binary control.tar.gz data.tar.gz)
fi
echo "$DEB"
