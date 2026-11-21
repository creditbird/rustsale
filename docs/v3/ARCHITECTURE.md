# RS: RustSale v3 — NSH rewrite plan

Owner decision 2026-11-22: rewrite RustSale on NSH, keeping every v2 feature, with the
smoothest production-grade UI/UX. v2 (`RustSale_v2`, Tauri 2 + SvelteKit) keeps running in
production until v3 passes full QA and the cutover. Tech lead: Nakroth.

## Starting point

- This repo is a clone of v2 at production commit `2535767` (Zalo webhook allowlist).
  Old v3 (NSH 0.4 spike, 10/11) is archived at
  `~/backups/rustsale-v3-old-20261122022121` (source tar, git bundle, nsh skills).
- NSH skills: `.agents/skills/nsh`, `.agents/skills/nsh-design-guides`. Read them first.

## Target architecture

NSH **Rust mode** (`nsh.json` with `"rust"` + `"root"`), no QuickJS, no WebView.
On Linux the native GTK 4 backend is the default; GPUI stays an optional feature, not the
default. Targets: Linux (primary), Windows, macOS — same as v2's release matrix.

```
Cargo workspace (RustSale_v3/)
├── crates/rustsale-core/     # all v2 business logic, Tauri-free
│     db, conversation (zalo, whatsapp, facebook), campaign, scrapers,
│     google_maps_jobs, ai, lead_importer, administrative_catalog,
│     local_api (axum 127.0.0.1), notification port, events port
├── crates/zca-rs/            # moved from src-tauri/vendor/zca-rs, unchanged
├── sidecars/cloak-runtime/   # Google Maps browser sidecar, unchanged
├── src/                      # NSH app crate (root component "App")
│     ui/        design system: theme tokens + styled wrappers of nsh_base
│     store/     app stores (signals private, methods public)
│     screens/   one module per v2 screen
│     bridge/    the only place UI calls core: async calls off the UI thread,
│                core events -> signals
├── src-tauri/ + src/ (Svelte)  # v2 code: kept read-only as the parity reference,
│                                 deleted only at cutover
└── docs/v3/                  # this file, feature-parity.md, perf.md
```

Rules
- `rustsale-core` has no `tauri` dependency. Things Tauri gave us (event emit, tray,
  notifications, updater, autostart, dialogs, opener) become ports (traits) in core,
  implemented in the app crate with NSH desktop capabilities.
- UI never blocks: DB, network and Zalo work run on tokio in core; the UI gets results
  through `bridge/` into signals. Collections that can grow (messages, conversations, leads,
  places, campaign logs) use `virtual_list`, never a plain scroll of all rows.
- Rust quality: no `unwrap`/`expect` in production paths, typed errors (`thiserror`),
  `cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace` green.
  RustSale is single-user desktop: no tenant_id.
- Theme: semantic tokens only (`theme()`); no raw colors in screens.

## Running v3 next to production v2 (mandatory)

- App id `com.rustsale.crm.v3`, data dir `~/.local/share/com.rustsale.crm.v3-dev/`
  (a copy of the production DB made 22/11 02:23). Never open
  `~/.local/share/com.rustsale.crm/` from v3.
- Local API port: set `RUSTSALE_LOCAL_API_PORT=8797` for v3 (v2 owns 8787).
- **Never log the production Zalo / WhatsApp accounts into v3.** A second login can kick
  v2's session, and the VTQA bot depends on it. Use a test account or no login until cutover.
- Zalo webhook disabled in v3 dev (no allowlist file in the v3 data dir).
- Shared build dir to save disk: `CARGO_TARGET_DIR=/home/arch/Project/test/.target-rustsale-v3`.

## Performance targets (measured on the v3-dev DB copy, Linux)

| Metric | Target |
|---|---|
| Cold start to first usable screen | < 1 s |
| Scroll the longest conversation (thousands of messages) | 60 fps, no blank rows |
| Leads table (4.6k rows) filter/sort | < 100 ms |
| Idle memory | < 150 MB |
| UI thread never waits on DB/network | 0 blocking calls |

`docs/v3/perf.md` records v2 vs v3 numbers for each row.

## Phases

| Phase | Work | Who | Gate |
|---|---|---|---|
| P0a | Feature-parity inventory of v2 (`docs/v3/feature-parity.md`) | Lữ Bố, reviewed by Nakroth | complete list |
| P0b | Extract `rustsale-core` + `zca-rs` crates, Tauri-free, v2 tests green | Toro | Alice QA |
| P0c | NSH version decision, design system, app shell, perf spike (conversation + leads) | Ishar | perf targets met, else stop and report |
| P1 | Port screens onto core via `bridge/`; repetitive CRUD screens + bulk tests to Lữ Bố | Ishar, Toro, Lữ Bố | per-screen review + Alice |
| P2 | Desktop features (tray, autostart, updater, notifications), local API, webhook, packaging 3 OS | Toro, Ishar | Alice full-parity QA |
| P3 | Cutover: backup, switch production to v3, v2 kept for rollback | Nakroth | owner approval |

## Keeping v2 fixes

v2 stays in production and keeps getting fixes. Every fix merged to v2 is ported into
`rustsale-core` (cherry-pick or re-apply) in the same week; the parity list tracks it.

## Lữ Bố boundaries

Lữ Bố works only on code and synthetic fixtures. It never gets the DB copy, real
messages, tokens or anything under `~/.local/share/com.rustsale.crm*`.
