# RustSale Linux packages (GTK 4)

The Linux release is the GTK 4 host built by `nsh package --target linux` (decision of 22/11, issue #48).
That runner links `libwebkitgtk-6.0.so.4`, `libjavascriptcoregtk-6.0.so.1` and `libgtk-4.so.1` at load
time (DT_NEEDED, `docs/v3/perf.md` section 4b), so a missing library stops the binary before it starts. The
packages here declare those dependencies, which the plain nsh `.tar.gz` does not.

| Format | Files | Declares |
|---|---|---|
| `.deb` (Debian 13, Ubuntu 24.04) | `build-deb-container.sh` (runs `build-deb.sh` inside the distro) | `Depends` from `dpkg-shlibdeps`: `libc6 (>= 2.39)`, `libssl3t64 (>= 3.0.0)`, `libgtk-4-1`, `libwebkitgtk-6.0-4`, `libjavascriptcoregtk-6.0-1`, `libsoup-3.0-0`, GStreamer, pango, cairo, ... |
| Arch `PKGBUILD` | `PKGBUILD` + `build-tarball.sh` | `depends=(webkitgtk-6.0 gtk4 'openssl>=4' 'glibc>=2.44')` |
| `.rpm` (Fedora) | `rustsale.spec`, `build-rpm.sh` | `Requires: webkitgtk6.0, gtk4 >= 4.14, glibc >= 2.44, libssl.so.4, libcrypto.so.4` |

Install tree (the same in all three): `/opt/rustsale/` (runner and bundle), `/usr/bin/rustsale` (symlink),
`/usr/share/applications/com.rustsale.crm.v3.desktop`, `/usr/share/icons/hicolor/512x512/apps/rustsale.png`.

## Build

```sh
# one machine, heavy job (about 18 minutes cold): nsh package, then .deb + tarball (+ .rpm if rpmbuild exists)
CARGO_TARGET_DIR=... heavy-job <you> "RS: linux package" -- packaging/linux/build.sh [out-dir]

# or from an nsh output you already have:
packaging/linux/build-deb.sh     <nsh-output-dir> <out-dir>
packaging/linux/build-tarball.sh <nsh-output-dir> <out-dir>   # then copy PKGBUILD next to it and run makepkg
packaging/linux/build-rpm.sh     <nsh-output-dir> <out-dir>   # needs rpmbuild (or a Fedora container)
```

Without `dpkg-deb` (Arch, for example) `build-deb.sh` assembles the archive with `ar` and `tar`.
`build-*.sh` refuse a runner that carries `Demo.seed`/`Demo.prepare`, and `build.sh` refuses `RUSTSALE_DEMO=1`.

## Where each package runs (checked 24/11/2026)

The runner is linked against the libraries of the machine that built it (here Arch: glibc 2.44, OpenSSL 4).
`build-deb.sh` writes that floor into `Depends` (`libc6 (>= 2.44)`) and warns about `libssl.so.4`.

| Target | Result |
|---|---|
| Arch Linux (clean container, `pacman -U`) | pulls `webkitgtk-6.0`, `gtk4`, `openssl`; `ldd` has no `not found`; the app starts and keeps running under Xvfb |
| Fedora 44 (glibc 2.43, OpenSSL 3, clean container) | `dnf install ./rustsale-*.rpm` is refused (`glibc >= 2.44`, `libssl.so.4`) |
| Fedora 45 (clean container, `dnf install ./rustsale-*.rpm`) | pulls `webkitgtk6.0` and `gtk4`; `ldd` has no `not found` (Fedora 45 ships OpenSSL 4) |
| Ubuntu 24.04 (glibc 2.39, OpenSSL 3) | the Arch-built `.deb` is refused (`libc6 (>= 2.44)`, and the binary needs `libssl.so.4`). The `.deb` built in `ubuntu:24.04` (`build-deb-container.sh`) installs in a clean `ubuntu:24.04`, `ldd` has no `not found`, and the app keeps running for 15 s under Xvfb |
| Debian 13 (glibc 2.41, OpenSSL 3.5) | the `.deb` built in `debian:13` installs in a clean `debian:13`, `ldd` has no `not found`, the app keeps running for 15 s under Xvfb |

A runner is only installable on a distro at least as new as the machine that built it, so the `.deb` is built inside the target distro
(issue #57):

```sh
# one distro at a time, about 20-30 minutes cold; the cargo cache lives in a podman volume per distro
HEAVY_JOB_WAIT=5400 heavy-job <you> "RS: deb debian13" -- packaging/linux/build-deb-container.sh debian:13 [out-dir]
HEAVY_JOB_WAIT=5400 heavy-job <you> "RS: deb ubuntu2404" -- packaging/linux/build-deb-container.sh ubuntu:24.04 [out-dir]
```

The script builds `container/Containerfile.deb` (the distro's `libwebkitgtk-6.0-dev`, `libgtk-4-dev`, `libssl-dev`, GStreamer dev
packages, Rust via rustup), copies the committed tree and the host `nsh` home in, runs `nsh package --target linux`, then
`build-deb.sh`. With `dpkg-shlibdeps` present `build-deb.sh` takes `Depends` from the distro (so `libssl3t64`, not a hand-written
OpenSSL name); without it (Arch) it keeps the hand-written list. Commit first: only `HEAD` is built, not uncommitted edits.

## Build-time settings (not in the repo)

The release reads two values when it is built, never at run time:

| Variable | What |
|---|---|
| `VITE_SUPABASE_URL` | Supabase project URL (`https://<ref>.supabase.co`) |
| `VITE_SUPABASE_ANON_KEY` | the project's public anon key |

Set them in the shell that runs the build, or put them in a `.env` at the repo root (git-ignored; start from
`.env.example`). The environment wins over `.env`. `native/build.rs` hands them to the crate, and
`nsh_app_rustsale::config::supabase()` returns them, or `None` when either is missing, empty, `http://`, or still the
placeholder from `.env.example`. A build without them is a dev build: sync stays off and no default project is used.
In CI, set them from repository secrets (`.github/workflows/release.yml` already writes the `.env`).
Do not commit real values; the anon key is public by design but still belongs to the release configuration, not the source.

## Check an installed package

```sh
dpkg -s rustsale | grep Depends           # or: pacman -Qi rustsale | grep Depends
readelf -d /opt/rustsale/RustSale | grep NEEDED
ldd /opt/rustsale/RustSale | grep 'not found'    # must print nothing
```

`libwebkitgtk-6.0.so.4`, `libjavascriptcoregtk-6.0.so.1` and `libgtk-4.so.1` in `NEEDED` correspond to the declared
packages. When upstream nsh can build a host without the `webview` feature, the WebKit dependency can go.
