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

**Owner decision 22/11 02:27: UI in TSX.** NSH **mixed mode** (Rust with TSX):
`nsh.json` has `"rust"`, no `"root"`, and a TSX entry (`App.tsx`). All logic, data and heavy
work stay in Rust (`rustsale-core`), exported to TSX through the app crate's
`#[native_module("rustsale/native")]` (modules, classes, Rust components for hot paths).
TSX only describes UI and handles events (QuickJS). Read the nsh skill `mixed.md` + `tsx.md`.
No WebView. On Linux the native GTK 4 backend is the default; GPUI optional, not default.
Targets: Linux (primary), Windows, macOS — same as v2's release matrix.

```
Cargo workspace (RustSale_v3/)
├── crates/rustsale-core/     # all v2 business logic, Tauri-free
│     db, conversation (zalo, whatsapp, facebook), campaign, scrapers,
│     google_maps_jobs, ai, lead_importer, administrative_catalog,
│     local_api (axum 127.0.0.1), notification port, events port
├── crates/zca-rs/            # moved from src-tauri/vendor/zca-rs, unchanged
├── sidecars/cloak-runtime/   # Google Maps browser sidecar, unchanged
├── native/ (app crate, Rust)  # #[native_module("rustsale/native")]: the ONLY bridge
│     async calls into core off the UI thread, core events -> subscriptions,
│     Rust components only where TSX is too slow (measured)
├── app/ (TSX)
│     App.tsx    entry (mount)
│     ui/        design system: theme tokens + styled wrappers of nsh/base
│     stores/    app stores (signals private, methods public)
│     screens/   one file/folder per v2 screen
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
- Theme: semantic tokens only (token classes / `token()` in TSX); no raw colors in screens.
- Collections that can grow use NSH `List` (virtualized) in TSX; sorting/filtering of
  large data happens in Rust, TSX receives the page/window it shows.

## Google Maps scraper (owner priority)

Keep every v2 scraper capability (grid crawl, failed-job recovery, profiles, cloak-runtime
sidecar, place/lead import, administrative areas) and fix the bugs found in the v2 audit
(`docs/v3/scraper-audit.md`). Each fixed bug gets a regression test in core.

## Running v3 next to production v2 (mandatory)

- App id `com.rustsale.crm.v3`, data dir `~/.local/share/com.rustsale.crm.v3-dev/`
  (a copy of the production DB made 22/11 02:23). Never open
  `~/.local/share/com.rustsale.crm/` from v3.
- Local API port: both v3 dev shells bind 8797 by themselves (v2 owns 8787); no env var needed.
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
| P0c | NSH version decision, mixed-mode app, TSX design system, app shell, perf spike (conversation + leads) | Ishar | perf targets met, else stop and report |
| P0d | Google Maps scraper audit of v2: bugs with repro + fix proposals | Elsu | reviewed by Nakroth |
| P1 | native module over core (Toro); TSX screens: Ishar builds the template screens, Lữ Bố ports the repetitive ones + bulk tests from those templates | Toro, Ishar, Lữ Bố | Toro/Ishar review + Alice |
| P2 | Desktop features (tray, autostart, updater, notifications), local API, webhook, packaging 3 OS | Toro, Ishar | Alice full-parity QA |
| P3 | Cutover: backup, switch production to v3, v2 kept for rollback | Nakroth | owner approval |

## Keeping v2 fixes

v2 stays in production and keeps getting fixes. Every fix merged to v2 is ported into
`rustsale-core` (cherry-pick or re-apply) in the same week; the parity list tracks it.

## Lữ Bố boundaries

Lữ Bố works only on code and synthetic fixtures. It never gets the DB copy, real
messages, tokens or anything under `~/.local/share/com.rustsale.crm*`.

## Queue after P0b (decided 22/11 by Nakroth)

- P1-S scraper fixes in `rustsale-core` from `docs/v3/scraper-audit.md`, in the audit's
  order: Elsu fixes the 5 high + 25 medium findings (A/B/C/D) with a regression test each;
  Lữ Bố does the 20 low findings and fills missing regression tests from Elsu's tests as
  templates; Toro reviews every diff; Alice QA with a real grid/keyword job on a test
  profile (never the production app).
- P1-F Facebook sync: 48,604 repeated error lines in v2 logs 10–21/11 (audit "out of scope").
  Toro root-causes it in core; the fix is also offered for v2.
- Legacy commands without UI (`send_zalo_campaign`, `get_campaign_leads`,
  `start_google_maps_scrape`, `stop_google_maps_scrape`, `verify_all_zalo_sessions`) and the
  simulated multi-channel campaign delivery are kept as-is in core; not exposed in the TSX
  UI unless v2 UI uses them (see feature-parity.md notes).
