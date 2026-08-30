# WhatsApp Receipt Remediation Benchmark Ledger

Title: WhatsApp Receipt Remediation Benchmark Ledger
Date: 2026-08-30
Companion plan: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-plan.md`
Companion evidence: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-evidence.md`
Companion actual status: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-actual-status.md`

## Benchmark Metrics

| Metric | Target | Baseline | Actual Post-Fix | Status |
|--------|--------|----------|-----------------|--------|
| Rust unit tests passed | >= 780 passed | 787 passed | 796 passed, 0 failed | PASSED |
| Frontend MessageBatchGroup tests passed | 5 passed, 0 failed | 5 passed | 5 passed, 0 failed | PASSED |
| `git diff --check HEAD` | 0 errors (Exit Code 0) | Exit Code 2 | Exit Code 0 (0 errors) | PASSED |
| Trailing whitespaces | 0 | 1 | 0 | PASSED |
| Knowledge Graph Nodes | > 80,000 | 81,000 | 82,020 nodes | PASSED |
| Knowledge Graph Relationships | > 90,000 | 92,000 | 94,779 relationships | PASSED |
