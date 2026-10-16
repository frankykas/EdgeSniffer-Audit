# Canonical R2 Source Transaction-Index Review

Review target: `ecd9f6ebc25516c08740e8442b91f30654677153`.

## Verdict

The R2 removal of `FILTERED_TRANSACTION_INDEX_MISMATCH` correctly prevents
Yellowstone **delivery order** from becoming scientific order, but its comment
and field name were misleading: `SubscribeUpdateTransactionInfo.index` is not
arrival metadata. It is Yellowstone's source transaction index in the block.

The current exact `(slot, signature)` set comparison remains sufficient to
prove bounded **filtered relevance membership** when it is bound to the
finalized-RPC block receipt. It is not a substitute for canonical ordering;
canonical ordering must continue to come from finalized RPC:

```text
slot -> finalized transaction index -> instruction path -> semantic ordinal
```

Removing the source-index equality check drops a useful independent
consistency diagnostic. It does not make finalized-RPC ordering ambiguous, but
it means a malformed Yellowstone `index` value is no longer detected. Before a
scientific release path relies on that field for anything, it should be
renamed/migrated to `source_transaction_index` and its equality with finalized
RPC made an explicit, separately documented fail-closed cross-check. It must
never be called arrival order.

## Direct answers

1. `observed_arrival_index` is currently populated from
   `SubscribeUpdateTransactionInfo.index` (`info.index` in the producer). The
   Yellowstone implementation copies it from `ReplicaTransactionInfoV3.index`.
   The Agave interface documents that field as the transaction's index in the
   block, not stream arrival order.
2. It is source/provider transaction-position metadata. It is not the
   authoritative finalized ordering proof by itself: finality, block identity,
   and the canonical transaction list remain established by finalized RPC.
   Actual arrival provenance is already recorded independently as
   `receive_sequence` and `received_at_unix_ns`.
3. The old equality check was **not** semantically invalid merely because
   Yellowstone messages can arrive out of order. It compared two transaction
   position fields, not message arrival sequence. Given a source record bound
   to the same finalized block, it was a valid consistency check. The prior
   name/comment obscured this distinction.
4. Removing the check weakens the independent provider-metadata integrity
   diagnostic. It does not remove the required membership or ordering proof:
   finalized RPC supplies the canonical transaction index and instruction path.
   Therefore no current required guarantee is replaced with arrival order, but
   one useful source-index cross-check is absent.
5. The current same-identity invariant is an exact set equality of
   `(slot, signature)` between the hash-chained, source/epoch/continuity-bound
   Yellowstone manifest and the relevant transactions derived from the
   finalized RPC block. The resulting reconciliation receipt binds the slot to
   the finalized blockhash. Duplicate signature/indexes and missing/extra
   identities fail closed.
6. `(slot, signature)` equality is sufficient for **bounded relevance
   reconciliation** because Solana signatures identify transactions and the
   finalized block supplies their canonical indexes. It is insufficient for
   canonical semantic ordering on its own; that additionally requires the
   finalized transaction index, authoritative outer/inner instruction path,
   and semantic ordinal. A source transaction index is not required for the
   identity-set proof, but should be retained as an optional or required
   provider-consistency cross-check once its contract is frozen.

## Offline regression adjustment

The prior regression used an impossible source value (`999`) and called it
arrival metadata. It was adjusted to prove the intended property correctly:

- the source receives transaction index `2` before transaction index `0`;
- receive sequence/timestamp are reverse of final block order;
- source indexes remain valid block indexes;
- finalized reconciliation succeeds because source arrival does not determine
  canonical order.

Command run with the existing installed compiler only:

```text
cargo test --release --lib filtered_identity_manifest_v1::tests::complete_interval_reconciles_when_arrival_is_reverse_of_block_order -- --nocapture
1 passed, 0 failed
```

No binary was built or installed for this test-only/documentation correction.
No live RPC, R3 run, service action, V8 action, dust action, trading action,
or evidence change occurred.

## Evidence checked

- Yellowstone source mapping from `MessageTransactionInfo::from_geyser`:
  [`message.rs`](https://github.com/rpcpool/yellowstone-grpc/blob/master/yellowstone-grpc-geyser/src/plugin/message.rs)
  copies `ReplicaTransactionInfoV3.index` into the gRPC message's `index`.
- Agave's `ReplicaTransactionInfoV3` documentation describes `index` as the
  transaction index in the block:
  [`docs.rs`](https://docs.rs/agave-geyser-plugin-interface/latest/agave_geyser_plugin_interface/geyser_plugin_interface/struct.ReplicaTransactionInfoV3.html).
- The local pinned Yellowstone protobuf declares `index` on
  `SubscribeUpdateTransactionInfo`; the production producer persists that
  value separately from receive sequence/timestamp.

## Review conclusion

This review does not authorize R3. The immediate implementation should remain
unchanged until the owner decides whether source-index equality is retained as
an explicit extra integrity gate. If it is retained, implement it using a
correctly named `source_transaction_index` field and a test for both reversed
delivery (accepted) and source/finalized index disagreement (rejected).
