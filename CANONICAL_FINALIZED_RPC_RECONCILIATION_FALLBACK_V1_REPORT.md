# Canonical Finalized-RPC Reconciliation Fallback V1

## Scope

Implemented and tested the existing fallback design only:

`filtered Yellowstone identities -> append-only manifest -> finalized getBlock -> exact relevant-set/index reconciliation -> fail-closed release`.

No V8, kEFi service, historical evidence, AOM-V0, cohort, or execution service was changed.

## Checklist

| Requirement | Status | Implementation evidence | Test/live evidence | Remaining work |
|---|---|---|---|---|
| Manifest before selection | VERIFIED | `filtered_identity_manifest_v1.rs` durable JCS/hash-chain writer | restart/corruption/duplicate tests pass | Wire into future producer deployment only after canary passes |
| Finalized RPC relevant-set reconciliation | VERIFIED offline | `reconcile_finalized_block` derives relevance from finalized message/account/program IDs | missing/extra/duplicate/version/fork tests pass | Live dispatcher flow needs repair |
| Exact signature/index reconciliation | VERIFIED offline | provider index is compared against finalized block index when present | `provider_index_is_proven_against_finalized_block_before_release` passes | None for offline component |
| Strict fail-closed release | VERIFIED | only `MATCHED` can release | all mismatch paths reject | None |
| Bounded resources | VERIFIED | 64 MiB manifest quota, 16,384 identity cap, 256 pending-slot test cap, 8 RPC calls, 8 GiB RSS service cap | live peak RSS 48,771,072 bytes | Reduce live pending pressure before another run |
| Real filtered source / protobuf semantic decoder | VERIFIED | isolated canary uses Yellowstone `SubscribeUpdateTransaction` and `decode_events` | 14,801 transactions, 6,399 events, zero decoder rejections | Instruction-path decoder remains a separate release prerequisite |
| Bounded live reconciliation canary | BLOCKED | isolated transient service, separate hot-volume root | zero releases; bounded dispatcher budget exhausted while awaiting missing identities | Correct slot scheduling and re-run |
| No live-service interruption | VERIFIED | no service unit modified | V8, watchdog, and ledger active after all runs | None |

## Live Evidence

The final isolated run was rooted at:

`/mnt/edgesniffer-kefi-hot/canonical-finalized-rpc-canary-20261015T0925Z`

It ran with a transient 8 GiB memory cap and 200% CPU quota. It made 8 finalized RPC calls, observed 14,801 filtered transactions and 6,399 decoded PumpSwap events, with zero decoder rejections. Peak RSS was 48,771,072 bytes. The retained canary output was 9.6 MiB; the apparent 738,914,304-byte free-space delta included compilation/cache activity in the isolated build root, not permanent raw full-block capture.

No slot was released. The disposition was `WAITING_FOR_FILTERED_IDENTITIES`, then `RPC_BACKLOG_CAP`. This is correct fail-closed behavior.

## Root Cause

The canary dispatched finalized RPC reconciliation too eagerly over a high-volume finalized transaction stream. A `MISSING_FILTERED_IDENTITY` receipt was kept pending, but its fixed eight-call budget was consumed before all transaction identities for the pending slots had arrived/been grouped. The resulting pending-slot set grew past the temporary canary cap.

This does **not** establish a provider completeness failure, and it does **not** establish a scientifically valid interval. It proves the release gate refused to claim one.

## Required Repair Before Another Canary

Replace the canary's eager per-update polling with a bounded slot dispatcher that:

1. journals every identity immediately;
2. retains a bounded active-slot table;
3. queues a slot only after an explicit finalized-reconciliation scheduling condition;
4. retries only that slot at the configured cadence until exact match or deadline/budget failure;
5. applies backpressure before accepting more pending slots;
6. records a failed receipt for any deadline, source epoch change, duplicate, extra, index mismatch, or RPC error.

The retry condition must not be an inactivity-based scientific ordering barrier. A slot is released solely on exact finalized set/index equality; an unresolved slot remains unreleased and makes the interval unusable.

## Verdict

`FALLBACK_CANARY_BLOCKED`

The reusable manifest/reconciliation components are ready. The live dispatcher is not yet ready for Stage-1 scientific capture. A repaired isolated canary must pass at least one slot with exact finalized identity/index reconciliation, then separately prove instruction-path, coverage, state, and complete candidate-universe requirements.
