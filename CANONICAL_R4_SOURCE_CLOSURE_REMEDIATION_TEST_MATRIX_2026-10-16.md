# Canonical R4 Source-Closure Remediation Test Matrix

## Offline Results

| Area | Result | Evidence |
|---|---|---|
| Source closure atomic write/read and tamper detection | PASS | `source_block_closure_v1`: 4 tests |
| Source-index mismatch is not canonical-order mismatch | PASS | `filtered_identity_manifest_v1`: 14 tests |
| Reverse arrival remains valid | PASS | `complete_interval_reconciles_when_arrival_is_reverse_of_block_order` |
| Missing, extra, duplicate, or wrong-blockhash closure identity | PASS / fail closed | `source_block_closure_v1` |
| Missing fifth closure cannot seal | PASS / fail closed | `finalized_status_without_a_filtered_block_closure_cannot_seal` |
| Finalized fifth slot before late identities | PASS / waits then seals only after evidence | `fifth_finalized_before_late_source_identities_waits_then_becomes_sealable` |
| Genuine missing source evidence | PASS / does not seal | `missing_fifth_source_evidence_cannot_seal_complete_inventory_and_zero_relevant_remains_valid` |
| Zero-relevant produced slots | PASS | `five_produced_zero_relevant_slots_still_emit_complete_receipt_chain` |
| 16-request range reservation / dispatcher starvation | PASS | `five_slot_reservation_prevents_dispatcher_starvation`, `range_first_production_helpers_reserve_inventory_before_any_dispatcher_work` |
| R1/R2/R3 regression surface | PASS | delayed identities, relevance/version rules, skipped slots, index mismatch, source-gap and storage tests in the 24-test canary suite |
| Persisted producer -> receipt-only cohort validation | PASS | `scripted_yellowstone_to_producer_to_persisted_cohort_validator_round_trip` |
| Closure hash must survive bundle and inventory chain | PASS | persisted cohort tests reopen and validate source closure files |

## Test Commands

```text
cargo +stable test --lib source_block_closure_v1
cargo +stable test --lib finalized_slot_inventory_v1
cargo +stable test --lib forward_new_pool_cohort_v1
cargo +stable test --lib filtered_identity_manifest_v1
cargo +stable test --bin kefi_finalized_rpc_reconciliation_canary_v1
```

Observed results in the isolated Linux build environment:

- closure module: 4 passed;
- finalized inventory: 3 passed;
- forward cohort quarantine guard: 1 passed;
- filtered identity/reconciliation: 14 passed;
- real canary binary integration suite: 24 passed.

The offline-only Linux release candidate was built successfully from this
implementation:

```text
SHA-256: 2677c88c80680f8d3d04b3498a2c5837bd3236a685fda7c79e7a121e0751d1f2
size:    10,605,976 bytes
```

It has not been installed or live-tested on the VPS.

The full ledger suite still contains one unrelated pre-existing WSL-mounted-filesystem timing failure in `twenty_forced_hourly_transitions_detach_before_background_compression`; it is not part of the canonical capture remediation and was not suppressed.

## Live Acceptance Criteria

Do not run a live diagnostic until all are checked at preflight:

1. Provider conformance confirms the requested filtered Yellowstone block update exists and follows the expected filter semantics.
2. The deployed release is built from this candidate and its SHA-256 is recorded.
3. Fresh evidence root, 181 GiB startup floor, 160 GiB runtime reserve, 6 GiB output cap, 8 GiB RSS cap, 45-minute limit, and 16-request limit all pass.
4. V8 stays inactive; the dust service remains untouched; no transaction-submission capability is invoked.
5. Every produced slot has finalized RPC, transaction-stream, and source-block-closure exact signature-set equality.

The live result is `PASS` only if the persisted receipt-chain verifier accepts the bounded interval. A missing closure or provider semantic mismatch is `INCONCLUSIVE`, never a reason to weaken the gate.
