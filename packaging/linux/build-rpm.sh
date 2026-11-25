#!/usr/bin/env bash
# Builds rustsale-<version>-1.x86_64.rpm from an `nsh package --target linux` output directory. Needs rpmbuild.
# Usage: build-rpm.sh <nsh-output-dir> <out-dir>
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

SRC="${1:?usage: build-rpm.sh <nsh-output-dir> <out-dir>}"
OUT="${2:?usage: build-rpm.sh <nsh-output-dir> <out-dir>}"
VERSION="${PKG_VERSION:-$(app_version)}"
command -v rpmbuild >/dev/null 2>&1 || { echo "rpmbuild not found (install rpm-build, or run this in a Fedora container)" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/root" "$WORK/rpmbuild/SOURCES" "$OUT"
stage_tree "$SRC" "$WORK/root"
assert_no_demo "$(find_runner "$SRC")"
tar --owner=0 --group=0 --numeric-owner -czf "$WORK/rpmbuild/SOURCES/rustsale-root.tar.gz" -C "$WORK/root" .
rpmbuild -bb --define "_topdir $WORK/rpmbuild" --define "pkg_version $VERSION" "$HERE/rustsale.spec" >/dev/null
cp "$WORK"/rpmbuild/RPMS/*/*.rpm "$OUT/"
ls "$OUT"/*.rpm
