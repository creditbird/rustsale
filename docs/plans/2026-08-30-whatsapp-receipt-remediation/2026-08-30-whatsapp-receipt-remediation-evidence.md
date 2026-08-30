# WhatsApp Receipt Remediation Evidence Ledger

Title: WhatsApp Receipt Remediation Evidence Ledger
Date: 2026-08-30
Companion plan: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-plan.md`
Companion benchmark: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-benchmark.md`
Companion actual status: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-actual-status.md`

## Evidence Log

| Evidence ID | Phase / Slice | Description | Command / Action | Result / Output |
|-------------|---------------|-------------|------------------|-----------------|
| `E0-P0A-GITDIFF` | P0-A | Baseline `git diff --check HEAD` | `git diff --check HEAD` | Exit code 2 (trailing whitespace at supervisor_test.rs:396) |
| `E0-P0A-GITNEXUS` | P0-A | GitNexus index and change detection | `gitnexus detect_changes(scope=all)` | 339 changed symbols, 34 affected, 21 files, risk: critical |
| `E1-P1A-DIFF` | P1-A | Clean whitespace verification | `git diff --check HEAD` | Exit code 0 (0 whitespace errors) |
| `E2-P1B-NORM` | P1-B | State machine and direction isolation | `cargo test conversation::whatsapp::normalizer_test` | 43 passed; 0 failed (including inbound immunity and terminal states) |
| `E3-P1C-SUP` | P1-C | WhatsApp event mapper & error ack | `cargo test conversation::whatsapp::supervisor_test` | 10 passed; 0 failed |
| `E4-P1D-LIVE` | P1-D | Listener error handling & liveness test | `cargo test test_whatsapp_receipt_listener_event_handling_logging_and_liveness` | 1 passed; 0 failed |
| `E5-P1E-DEAD` | P1-E | Dead code removal verification | `cargo check --tests` | Exit code 0 (clean compilation, no dead code) |
| `E6-P1F-FULL` | P1-F | Full backend & full frontend test suites | `cargo test --manifest-path src-tauri/Cargo.toml` & `bun run test` | 798 passed (0 failed, 1 ignored) backend; 625 passed, 1 skipped, 0 failed across 70 files frontend |
| `E7-P1F-ZALO` | P1-F | Zalo crate unit tests & group chat multi-page sync | `cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml` & `cargo test conversation::zalo` | 29 passed (0 failed) in zca-rs; 13 passed (0 failed) in conversation::zalo |
| `E8-P1F-GITNEXUS` | P1-F | GitNexus fresh graph analyze & change detection | `bunx gitnexus analyze --force` & `gitnexus detect_changes` | Freshly indexed on commit HEAD; 7,479 nodes, 19,364 edges |
| `E9-P1F-E4-DISCLOSURE` | P1-F | Tier-3 External Integration Runtime Boundary Disclosure | Integration probe review | Downgraded to CONDITIONALLY READY: mock/unit/integration passing 100%, live network probes (Zalo live probe #[ignore] & live WhatsApp network) require live staging credentials |
