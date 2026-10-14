# Canonical State V1 Offline Implementation Status

## Scope respected

No live service was restarted, replaced, or deployed. V8, V6 evidence, frozen
models, 2CQ services, execution services, raw-spool policy, and historical
evidence were not changed.

## Implemented offline components

- `rust-exact-ledger/src/canonical_state_v1.rs`
  - schema-bound semantic records;
  - deterministic JCS-compatible restricted serializer: ASCII schema keys,
    string-encoded exact quantities, explicit nulls, sorted object keys;
  - SHA-256 hash chain over uncompressed canonical bytes;
  - strict causal order status and `ORDER_AMBIGUOUS` rejection;
  - append-only segment writer/manifests and recovery scanner;
  - canonical state, bounded flow state, source sequence enforcement;
  - coverage ledger, `UNIVERSE_INCOMPLETE` decision block, immutable failure
    receipt type.
- `rust-exact-ledger/src/lib.rs`: exports the offline module only.
- Existing `rust-sidecar/src/kefi_canonical_source_v1.rs`: independently
  tested as a restart-safe reader of the pre-existing durable source format;
  it is not yet wired into the observer or a live service.

## Test results

| Gate | Result |
|---|---|
| new canonical module unit tests | PASS: 4/4 |
| exact-ledger full library suite | PASS: 16/16 |
| existing sidecar canonical-reader test | PASS: 1/1 |
| release library build | PASS |
| JCS stable ordering / numeric rejection | PASS |
| journal restart/no-dupe replay | PASS |
| hash/sequence corruption rejection | PASS |
| `ORDER_AMBIGUOUS` and `UNIVERSE_INCOMPLETE` blocking | PASS |

## Equivalence status

**BLOCKED, not claimed.** The new writer/state engine is not yet connected to
the real PumpSwap decoder and Pool identity resolver over retained evidence.
Therefore the required historical old-vs-new comparison of pool identity,
candidate membership, eligibility, F01-F13 values, causal ordering, wallet
identity and fail-closed disposition has not been executed. No semantic
difference has been observed because no valid comparison run exists yet; this
is not equivalent to a zero-difference result.

## Storage benchmark

Read-only baseline output:
`/mnt/edgesniffer-canonical-offline-v1/semantic-storage-benchmark-v1.json`

- sealed legacy compact artifacts measured: 9;
- rows: 82;
- compressed bytes: 22,267,596;
- weighted compressed bytes/row: 271,556.

These are legacy choice-snapshot artifacts containing candidate sets, not the
new canonical event journal. They do not establish journal GiB/hour, new
bytes/event, checkpoint size, decision package size, or live write bandwidth.
Those measurements remain blocked on the real decoder/state adapter and an
equivalence-passing replay.

## Resources and recovery

The small unit replay completed in under a second. No representative CPU/RAM
or retained-evidence replay throughput measurement exists yet, so no capacity
claim is made. The release library artifact is 2,804,372 bytes. Recovery is
functionally tested only on a synthetic two-record segment; real historical
recovery performance remains unmeasured.

## Shadow-live verdict

**NOT READY FOR SHADOW-LIVE.** Required blockers are: decoder/Pool-resolver
adapter, complete F01-F13 state parity, deterministic candidate-universe
reconstruction, wallet decision binding, full historical equivalence twice plus
restart, representative storage benchmark, and the complete fault matrix over
the integrated pipeline.
