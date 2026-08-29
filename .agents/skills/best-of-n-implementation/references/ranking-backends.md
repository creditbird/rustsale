# Ranking backends

Choose one backend explicitly in the task contract. The orchestrator never falls back from one backend to the other.

## `agy-ppt` (default, no separate API key)

Use this when agy is already authenticated and the candidate pool contains two to five substantial implementations. It performs a Pivot-based Pairwise Tournament (PPT): a seeded ring identifies pivots, then candidates are compared against those pivots. Duplicate unordered pairs across phases are removed. Every remaining pair receives two fresh judges with reversed A/B placement. A third fresh judge is invoked only when the first two mapped preferences disagree.

```json
{
  "ranking_backend": "agy-ppt",
  "agy_verifier": {
    "effort": "high",
    "pivots": 1,
    "seed": 0,
    "tie_breaker": true,
    "max_parallel_pairs": 2
  }
}
```

The backend uses agy's existing authentication, `plan` mode, sandboxing, a strict JSON schema, blinded candidate labels, and persisted judge inputs/results. It does not expose or emulate raw token logprobs, so do not claim numerical equivalence with `llm-as-a-verifier`. Its operational substitute is independent pairwise reasoning plus position-bias control and auditable evidence.

The run is `INCOMPLETE` if agy is unavailable, any structured result is malformed or limited, reversed judges remain undecided, the selected candidate has an observed blocker, or the winner's aggregate margin is below the fail-closed threshold.

## `llm-verifier` (optional API-backed mode)

Use this for research reproduction or high-volume ranking when a compatible model backend exposes the token probabilities required by `llm-as-a-verifier`.

```json
{
  "ranking_backend": "llm-verifier",
  "verifier": {
    "model": "provider/model-name",
    "n_evaluations": 4,
    "pivots": 2,
    "seed": 0
  }
}
```

Install its isolated Python dependency once with:

```bash
bun run best-of-n:setup-llm-verifier
```

Provider credentials are intentionally external to the task JSON. Missing credentials, package failures, and malformed responses produce `INCOMPLETE`; they never trigger `agy-ppt` automatically.

## Evidence and interpretation

`selection.json` must name the configured backend. For `agy-ppt`, inspect `<run-dir>/agy-ppt-evidence/tournament.json` and its judge inputs. For `llm-verifier`, inspect the cached verifier result, stdout/stderr, ranking, scores, and usage.

In either mode, `SELECTED` means only “best observed eligible candidate.” Independent final review, repository quality gates, GitNexus change detection, and the Completion Lock still decide `PASS`, `FAIL`, or `INCOMPLETE`.
