# Agent Command Evidence & Verification Harness Plan

## Metadata

- Date: `2026-10-01`
- Status: `complete`
- Plan: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-plan.md`
- Evidence: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-evidence.md`
- Benchmark: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-benchmark.md`
- Actual status: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-actual-status.md`

## Goal

Thiết lập một hệ thống Command Evidence & Verification Harness hoàn chỉnh, chống ảo giác (hallucination-proof) cho RustSale_v2, tham khảo từ kiến trúc thành công của pb-desk. Hệ thống cho phép:
1. Agent trong phiên làm việc khai báo Task Contract (`.agents/task.json`) và tự động kiểm chứng công việc qua `bun --no-env-file run agent:verify <sessionId>`.
2. Chạy độc lập qua driver `bun --no-env-file run agent:run "<prompt>"`.
3. Kiểm tra tính toàn vẹn của mã nguồn qua SHA-256 snapshot, phát hiện stale code, bắt lỗi skip/todo test, và lưu trữ log độc lập tại `~/.local/state/rustsale-agent/`.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- Always use `bun` and `bunx` instead of `node`, `npm`, and `npx`.
- Do not modify existing CRM features or Tauri commands; keep changes strictly isolated to harness infrastructure.

## Problem

Hiện tại, việc xác nhận hoàn thành công việc trong RustSale_v2 chủ yếu dựa vào việc agent tự viết các file markdown trong `docs/plans/` hoặc chạy các lệnh ad-hoc. Điều này dẫn đến nguy cơ:
- Agent tự tuyên bố "PASS" mà không thực sự chạy lệnh kiểm tra, hoặc chạy lệnh sai tham số nhưng vẫn báo thành công.
- Code bị sửa sau khi kiểm tra nhưng vẫn được coi là đã pass (stale evidence).
- Test bị skip hoặc rỗng (empty test suite) nhưng vẫn báo pass do exit code = 0.
- Không có runner độc lập tự động đánh giá task theo các tiêu chí nghiệm thu máy đọc được (machine-readable task contract).

## Scope

- Tạo engine `scripts/agent/core.ts`, `tsconfig.json`, và test suite `scripts/agent/core.test.ts`.
- Tạo runner `scripts/agent/verify.ts` và independent driver `scripts/agent/run.ts` kèm test `scripts/agent/run.test.ts`.
- Cấu hình các lệnh trong `package.json`: `agent:verify`, `agent:run`, `agent:eval`, `agent:typecheck`.
- Tạo AGY always-on rule `.agents/rules/command-evidence.md` và mẫu task contract `.agents/task.json`.
- Soạn thảo tài liệu hướng dẫn và đặc tả kỹ thuật `docs/agent-harness/README.md` và cập nhật `AGENTS.md`.

## Non-Goals

- Triển khai native hooks trên AGY (vì hook execution chưa được xác nhận ổn định trên AGY 1.2.3 tại môi trường hiện tại, như bài học từ pb-desk).
- Thay thế hoàn toàn `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`; thay vào đó, hệ thống này đóng vai trò là engine tự động cung cấp bằng chứng E1-E4 có chữ ký snapshot cho harness hiện có.

## Requirements

- **R1 (Real Command Execution & Snapshot Integrity)**: Các lệnh kiểm tra phải được spawn thực tế, capture toàn bộ stdout/stderr vào file log với SHA-256 hash. Snapshot toàn bộ source tree trước và sau khi chạy kiểm tra; nếu source bị thay đổi hoặc bằng chứng quá 6 tiếng thì tự động coi là STALE.
- **R2 (Task Contract & Invariant Gates)**: File `.agents/task.json` phải ánh xạ mọi requirement với các acceptance checks cụ thể. Không cho phép ghi đè hay bỏ qua BASE_CHECKS (rust-clippy, rust-tests, frontend-check, frontend-build, harness-types, harness-evals).
- **R3 (Zero False-Positive Test Parser)**: Parser phải đọc stdout của `cargo test` và `bun test`, bắt buộc số lượng test pass > 0 và số lượng skipped/todo/ignored = 0 (trừ khi được chỉ định rõ).
- **R4 (Independent Driver Loop)**: `scripts/agent/run.ts` có thể gọi AGY headless, truyền feedback lặp lại tối đa 3 lần nếu verification thất bại, và chỉ trả về PASS khi `agent:verify` trả về 0 và toàn bộ receipt hợp lệ.

## Acceptance Criteria

- `bun test scripts/agent` chạy thành công 100% các unit test cho snapshot, task validation, process execution, và receipt evaluation.
- `bun run agent:typecheck` không có lỗi TypeScript.
- `bun --no-env-file run agent:verify <sessionId>` chạy thành công trên repo với kết quả JSON rõ ràng và exit code phản ánh đúng trạng thái thực tế.
- File `.agents/rules/command-evidence.md` được tạo đúng đường dẫn workspace của RustSale_v2.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source-of-truth files, classify each surface, record blocked or missing pieces, and update later phase status assumptions, next actions, and work steps from evidence.
  - Implementation Gate: no implementation or editing starts until `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-actual-status.md` has a final P0 decision.
  - Acceptance: actual status identifies correct, partial, missing/unbound, fake-or-stub, and blocked surfaces for this scope.

### P1: Core Engine & Type Configuration

- Phase Goal: Xây dựng core engine xử lý snapshot, process execution, test parsing, task validation và receipt evaluation.
- Phase Boundary:
  - In scope: `scripts/agent/core.ts`, `scripts/agent/tsconfig.json`, `scripts/agent/core.test.ts`.
  - Out of scope: CLI commands và AGY driver.
  - Dependencies: `bun`, `cargo`.
- Phase Implementation Rule: Implement `P1-A`, verify it, record evidence, then continue to `P1-B`.
- Ordered Slice List:
  - P1-A: Tạo `scripts/agent/core.ts` và `scripts/agent/tsconfig.json`
  - P1-B: Tạo `scripts/agent/core.test.ts` và xác nhận tất cả test pass

- [x] P1-A: Tạo `scripts/agent/core.ts` và `scripts/agent/tsconfig.json`
  - Goal: Cung cấp đầy đủ các types, hàm snapshot sha256, atomicJson, runCheck với spawn/kill/timeout, test output parsers (bun + cargo), và BASE_CHECKS phù hợp với RustSale_v2 (`src-tauri/Cargo.toml`).
  - Scope Boundary:
    - Editable: `scripts/agent/core.ts`, `scripts/agent/tsconfig.json`.
    - Inspect-only: `src-tauri/Cargo.toml`, `package.json`.
    - Preserve-only: Các file mã nguồn hiện có của dự án.
    - Out of scope: CLI runners.
  - Non-Goals: Không chỉnh sửa Cargo.toml.
  - Pre-flight Questions:
    - Data source: Local filesystem and subprocess execution.
    - Display permission: N/A.
    - DB read flow: N/A.
    - DB write flow: N/A.
    - Render location: Terminal stdout/stderr.
    - UI behavior flow: N/A.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: `bun test scripts/agent`.
    - Cleanup/quarantine: Temporary test directories cleaned up in finally blocks.
    - External side effects: State written to `~/.local/state/rustsale-agent/`.
    - N/A notes: Đây là công cụ CLI/harness, không có UI/DB.
  - Work Steps:
    1. Viết `scripts/agent/tsconfig.json` cấu hình strict TypeScript cho Bun.
       - UI flow check: N/A
       - DB/data flow check: N/A
       - Render location check: N/A
       - Mini QA: Chạy `bun x tsc -p scripts/agent/tsconfig.json`
       - Evidence target: `E1-P1A-TS1`
    2. Viết `scripts/agent/core.ts` với đầy đủ BASE_CHECKS:
       - rust-clippy (`cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`)
       - rust-tests (`cargo test --manifest-path src-tauri/Cargo.toml`)
       - frontend-check (`bun run check`)
       - frontend-build (`bun run build`)
       - harness-types (`bun run agent:typecheck`)
       - harness-evals (`bun test scripts/agent`)
       - UI flow check: N/A
       - DB/data flow check: N/A
       - Render location check: N/A
       - Mini QA: Syntax và import check bằng `bun run scripts/agent/core.ts`
       - Evidence target: `E1-P1A-CORE1`
  - Implementation Gate: `anvien file-detail package.json` đã xác nhận risk thấp.
  - Acceptance:
    - Source: `scripts/agent/core.ts` và `tsconfig.json` tồn tại, định nghĩa đầy đủ Task, Check, Result, Evidence, validateTask, snapshot, evaluateEvidence.
    - Runtime/UI: N/A.
    - DB/data: N/A.
    - Behavior test: Imports hợp lệ dưới Bun.
    - Cleanup/quarantine: File temp được dọn dẹp.

- [x] P1-B: Tạo `scripts/agent/core.test.ts` và kiểm thử toàn diện core engine
  - Goal: Đảm bảo snapshot phát hiện file thêm/sửa/xóa, validateTask chặn contract sai, runCheck ghi nhận exit code thật, và evaluateEvidence chặn bằng chứng rỗng/stale/bị sửa.
  - Scope Boundary:
    - Editable: `scripts/agent/core.test.ts`.
    - Inspect-only: `scripts/agent/core.ts`.
    - Out of scope: Runner files.
  - Non-Goals: Không test tính năng app.
  - Pre-flight Questions:
    - Behavior test: `bun test scripts/agent/core.test.ts`.
    - Cleanup/quarantine: Dùng `mkdtempSync` và `rmSync` trong `finally`.
  - Work Steps:
    1. Soạn thảo `scripts/agent/core.test.ts` bao phủ 100% các ca: snapshot invalidation, task validation, empty test rejection, exit code capture, receipt validation.
       - Mini QA: Chạy `bun test scripts/agent/core.test.ts`
       - Evidence target: `E1-P1B-TEST1`
  - Implementation Gate: `P1-A` hoàn thành.
  - Acceptance:
    - Source: `scripts/agent/core.test.ts` hoàn chỉnh.
    - Behavior test: `bun test scripts/agent/core.test.ts` trả về PASS 100%.

### P2: Verifier Runner & Independent Driver

- Phase Goal: Xây dựng CLI entry point `verify.ts` và headless AGY driver `run.ts`.
- Phase Boundary:
  - In scope: `scripts/agent/verify.ts`, `scripts/agent/run.ts`, `scripts/agent/run.test.ts`.
  - Out of scope: Thay đổi `package.json`.
- Phase Implementation Rule: Implement `P2-A`, sau đó implement `P2-B`.
- Ordered Slice List:
  - P2-A: Tạo `scripts/agent/verify.ts`
  - P2-B: Tạo `scripts/agent/run.ts` và `scripts/agent/run.test.ts`

- [x] P2-A: Tạo `scripts/agent/verify.ts`
  - Goal: Cung cấp runner chạy `agent:verify <sessionId>`, đọc `.agents/task.json`, khóa process lock, chạy toàn bộ base checks và task checks, ghi log vào `~/.local/state/rustsale-agent/`, xuất output JSON và exit code tương ứng.
  - Work Steps:
    1. Viết `scripts/agent/verify.ts` đảm bảo lockfile chống chạy trùng, validate task contract, đo snapshot trước và sau, ghi receipt `evidence.json`.
       - Mini QA: Chạy thử cú pháp bằng `bun run scripts/agent/verify.ts` không truyền sessionId (phải ra exit 2).
       - Evidence target: `E2-P2A-VERIFY1`
  - Acceptance: `verify.ts` hoạt động đúng đặc tả, xử lý timeout và process kill.

- [x] P2-B: Tạo `scripts/agent/run.ts` và `scripts/agent/run.test.ts`
  - Goal: Cung cấp driver chạy ngoài terminal điều phối AGY headless, tự động gọi verify sau mỗi turn, cung cấp feedback loop tối đa 3 lần.
  - Work Steps:
    1. Viết `scripts/agent/run.ts` với hàm `parseAgentEnvelope`, `runAgent`.
    2. Viết `scripts/agent/run.test.ts` kiểm thử `parseAgentEnvelope`.
       - Mini QA: Chạy `bun test scripts/agent/run.test.ts`.
       - Evidence target: `E2-P2B-RUN1`
  - Acceptance: Tests của driver pass.

### P3: Scripts Wiring, Rule & Task Contract

- Phase Goal: Tích hợp các lệnh vào `package.json`, tạo rule `command-evidence.md` và mẫu `.agents/task.json`.
- Ordered Slice List:
  - P3-A: Thêm scripts vào `package.json`
  - P3-B: Tạo `.agents/rules/command-evidence.md` và `.agents/task.json`

- [x] P3-A: Thêm scripts vào `package.json`
  - Goal: Thêm `agent:verify`, `agent:run`, `agent:eval`, `agent:typecheck` vào `package.json`.
  - Work Steps:
    1. Cập nhật `package.json` với 4 scripts mới.
    2. Chạy `bun run agent:typecheck` và `bun run agent:eval` để kiểm chứng.
       - Evidence target: `E3-P3A-PKG1`
  - Acceptance: Các scripts được thực thi trơn tru qua `bun run`.

- [x] P3-B: Tạo `.agents/rules/command-evidence.md` và `.agents/task.json`
  - Goal: Thiết lập rule `always_on` cho AGY và file contract mẫu.
  - Work Steps:
    1. Tạo `.agents/rules/command-evidence.md` với workspace path `/home/arch/Project/test/RustSale_v2`.
    2. Tạo `.agents/task.json` mẫu chuẩn schema.
       - Evidence target: `E3-P3B-RULE1`
  - Acceptance: Files tồn tại đúng vị trí và nội dung.

### P4: Documentation & Integration

- Phase Goal: Viết tài liệu hướng dẫn và tích hợp vào `AGENTS.md`.
- Ordered Slice List:
  - P4-A: Tạo `docs/agent-harness/README.md`
  - P4-B: Cập nhật `AGENTS.md`

- [x] P4-A: Tạo `docs/agent-harness/README.md`
  - Goal: Cung cấp tài liệu đầy đủ bằng tiếng Việt và tiếng Anh về cách dùng `agent:verify`, cấu trúc file task contract, và cách chạy driver.
  - Evidence target: `E4-P4A-DOC1`

- [x] P4-B: Cập nhật `AGENTS.md`
  - Goal: Đưa rule Command Evidence vào `AGENTS.md` để mọi session AGY tự động tuân thủ.
  - Evidence target: `E4-P4B-DOC2`

### P5: End-to-End Verification & Closure

- Phase Goal: Chạy kiểm thử toàn diện toàn bộ harness và nghiệm thu.
- Ordered Slice List:
  - P5-A: Chạy full test và live verification test
- [x] P5-A: Chạy full test và live verification test
  - Goal: Chạy `bun run agent:eval`, `bun run agent:typecheck`, và một phiên `bun --no-env-file run agent:verify <sessionId>` mẫu thành công 100%.
  - Evidence target: `E5-P5A-FINAL1`
