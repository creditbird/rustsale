# Implementation Completion Harness

## 1. Purpose and Authority

This document is the mandatory completion protocol for every task that creates, changes, fixes, refactors, migrates, or ships executable behavior in this repository. It applies to all coding agents and human implementers, including Codex, Claude, and Antigravity.

The harness exists to prevent a common failure mode: implementing the literal review finding, obtaining green tests for that patch, and declaring completion without proving the real requirement or security invariant.

This document is the single source of truth for implementation readiness. Task-specific plans may add stricter gates, but they may not remove or weaken these gates.

### Trigger

Run this harness for changes to:

- Application code, native code, APIs, IPC commands, jobs, or background workers.
- Authentication, authorization, filesystem access, database behavior, migrations, payments, or external integrations.
- UI behavior, stores, state transitions, error handling, or user-visible success reporting.
- Configuration, permissions, capabilities, build logic, or deployment behavior.
- Tests when those tests define or change expected product behavior.
- Remediation work prompted by a bug report, security finding, code review, or failed quality gate.

Read-only investigation and text-only documentation edits may use a reduced process. Any documentation change that establishes implementation policy, permissions, commands, or release behavior uses the full process.

### Non-negotiable Rule

Passing tests is evidence, not proof of requirement completion. Completion requires a traceable chain:

`requirement -> invariant -> authoritative enforcement -> adversarial counterexample -> fresh evidence`

Do not claim a task is complete while any Completion Lock condition in section 13 remains open.

## 2. Allowed Readiness States

Every implementation report must end in exactly one state:

### `VERIFIED READY`

Use only when every applicable Completion Lock condition passes with fresh evidence produced after the last relevant edit.

### `CONDITIONALLY READY`

Use when implementation is finished but required evidence cannot be produced because of a specific external limitation, such as unavailable credentials, hardware, operating-system access, or a third-party sandbox. State the missing evidence, affected requirement IDs, residual risk, and exact verification action. Do not describe this state as complete, fixed, production-ready, or fully tested.

### `NOT READY`

Use when any requirement remains unimplemented, unproven, failing, bypassable, or contradicted by the current behavior. State the blocker and the next concrete action.

## 3. Risk Classification

Classify the task before implementation. When uncertain, use the higher tier.

| Tier | Scope | Minimum evidence |
|---|---|---|
| Tier 1 — Local | Pure/local logic, isolated presentation, copy, or a low-risk helper with no trust boundary | Targeted unit or component test, relevant static check, diff inspection |
| Tier 2 — Boundary | Behavior crossing components, stores, API clients, route handlers, IPC layers, persistence adapters, or shared workflows | Tier 1 evidence plus contract/integration coverage and failure-path verification |
| Tier 3 — Critical | Auth/authz, tenant isolation, filesystem writes, native IPC, DB schema/migrations, payments, destructive/bulk actions, secrets, external integrations, or security findings | Tier 2 evidence plus direct-entry adversarial tests, real runtime or production-like verification, and full relevant release gates |

Risk is based on the strongest boundary touched, not the apparent size of the diff. A five-line filesystem command is Tier 3.

## 4. Mandatory State Machine

Follow these phases in order. A phase may be brief for Tier 1, but it may not be silently skipped.

```text
WAKE_UP
  -> REQUIREMENTS_LOCKED
  -> INVARIANTS_DEFINED
  -> RED_OR_BASELINE_PROVEN
  -> IMPLEMENTED
  -> TASK_QA_PASSED
  -> INTEGRATION_AUDITED
  -> ADVERSARIAL_CLOSED
  -> RELEASE_QA_PASSED
  -> VERIFIED_READY
```

If a gate fails, stop progression, repair the defect, invalidate affected downstream evidence, and rerun the required gates.

## 5. Phase 0 — Wake-up and Repository Safety

Before planning or editing:

1. Read the applicable `AGENTS.md`, `CLAUDE.md`, this harness, and relevant skills.
2. Recall Agent Memory for prior decisions, regressions, and project-specific lessons.
3. Inspect `git status --short --branch` and current diffs. Preserve unrelated work.
4. Identify the user request, review findings, acceptance criteria, and stated exclusions.
5. Check GitNexus freshness. If reindexing is required, use `bunx gitnexus analyze`; never use `node`, `npm`, or `npx`.
6. Create a plan for multi-step work and keep exactly one implementation step in progress.

**Gate:** Repository instructions are loaded, existing work is understood, and no edit has started.

## 6. Phase 1 — Requirement Ledger

Convert prose requirements and review findings into a ledger before implementation. Even a small Tier 1 task needs at least one row.

| ID | Exact requirement | Invariant | Positive case | Negative/failure case | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|---|---|
| R-01 | Observable behavior to deliver | Condition that must always hold | Expected valid use | Bypass, invalid input, cancellation, or dependency failure | Unit, integration, runtime, or manual | Plan step name | `UNPROVEN` |

Allowed row statuses:

- `UNPROVEN`: No fresh evidence yet.
- `RED`: A test or reproduction demonstrates the missing behavior.
- `GREEN`: Required evidence proves the row after the latest relevant edit.
- `BLOCKED`: Evidence cannot be produced; the final state cannot be `VERIFIED READY`.

Ledger rules:

- Split combined requirements when they have different failure modes or evidence levels.
- Record implicit requirements such as cancellation, permission denial, partial failure, retry, duplicate submission, and success-message accuracy.
- Map every review finding to one or more ledger rows. Closing the wording of a finding is not enough; close the underlying invariant.
- Mark a row `GREEN` only when its positive and negative/failure cases are both covered where applicable.

**Gate:** Every requested behavior and review finding has an ID, invariant, counterexample, evidence requirement, and owner.

## 7. Phase 2 — Root Cause, Trust Boundaries, and Invariants

Answer these questions before selecting a fix:

1. What must always be true when the operation succeeds?
2. What must never be possible, including through direct API or IPC invocation?
3. Which layer is authoritative for enforcing that rule?
4. Can a caller bypass the intended UI, client validation, or navigation flow?
5. Which input, identity, path, response, or state crosses a trust boundary?
6. What happens on cancellation, timeout, permission denial, disk full, network failure, stale state, and partial completion?
7. What counterexample would defeat a superficial patch?
8. Could concurrency, retries, symlinks, traversal, duplicate events, or reordered responses violate the invariant?
9. Can the system report success when the authoritative operation failed?

### Boundary Trace

Trace the complete path, not only the edited file:

```text
UI/event
  -> client helper/store
  -> IPC or HTTP boundary
  -> backend command/route/service
  -> database/filesystem/external service
  -> backend response
  -> client state and user feedback
```

For each boundary, record:

- Input origin and trust level.
- Validation and authorization owner.
- Data transformation and loss risks.
- Error, cancellation, and retry semantics.
- Observable success criteria.

Frontend checks improve UX but do not enforce backend security. Enforce security invariants at the authoritative backend or native boundary.

**Gate:** The root invariant, authoritative layer, bypass path, and superficial-patch counterexample are explicit.

## 8. GitNexus Safety Protocol

Use GitNexus before and after code changes:

1. Use `query` for unfamiliar concepts and execution flows.
2. Use `context` for the full caller/callee and process view of a target symbol.
3. Run upstream `impact` before editing every function, method, class, route handler, or shared symbol.
4. Use `api_impact` before modifying an API route handler.
5. Report direct callers, affected processes, affected modules, and risk before editing.
6. Warn before proceeding when risk is `HIGH` or `CRITICAL`; update every d=1 dependent.
7. Run `detect_changes` after implementation to verify the actual affected scope.
8. Treat graph results as navigation and impact evidence, not runtime correctness proof.

**Gate:** All edited symbols have pre-change impact evidence, and the implementation plan covers every direct dependent.

## 9. Phase 3 — Baseline and RED Proof

Before the production fix:

- Bug or review remediation: reproduce the defect or add a test that fails for the correct reason.
- New behavior: create an executable acceptance check, contract test, or focused failing test where practical.
- Refactor: capture behavior and integration baselines before structural edits.
- Migration: capture pre-migration state, expected transformation, rollback behavior, and data integrity checks.

The RED proof must fail against the superficial implementation too. A test that only asserts the chosen code path can preserve the original defect.

If RED cannot be produced, record why, raise the evidence requirement, and do not silently substitute code inspection for runtime proof.

**Gate:** The missing requirement is observable before the fix, or the ledger records a justified evidence exception that prevents `VERIFIED READY` until resolved.

## 10. Phase 4 — Implementation Discipline

Implement the smallest complete change that enforces the invariant at the authoritative layer.

- Fix the root cause, not only the reported symptom or literal review wording.
- Update all equivalent entry points and duplicated workflows covered by the requirement.
- Preserve cancellation as a non-error only when no authoritative operation occurred.
- Propagate real failures; do not fall back to a different path that can produce false success.
- Emit success UI only after authoritative success is confirmed.
- Use least-privilege commands and capabilities. Avoid generic arbitrary file, shell, SQL, or network primitives.
- Keep validation, serialization, naming, and status mapping consistent across layers.
- Update tests, permissions, command registration, migrations, and documentation in the same change when required.
- Avoid unrelated cleanup and scope expansion.

Any implementation edit invalidates evidence for affected ledger rows. Return those rows to `UNPROVEN` until checks are rerun.

**Gate:** The code enforces the invariant, all affected entry points are updated, and no known bypass remains unaddressed.

## 11. Phases 5–8 — Evidence and Verification

### Evidence Ladder

Use the strongest applicable level:

| Level | Evidence | What it can prove |
|---|---|---|
| E1 | Code/diff inspection and static analysis | Structure, obvious omissions, type/format issues |
| E2 | Unit/component tests | Local transformations and isolated state behavior |
| E3 | Contract/integration tests | Cross-module calls, response shapes, registration, persistence adapters |
| E4 | Real runtime or production-like test | OS dialogs, native IPC, real DB/filesystem semantics, browser behavior, external service interaction |

Rules:

- Higher-level claims require higher-level evidence. E2 mocks cannot prove an E4 operating-system or persistence behavior.
- A mock may prove orchestration, but not the behavior of the mocked boundary.
- Existing green tests count only when they assert the requirement and were rerun after the last relevant edit.
- Record skipped, ignored, filtered, flaky, or environment-gated tests. A green command with the critical test skipped is not evidence.
- Verify assertions, not only process exit codes or call counts.

### Task QA

Run the smallest relevant checks first:

1. Focused unit/component test for changed logic.
2. Focused backend or frontend integration test.
3. Relevant typecheck, compiler, formatter, and linter checks.
4. Manual or automated user interaction for changed UI behavior.

### Integration Audit

Walk the boundary trace end to end and verify:

- The UI calls the intended helper with correct data.
- The helper calls the registered route or command.
- The backend validates and authorizes independently of the UI.
- Persistence or external side effects match the contract.
- Failure and cancellation return distinguishable outcomes.
- User-visible state and toast/message reflect the authoritative outcome.

### Adversarial Closure

Build a task-specific matrix from these categories:

| Category | Required challenge |
|---|---|
| Direct entry | Call API/IPC/service without the intended UI |
| Malformed input | Empty, oversized, wrong type, invalid encoding, special characters |
| Boundary bypass | Traversal, forged identifier, unauthorized tenant, stale token, alternate route |
| Dependency failure | Permission denied, disk full, DB error, timeout, offline service |
| State race | Double click, duplicate request, retry, stale selection, reordered response |
| Cancellation | User cancel before side effect; verify no false error or success |
| Compatibility | Locale, timezone, encoding, platform, spreadsheet/browser/native behavior |
| Observability | Correct error propagation and no false success reporting |

Add at least one **superficial-patch killer**: a test or runtime check that would pass the intended solution and fail an incomplete shortcut.

### Release QA

After the final relevant edit, run fresh gates appropriate to the risk tier. Discover available scripts instead of inventing commands. Typical repository gates include:

```bash
bun test
bun run check
cargo test
cargo fmt --check
git diff --check
```

Tier 3 work requires the full relevant frontend and backend suites unless an explicit external blocker forces `CONDITIONALLY READY`.

**Gate:** Every ledger row has fresh evidence at its required level, including negative and failure behavior.

## 12. Independent Review Pass

Before final reporting, review the change from a hostile perspective:

1. Ignore the implementation description and reconstruct behavior from the diff.
2. Attempt to violate each invariant using the shortest bypass path.
3. Confirm tests fail if the core enforcement is removed or weakened.
4. Check for untested alternate entry points, stale mocks, permissive fallbacks, and misleading success messages.
5. Compare actual changed files and GitNexus processes with the declared scope.
6. Reopen any ledger row whose proof depends on assumption rather than evidence.

The implementer may perform this pass, but it must be logically independent from the original patch reasoning. For Tier 3, prefer a separate reviewer when active instructions permit delegation; otherwise perform and document the hostile review locally.

## 13. Completion Lock

`VERIFIED READY` is forbidden until every applicable condition is true:

- [ ] Every requirement and review finding has a ledger ID.
- [ ] Every ledger row is `GREEN`; none is `UNPROVEN`, `RED`, or `BLOCKED`.
- [ ] Root invariants are enforced at authoritative boundaries, not only in the UI.
- [ ] Direct API/IPC/service bypass paths were tested where applicable.
- [ ] Positive, negative, cancellation, and dependency-failure behavior were verified.
- [ ] At least one superficial-patch killer exists for remediation and Tier 2/3 work.
- [ ] Evidence matches the required ladder level; cross-boundary claims are not mock-only.
- [ ] All evidence is fresh after the last relevant edit.
- [ ] Success reporting occurs only after authoritative success.
- [ ] GitNexus impact was reviewed for edited symbols and all d=1 dependents were handled.
- [ ] GitNexus `detect_changes` matches the intended files, processes, and modules.
- [ ] Relevant tests, typechecks, compiler checks, formatting, and `git diff --check` pass.
- [ ] Skipped or unavailable checks are disclosed and reflected in readiness status.
- [ ] No unrelated work was overwritten, reverted, committed, or included accidentally.
- [ ] Final claims contain only commands and results actually observed in the current run.

One unchecked item means the task is not `VERIFIED READY`.

## 14. Final Report Template

Use this structure for implementation handoff:

```md
Status: VERIFIED READY | CONDITIONALLY READY | NOT READY

Scope
- Requirement IDs delivered and files changed.

Invariants
- Authoritative invariant and enforcement layer.
- Superficial-patch counterexample closed.

Evidence
- Requirement ID: command/runtime check — observed result.
- Quality gate: command — observed result and skipped-test count.

GitNexus
- Pre-change impact risk, direct dependents, and affected processes.
- Post-change detect_changes result.

Residual Risk
- None, or exact missing evidence/blocker and required next action.
```

Do not report test counts copied from another agent, earlier run, or pre-edit state. Do not say “all tests pass” when only a filtered suite ran.

## 15. Example — Native CSV Export

### Superficial remediation

A generic backend `write_text_file(path, content)` command is changed to require a `.csv` extension and an existing parent directory. Unit tests confirm those checks.

This is not sufficient. Extension and parent validation constrain syntax; they do not prove authorization or trusted user consent.

### Root invariant

The native backend may write export content only to a destination authorized for that specific export operation through a trusted save flow. A compromised or directly-invoked webview command must not gain a general arbitrary-file-write capability.

Valid enforcement designs include:

- The native backend owns both the trusted save dialog and the write in one narrow export command.
- A trusted native layer issues a short-lived, single-use authorization tied to the selected path and export operation, then consumes it during the write.
- A Tauri capability/plugin scope enforces an equivalently narrow destination policy and the design is proven with direct IPC tests.

### Required counterexamples

- Invoke the backend command directly with an arbitrary `.csv` path outside the authorized flow.
- Attempt traversal, alternate separators, canonicalization edge cases, and symlink redirection.
- Cancel the save dialog and verify no file, success toast, or error toast is produced.
- Simulate dialog failure, permission denial, and disk-full/write failure; verify an error reaches the UI and no success is reported.
- Verify UTF-8 BOM, CSV escaping, formula-injection defense, Vietnamese phone leading zeros, and exact field/status mapping.
- Verify both Leads and Contacts entry points, including per-row, page selection, and “select all matching” behavior.

The superficial-patch killer is the direct IPC arbitrary-path test: it passes only when authorization is enforced independently of the intended UI.

## 16. Why Green Suites Can Still Be Incomplete

An implementation remains incomplete when:

- Tests encode the patch instead of the requirement.
- Only the happy path or mocked boundary is exercised.
- UI restrictions are mistaken for backend authorization.
- The original defect was never reproduced before the fix.
- Equivalent entry points were not traced.
- Error fallback converts authoritative failure into false success.
- Test evidence predates the final edit.
- Review findings are marked closed without an adversarial counterexample.

The harness closes these gaps by making invariants, bypass tests, evidence strength, freshness, and the Completion Lock mandatory for every implementation task.
