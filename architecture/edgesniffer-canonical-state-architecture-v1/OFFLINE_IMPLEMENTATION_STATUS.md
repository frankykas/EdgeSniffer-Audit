# Canonical State V1 Integrated Offline Status

## Scope respected

No live service was restarted, replaced, deployed, or reconfigured. V8, V6,
PRE_H3, frozen F01-F13/model definitions, 2CQ/execution services, historical
evidence, and archive/release policy were untouched. No unbounded raw spool
was enabled.

## Offline implementation changed

- `rust-exact-ledger/src/canonical_state_v1.rs`
  - JCS semantic serialization over schema strings/nulls/booleans/arrays;
  - `record_sha256`, receipt/checkpoint/manifest hashes now exclude their own
    hash member from JCS input; no empty-string self-hash convention remains;
  - validation that `RESOLVED` causal records contain numeric slot,
    transaction index, slash-delimited instruction path, and semantic ordinal;
  - slot buffer/resolver that orders by `slot -> transaction_index ->
    instruction_path -> semantic_ordinal`, never arrival order;
  - append-only segment writer with atomic manifest publication, recovery of a
    valid `.open` segment, arbitrary configured first sequence, and no
    duplicate sequence on reopen;
  - coverage ledger with allowed state kinds, immutable proof-bound
    `UNIVERSE_COMPLETE`, and ambiguous interval overlap rejection;
  - checkpoint integrity type and decision admission guard.
- `rust-sidecar/src/kefi_canonical_source_v1.rs`
  - durable-input consumer now verifies source/epoch binding, strict sequence
    contiguity, source-frame hash presence, and a JCS/hash-bound consumer
    checkpoint before resume.

These source edits are offline candidates only. The existing live observer
still owns a Yellowstone subscription and was not modified or restarted.

## Tests actually run

| Gate | Result |
|---|---|
| Canonical state unit suite | PASS: 7/7 |
| Full exact-ledger library suite | PASS: 19/19 |
| Canonical consumer unit suite | PASS: 2/2 |
| Reverse gRPC arrival vs transaction-index ordering | PASS |
| Invalid `RESOLVED` ordering metadata | PASS: rejected |
| Open-segment interruption/reopen/append/rescan | PASS |
| Non-one first canonical sequence (`40`) | PASS |
| Wrong reopening first sequence | PASS: rejected |
| Corrupt sealed segment/checkpoint | PASS: rejected |
| Consumer sequence gap/reorder | PASS: rejected |
| Coverage overlap and proof-less universe complete | PASS: rejected |

## Integration/equivalence gate

**BLOCKED BEFORE REPLAY; no parity claim is made.** The currently active V8
root contains health, boundaries, tracked-wallet candidates, and promoted
window products, but no durable complete all-PumpSwap frame journal suitable
for the architectural canonical reader. It is therefore not a valid input to
reconstruct the complete candidate universe or replay F01-F13.

The existing observer remains a direct Yellowstone subscriber. It cannot be
truthfully called migrated until a future, separately deployed canonical
producer emits the complete semantic stream with causal-order metadata and the
observer is refactored onto that stream. The approved restrictions prohibit
deploying/restarting V8 to make that source available in this task.

Consequently the following gates are intentionally **not run**:

- old vs new full retained-evidence replay #1 and #2;
- restart-from-checkpoint equivalence;
- F01-F13/candidate/wallet-decision/snapshot parity;
- actual canonical journal storage/resource benchmark;
- integrated disconnect, producer/consumer restart, disk-pressure and
  universe-recovery fault matrix.

There are no measured semantic differences because no scientifically valid
comparison input exists. This is **not** a zero-difference result.

## Storage/resources

The legacy `271,556` compressed bytes/row result remains explicitly excluded:
it is a choice-snapshot artifact rate, not a Canonical State event-journal
rate. No canonical bytes/event, GiB/hour/day, p50/p95/burst CPU/RAM, checkpoint
size, decision-package size, replay throughput, or recovery time has been
measured on representative retained evidence.

## Remaining hard blockers

1. Build the new canonical semantic producer and durable bounded journal in a
   future controlled deployment plan; it must include authoritative causal
   transaction/instruction ordering, not gRPC arrival ordering.
2. Refactor the PumpSwap decoder/Pool resolver/state engine into reusable
   library code and make the observer consume only the canonical journal.
3. Capture a complete retained all-supported-pool interval from that producer.
4. Run the required double replay, checkpoint recovery replay, full fault
   matrix, and real sustained/burst benchmark. Any semantic mismatch blocks
   progression and must emit its counterexample.

## Verdict

**NOT READY FOR SHADOW-LIVE.** The low-level safety primitives have passed
offline unit validation. The integrated scientific pipeline has not yet earned
equivalence or resource claims, and no live operation was started.
