# Milestone 1 Integration Blockers Completion

Base: `bd4671955aec22b191e3dc37c354251268fdbf12`.

## Result

| Criterion | Status | Evidence |
|---|---|---|
| Genuine Rust Pump migration fixture | PASS | `genuine_finalized_migration_admits` exercises a successful finalized `PUMP_MIGRATE_CREATE_POOL`. |
| Rust/Python creation semantics | PASS | Python reference suite passes; Rust has the same successful, mismatch, absent-event, malformed-account/metadata, failed, and standalone-exclusion semantics. Rust deliberately requires finalization-grade stack-height and balanced program-log evidence before it creates an authoritative path receipt. |
| Adversarial fixture matrix | PASS | Eight Rust tests cover loaded addresses/multiple inner instructions, mismatched/absent/duplicate events, malformed stack metadata, failed transactions, and standalone exclusion. |
| Actual producer receipt chain | PASS | The canary executable writes per-transaction finalized identity receipts, authoritative V2 path receipts, and V2 creation receipts. A producer-path test proves the creation receipt references the independently decoded instruction-path receipt and identity receipt. |
| Caller-controlled cohort completeness | PASS | V1 boolean admission was replaced by `seal_forward_cohort_from_receipts`. It validates hashes, source/epoch, slot/boundary, finalized identity, authoritative path, creation witness, inventory membership, and duplicate/conflicting references. |
| Bounded finalized slot-range inventory | PASS (offline) | A range is capped at 16 slots. It records `getBlocks` produced slots, explicit finalized-skipped slots, and requires one reconciled `getBlock` proof for every produced slot. Duplicates, missing proofs, tampering, and over-budget ranges fail closed. |
| Live contiguous interval proof | EXTERNALLY BLOCKED | This task prohibited a live canary. The executable has opt-in `FINALIZED_RANGE_START_SLOT`/`FINALIZED_RANGE_END_SLOT` support; a separately authorized run is required to verify Shyft's live `getBlocks` behavior and actual request budget. |
| Production release build | PASS | `kefi_finalized_rpc_reconciliation_canary_v1` builds with the installed Rust 1.99 toolchain. SHA-256: `9b9b643fbcf1d7e7cd97f04f8c44363e2f8fd702e99e1d44d254c6c92c01aeec`. |

## Test Evidence

- Rust creation decoder: 8 passed.
- Slot inventory: 3 passed.
- Receipt-driven cohort ledger: 2 passed.
- Actual canary binary, dispatcher, and producer receipt path: 6 passed.
- Python reference decoder: `PUMPSWAP_INIT_DECODER_V1_TESTS_PASS`.

The first test pass exposed two real defects: a mismatching event could be silently ignored, and duplicate event evidence could still admit one witness. The decoder now applies a single global instruction/event cardinality check, matching the Python reference's fail-closed behavior. Both regressions now pass.

## Diagnostic Limits And Preflight

The executable will enter range inventory mode only when both range environment variables are explicitly provided. It rejects ranges over 16 slots. It spends exactly one finalized `getBlocks` request plus one finalized `getBlock` request per produced slot, and refuses the diagnostic before exceeding its existing 16-call budget. A range with more than 15 produced slots is therefore rejected before the per-slot calls.

Before a separately authorized diagnostic: keep the 181 GiB startup floor, 160 GiB runtime reserve, 6 GiB output cap, 8 GiB RSS cap, 45-minute ceiling, existing Shyft credentials, and V8 paused. The old dust-calibration unit is unrelated to the producer but currently loops in `auto-restart` with `203/EXEC`; it must be isolated by its owner before any operational diagnostic, without changing it in this task.

No canary was deployed or run. No V8 service was restarted. No evidence, frozen AOM-V0 contract, or trading behavior was changed.
