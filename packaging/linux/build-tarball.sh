#!/usr/bin/env bash
# Makes the tarball the PKGBUILD installs from: the staged install tree under root/.
# Usage: build-tarball.sh <nsh-output-dir> <out-dir>   (writes <out-dir>/rustsale-<version>-linux-x86_64.tar.gz)
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

SRC="${1:?usage: build-tarball.sh <nsh-output-dir> <out-dir>}"
OUT="${2:?usage: build-tarball.sh <nsh-output-dir> <out-dir>}"
VERSION="${PKG_VERSION:-$(app_version)}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/root" "$OUT"
stage_tree "$SRC" "$WORK/root"
assert_no_demo "$(find_runner "$SRC")"
TAR="$OUT/${PKG_NAME}-${VERSION}-linux-x86_64.tar.gz"
tar --owner=0 --group=0 --numeric-owner -czf "$TAR" -C "$WORK" root
echo "$TAR"
