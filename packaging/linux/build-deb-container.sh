#!/usr/bin/env bash
# Builds rustsale_<version>_amd64.deb inside a Debian 13 or Ubuntu 24.04 container, so the runner links that
# distro's glibc and OpenSSL and the package declares that distro's own dependencies (issue #57).
# Usage: build-deb-container.sh <debian:13|ubuntu:24.04> [out-dir]      (default out-dir: <repo>/dist/linux-<tag>)
# Needs podman or docker, a host `nsh` (its install dir is copied in; the binary needs glibc >= 2.39 only),
# and the build-time settings of README.md (VITE_SUPABASE_URL, VITE_SUPABASE_ANON_KEY) in the environment or .env.
# Never set RUSTSALE_DEMO: a release has no Demo.seed.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
BASE="${1:?usage: build-deb-container.sh <debian:13|ubuntu:24.04> [out-dir]}"
TAG="$(printf '%s' "$BASE" | tr ':/' '--')"
OUT="${2:-$REPO/dist/linux-$TAG}"
CTR="${CONTAINER_CLI:-$(command -v podman || command -v docker)}"
NSH_HOME="${NSH_HOME:-$(dirname "$(dirname "$(readlink -f "$(command -v nsh)")")")}"
[[ "${RUSTSALE_DEMO:-}" == "1" ]] && { echo "unset RUSTSALE_DEMO: a release must not carry the demo exports" >&2; exit 1; }
[[ -x "$NSH_HOME/bin/nsh" ]] || { echo "no nsh at $NSH_HOME/bin/nsh (set NSH_HOME)" >&2; exit 1; }

IMG="rustsale-deb-build:$TAG"
"$CTR" build -q --build-arg "BASE=$BASE" -t "$IMG" -f "$HERE/container/Containerfile.deb" "$HERE/container"

# A clean copy of the committed files (HEAD; the container writes native/.nsh and target/ into it), a copy of nsh's home,
# and a cache volume for cargo's registry and the build output (one per distro, never shared between distros).
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/src" "$OUT"
git -C "$REPO" archive HEAD | tar -C "$WORK/src" -xf -
[[ -f "$REPO/.env" ]] && cp "$REPO/.env" "$WORK/src/.env"
cp -a "$NSH_HOME" "$WORK/nsh"
VOL="rustsale-deb-cache-$TAG"
"$CTR" volume exists "$VOL" 2>/dev/null || "$CTR" volume create "$VOL" >/dev/null

"$CTR" run --rm -v "$WORK/src:/src" -v "$WORK/nsh:/root/.nsh" -v "$VOL:/cache" -v "$(cd "$OUT" && pwd):/out" \
  -e CARGO_TARGET_DIR=/cache/target -e CARGO_HOME=/cache/cargo -e RUSTUP_HOME=/opt/rustup \
  -e VITE_SUPABASE_URL -e VITE_SUPABASE_ANON_KEY -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
  -w /src "$IMG" bash -euo pipefail -c '
    export PATH=/root/.nsh/bin:/opt/cargo/bin:$PATH
    rm -rf /out/nsh && (cd app && nsh package --target linux -o /out/nsh)
    APP_DIR="$(find /out/nsh -mindepth 1 -maxdepth 1 -type d | head -n1)"
    packaging/linux/build-deb.sh "$APP_DIR" /out
    chmod -R a+rX /out'
echo "done: $OUT"
