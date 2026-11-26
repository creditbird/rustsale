# Vendored `whatsapp-rust-sqlite-storage` 0.7.0 Patch

Minimal vendored copy of `whatsapp-rust-sqlite-storage` `0.7.0` from crates.io,
wired via `[patch.crates-io]` to resolve `RUSTSEC-2026-0459` (bundled SQLite < 3.54.0).

## Changes from upstream `0.7.0`

Only 3 dependency version lines in `Cargo.toml` are changed, matching upstream
`oxidezap/whatsapp-rust` commit `cd2d885` (`cd2d885891851405648c109c8db9ed78b7ca93c4`, PR #1894):

- `diesel`: `2.3.11` -> `2.4.0`
- `diesel_migrations`: `2.3.2` -> `2.4.0`
- `libsqlite3-sys`: `0.37` -> `0.40.0` (bundles SQLite 3.54.1)

All Rust sources, migrations, proto files, and build scripts are kept unmodified
byte-for-byte from `whatsapp-rust-sqlite-storage 0.7.0`. The original MIT `LICENSE`
is preserved alongside this file.

## Why `.cargo/config.toml` is also used

`nsh 0.6.1` generates its own host Cargo workspaces under `.nsh/rust/Cargo.toml`
without forwarding `[patch.crates-io]` from `native/Cargo.toml`. Placing
`[patch.crates-io]` in `.cargo/config.toml` at the repository root ensures Cargo
applies the patch consistently across the root workspace, `native/`, and all
`nsh`-generated workspaces.

## Removal

Delete this directory, `.cargo/config.toml`, and the `[patch.crates-io]` entries
in root `Cargo.toml` and `native/Cargo.toml` once `whatsapp-rust` publishes a
release `> 0.7.0` to crates.io.
