# RustSale Best-of-N verifier criteria

## Ground Truth Note

Trust repository artifacts and observed command output, not a candidate's claim of success. The candidate pool may contain several plausible implementations and none is guaranteed to be correct. Rank technical evidence; do not reward verbosity, diff size, or confident narration.

## Criteria

### Requirement and root-cause completeness {#requirements}

Determine whether the implementation enforces every stated requirement and invariant at the authoritative layer. Prefer a root-cause fix that covers equivalent entry points and reject literal-example patches, downstream symptom handling, UI-only enforcement of backend rules, or silent scope omissions.

### Regression and contract safety {#regression}

Inspect the patch for semantic correctness and preservation of API, IPC, type, schema, state, error, cancellation, compatibility, and rollout contracts. Penalize unhandled direct callers, false-success paths, unsafe fallbacks, race conditions, duplicate effects, and unrelated changes.

### Security and data integrity {#safety}

Assess authorization, tenant isolation, input boundaries, filesystem and database effects, secrets, concurrency, idempotency, partial failure, and recovery. Prefer least-privilege enforcement at the trusted boundary with explicit failure propagation.

### Verification quality {#evidence}

Judge only observed evidence. Prefer a valid RED or baseline proof, focused tests that kill a superficial patch, relevant integration/runtime checks, and fresh results produced after the final candidate edit. Penalize skipped gates, stale output, self-comparisons, swallowed errors, and claims unsupported by command output.
