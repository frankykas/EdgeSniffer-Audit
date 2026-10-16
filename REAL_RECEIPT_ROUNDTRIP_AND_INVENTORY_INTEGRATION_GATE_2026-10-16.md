# Real Receipt Round-Trip and Inventory Integration Gate

Base audit commit: `950c5b98c2a17153526c2c164692aba3cd47a7f5`.

## Result

`OFFLINE_GATE_PASS — LIVE DIAGNOSTIC NOT RUN`

This change proves the actual producer/consumer file contract offline. It does
not run Yellowstone, make live RPC requests, deploy a service, modify V8, or
admit a scientific cohort.

## Corrected Contract

The producer and consumer now share `forward_cohort_receipts_v1.rs`; it is the
only schema for these persisted JCS receipts:

- finalized identity receipt;
- finalized instruction-path receipt;
- Pump migration creation receipt; and
- per-slot bundle cross-reference validation.

All receipt hashes are SHA-256 over RFC 8785/JCS bytes of the complete object
with only `receiptSha256` omitted. The consumer no longer deserializes into a
reduced schema before hash validation.

A real mismatch was found and corrected: `CreationWitness` was emitted as
snake_case while the embedded creation witness contract is camelCase. It now
has explicit camelCase serialization, so `finalizedBlockhash`, `eventId`,
`instructionPath`, and the other witness cross-references survive the exact
producer-to-consumer round trip.

`FinalizedSlotInventory::write_atomic` writes the producer inventory in JCS.
`read_verified` reloads it, rejects noncanonical bytes, and verifies the hash
of the retained `finalized-getblocks-response.json`. The new production cohort
entry point, `seal_forward_cohort_from_persisted_receipt_chain`, loads that
persisted inventory before it reads per-slot files. It does not accept a
caller-provided inventory object as proof.

## Bounded Inventory Capture Path

`bounded_finalized_slot_inventory` now calls an offline-testable core that:

1. validates the requested range before issuing `getBlocks`;
2. retains and hashes the provider response representation;
3. requires one finalized `getBlock` proof for every produced slot;
4. writes identity, path, creation, reconciliation, and inventory receipts for
   every produced slot, including zero-relevant-event blocks;
5. represents omitted slots as explicit `FINALIZED_SKIPPED` inventory entries;
6. rejects duplicate, missing, or unexpected block responses; and
7. writes no inventory marked complete when reconciliation fails.

The provider JSON proof is retained as provider-format bytes because JSON-RPC
slot numbers are provider-native numeric values and are deliberately outside
the JCS semantic-record contract. EdgeSniffer semantic receipts and the
inventory itself remain JCS-bound.

The total diagnostic budget is always inclusive. `getBlocks` costs one request
and each range slot can cost one `getBlock`; the requested span is rejected
unless `used_requests + 1 + span <= maximum_requests`. With the authorized
16-call cap and one earlier producer call, the largest all-produced range is
14 slots (16 total calls). A pristine budget permits at most 15 range slots.

## Offline Evidence

The real canary test drives production entry-point functions, not constructed
admission receipts:

`getBlocks` fixture (slots 11 and 13) -> per-slot finalized reconciliation ->
`producer_slot_bundle` -> atomic receipt files -> persisted inventory ->
`seal_forward_cohort_from_persisted_receipt_chain`.

The fixture includes one valid `PUMP_MIGRATE_CREATE_POOL` slot, one skipped
slot (12), and one produced zero-relevant-event slot (13). The unmodified chain
admits exactly one pool. These negative cases fail closed:

- missing path-receipt file;
- byte-level mutation of a persisted path receipt without resealing;
- duplicate creation witness in the persisted creation file;
- missing required produced-slot block response; and
- range exceeding remaining RPC budget.

Commands run on the existing compatible Rust toolchain:

```text
cargo test --bin kefi_finalized_rpc_reconciliation_canary_v1   # 7 passed
cargo test --lib finalized_slot_inventory_v1                  # 3 passed
cargo test --lib forward_new_pool_cohort_v1                   # 0 focused tests; compiled
python3 scripts/testPumpSwapInitDecoderV1.py                  # PASS
cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1  # PASS
```

Production release binary SHA-256:

```text
4afbbda93d232e255cbb4f4aededd8df284e7800ec5b4dfcb36aeb533342e89f
```

The only release warning is the pre-existing unused dispatcher diagnostic
field `first_seen_ms`; it has no effect on the file contract or canary path.

## Requirement Checklist

| Requirement | Status | Evidence | Remaining work |
|---|---|---|---|
| Exact production receipt schemas | VERIFIED | Shared lossless receipt module and round-trip test | None offline |
| Producer creation/path field compatibility | VERIFIED | camelCase `CreationWitness`; full hash validation | None offline |
| Atomic persisted identity/path/creation/inventory chain | VERIFIED | `write_atomic`, `read_verified`, producer integration test | Live exercise remains |
| Receipt-only cohort admission | VERIFIED | consumer loads persisted inventory/proof and slot bundles | No scientific cohort opened |
| Tamper, missing-file, duplicate-reference failure | VERIFIED | integrated negative cases | None offline |
| Finalized range inventory path | VERIFIED | actual range helper and deterministic mock fixture | Live provider behavior remains untested here |
| Produced, skipped, and zero-relevant slot accounting | VERIFIED | slots 11/12/13 fixture | None offline |
| Inclusive RPC budget preflight | VERIFIED | `used + 1 + span` check and test | Live budget remains separately bounded |
| No incomplete interval labeled complete | VERIFIED | missing produced block causes error before inventory write | None offline |
| Deployment/live diagnostic | NOT STARTED | prohibited by assignment | Separately authorize bounded live diagnostic |

## Operational Verdict

The diagnostic is **software-ready for separately authorized bounded live
execution**. It is not live-verified and a reconciled range would still prove
only captured transaction coverage, not a complete eligible-pool universe or
an AOM scientific market interval. The unrelated dust-calibration restart loop
must remain isolated as identified by the prior operational preflight.
