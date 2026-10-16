# Canonical Consolidated Remediation Test Matrix

This is an offline matrix. `PASS` means deterministic test evidence, not live
provider evidence.

| # | Required scenario | Evidence | Result |
| --- | --- | --- | --- |
| 1 | Nominal five-slot range | `range_first_production_helpers_reserve_inventory_before_any_dispatcher_work` | PASS |
| 2 | R1 delayed fifth identities | `fifth_finalized_before_late_source_identities_waits_then_becomes_sealable` | PASS |
| 3 | R1 permanent source miss | `missing_fifth_source_evidence_cannot_seal_complete_inventory...` | PASS |
| 4 | R2 unrelated/failed/unsupported finalized tx | `identity_receipts_share_exact_reconciliation_relevance_and_version_rules`; `mixed_finalized_block_emits_only_relevant_receipts_and_precise_creation_dispositions` | PASS |
| 5 | R3 noisy pre-range delivery | range-first reservation and no-range regression tests | PASS |
| 6 | Reverse Yellowstone delivery | manifest receive provenance / reconciliation ordering tests | PASS |
| 7 | Source/finalized transaction-index mismatch | `FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH` ledger tests | PASS |
| 8 | Zero relevant produced slots | `five_produced_zero_relevant_slots_still_emit_complete_receipt_chain` | PASS |
| 9 | Skipped slots | persisted producer receipt-chain round trip | PASS |
| 10 | RPC timeout/error/malformed response | dispatcher hard-error and malformed inventory tests | PASS |
| 11 | Continuity gap | manifest continuity validation and run terminal tests | PASS |
| 12 | Source/epoch transition | manifest epoch/source conflict and run binding tests | PASS |
| 13 | Receipt tampering | atomic-bundle manifest tamper test | PASS |
| 14 | Omitted receipt | persisted receipt-chain round-trip deletes a committed bundle and fails closed | PASS |
| 15 | Expected/emitted digest mismatch | atomic-bundle digest tamper test | PASS |
| 16 | Quarantined creation | candidate classifier distinguishes unrelated, standalone and migration candidates; forward cohort rejects `CREATION_QUARANTINED` | PASS |
| 17 | Crash at durable transition | terminal checkpoint reload and `crash_points_cannot_turn_staged_receipts_or_journal_output_into_verified_admission` | PASS |
| 18 | Disk/RAM/queue/RPC pressure | `ResourceAccounting`, storage guard, reservation, and dispatcher saturation tests | PASS |
| 19 | Router CPI, loaded addresses, multi-event causal paths | authoritative decoder library fixtures | PASS |
| 20 | Deterministic replay identity/hash behavior | canonical journal recovery and JCS/hash vector tests | PASS |
| 21 | Scripted producer -> persisted receipts -> real consumer | `scripted_yellowstone_to_producer_to_persisted_cohort_validator_round_trip` | PASS |

## Commands and Results

```text
cd /root/bot/EdgeSniffer/rust-exact-ledger
CARGO_TARGET_DIR=/mnt/edgesniffer-canonical-hot/canonical-remediation-target cargo test --lib --quiet
# 54 passed, 0 failed

cd /root/bot/EdgeSniffer/rust-sidecar
CARGO_TARGET_DIR=/mnt/edgesniffer-canonical-hot/canonical-remediation-target cargo test --lib --quiet
# 108 passed, 0 failed

CARGO_TARGET_DIR=/mnt/edgesniffer-canonical-hot/canonical-remediation-target cargo test --bin kefi_finalized_rpc_reconciliation_canary_v1 --quiet
# 23 passed, 0 failed
```

`cargo test --all-targets` was additionally attempted and is blocked by
pre-existing compilation errors in unrelated legacy `yellowstone_v3_*` bins.
Those bins are not imported or exercised by the finalized-RPC canary. The
focused production target used for this release candidate compiles and tests
cleanly.
