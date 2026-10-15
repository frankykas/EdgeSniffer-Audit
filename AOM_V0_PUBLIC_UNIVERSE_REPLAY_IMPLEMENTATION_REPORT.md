# AOM V0 Public Universe Replay: Offline Implementation Report

## Scope

This report covers an offline framework only. It does not read frozen
qualification outcomes, call RPC, open a source connection, mutate evidence,
or control any service. The implementation has no tracked-wallet inputs.

## Numbered completion checklist

| # | Requirement | Status | Implementation evidence |
|---:|---|---|---|
| 1 | Versioned immutable public input contract | VERIFIED | `aom_v0_public_universe_replay.py`: public-event, decision-boundary, execution-assumption, and partition schemas; identity/provenance validation and canonical hashes. |
| 2 | Partition admission validator | VERIFIED | Separate verified gates for transaction coverage, canonical order, pre-state via event contract, and universe proof. |
| 3 | Separate captured interval vs eligible universe | VERIFIED | `validate_partition_admission` emits different fail-closed reasons. |
| 4 | Inventory existing eligible partitions | BLOCKED | Read-only VPS inventory found 1,577 sealed `PUMP_EXACT_EVIDENCE_LEDGER_V1` manifests; zero declare `exactReplayComplete=true`, and none supplies AOM's independent complete-universe proof. No historical evidence was modified or promoted. |
| 5 | Public F01-F13 analogues | VERIFIED | Strict public `public_f01_f13`, masks, causal/event order, no wallet fields. |
| 6 | Transparent ranker / denominator / no-trade | VERIFIED | `deterministic_rank` preserves denominator, rejections, ties, top-K and no-trade. |
| 7 | Bounded exact-transition simulator | VERIFIED | Explicit constant-product-supported contract and cash ledger; AMM fee stays embedded exactly once. |
| 8 | 30/60/120 exploratory horizons | VERIFIED | `fixed_horizon_marks` accepts exactly the fixed 30/60/120-second set; no horizon optimizer is included. |
| 9 | Chronological evaluation gating | VERIFIED | `chronological_evaluate` refuses missing admission or independent cost evidence. |
| 10 | Leakage/gap/universe/cost/no-fill/replay tests | VERIFIED | `test_aom_v0_public_universe_replay.py`. |
| 11 | Resource bounds/no raw duplication | VERIFIED | Row, output, runtime, protected-free-space checks; reference-only writer; no RPC/raw input writer. |
| 12 | Publish code/tests/findings/resources/blockers | VERIFIED | This report and sanitized code/test files. |

## Real-data admission finding

No real historical partition is admitted by this implementation at publication
time. A read-only inventory found 1,577 sealed hourly-style manifests under the
existing shared-window evidence root. All 1,577 are
`PUMP_EXACT_EVIDENCE_LEDGER_V1`; all are temporally closed, but all report
`exactReplayComplete=false`. More importantly, none is an immutable proof that
the complete eligible-pool universe is known. Transaction coverage alone would
not be enough. The implementation therefore produces no autonomous ranking,
economic, or profitability result for real markets.

## Resource measurements

Synthetic benchmark, measured locally: 50,000 public-event rows completed
feature reconstruction in 6.282 seconds and deterministic ranking in 0.850
seconds; Python `tracemalloc` peak was 104,230,978 bytes. A compact
identity/reference JSONL was 7,366,670 bytes, or 147.33 bytes/row. The output
writer produced only canonical JSON reference rows; it does not write
full-meta or raw transactions. The enforced defaults are 500,000 rows, 64 MiB
output, 20 GiB protected free space, 900 CPU/runtime seconds, and zero RPC
requests. These are
engineering limits, not a production forecast. Real CPU/RAM/bytes-per-event
measurements remain BLOCKED pending one independently admissible sealed
partition.

## Verification results

- The base publication described **10 passed** tests, but the committed suite
  at that point contained 10 tests only after a late boundary-contract test
  was added; the earlier report text was stale. After this corrective review,
  the AOM-specific deterministic suite is **13 passed**.
- Syntax check: `py_compile` passed for the AOM module.
- Broader sanitized Python suite: **71 passed, 1 failed**. The failing test is
  outside AOM: `test_end_to_end_canonical_manifest_registry_restart_is_byte_identical` stops at its protected frozen-scorer hash assertion. Its expected
  hash is `ae1771fa...c801cff4`; the unchanged checked-in scorer hashes to
  `49c4ad5c...1566a647`. This implementation did not modify either frozen
  file and does not reconcile or bypass that protected mismatch.

## Readiness verdicts

- **Offline engineering readiness:** READY. Contract, validator, features,
  ranker, simulator, and deterministic tests exist.
- **Real-data replay readiness:** BLOCKED. Requires partition-level immutable
  transaction-coverage, ordering, state, and complete-universe proofs.
- **Autonomous scientific validation readiness:** BLOCKED. Requires an
  independent later certified public interval and frozen execution contract.
- **Paper-trading readiness:** BLOCKED. No autonomous model has passed a
  real-data chronological economic evaluation.

## Remaining work

1. Produce one retained partition manifest meeting every admission gate without
   reconstructing a universe from wallet-selected windows.
2. Bind independently observed execution costs/fills to the same source and
   timing contract.
3. Benchmark the framework on that admitted partition; only then decide whether
   a bounded autonomous discovery experiment is justified.
