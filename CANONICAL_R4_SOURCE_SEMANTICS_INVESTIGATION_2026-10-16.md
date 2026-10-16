# R4 Source-Semantics Investigation

## Scope and verdict

This is a read-only investigation of the preserved R4 root:

```text
/mnt/edgesniffer-canonical-hot/canonical-state-v1-live-diagnostic-r4-20261016T212200Z
```

No provider connection, RPC request, installation, service action, V8 action,
or R5 occurred. R1-R4 evidence was not changed.

### Required classifications

| Question | Classification | Conclusion |
|---|---|---|
| A. Yellowstone `index` versus finalized array index | `PROVIDER_INDEX_SEMANTICS_DIFFER` | The field carried by the actual Shyft stream is not safely comparable as an exact finalized `getBlock.transactions[]` index in this subscription. EdgeSniffer does not remap it. The underlying provider mechanism remains unproven because the captured source evidence lacks bank/block identity. |
| B. Fifth selected slot with `0 / 106` source identities | `NORMAL_PENDING_SOURCE_AT_FAILURE` | R4 stopped on the earlier index hard error before its normal `AWAITING_SOURCE_MATCH` state could continue. It is not proof of permanent source loss. |

The result warrants a production design change, but this investigation makes no
production change and does not authorize R5.

## Immutable inputs

The analysis used only copies of the preserved R4 manifest, checkpoint, report,
and raw finalized responses. Their SHA-256 values are below.

| Artifact | SHA-256 |
|---|---|
| `canary-epoch-1.filtered-identities.jsonl` | `3ca6dba73177313c513ea31b02a00a276f52ced91318e346cd2152511825f17d` |
| `capture-run.checkpoint.json` | `e95e3e1097d229f23bf9cb4f76cf2edf3337ed3358c677d37a07253e39ce7a26` |
| `raw-rpc-getBlocks-range.json` | `0d5e5ac00f42917a7fe8bbe191e4471c478febda088110bc047b1fbac1d90368` |
| `raw-rpc-getBlock-457785574.json` | `d5f84e6872bf1edd24027cb8da15b9633f7d4f8dc0abfd2c50998b2abe24ccfa` |
| `raw-rpc-getBlock-457785575.json` | `9530f18a33fe0f7ddcaf81fcebf24f90aa06023a0dc09172d6ae97e01dcc75b3` |
| `raw-rpc-getBlock-457785576.json` | `fad20396f9be47fd80add649fdbfd6a5917b1490dc2e2c6460c297603f32100a` |
| `raw-rpc-getBlock-457785577.json` | `ac5cce6c6b626cab7a70aecb1eb862f4ea1a7b206656cc6d5baad02d980feedd` |
| `raw-rpc-getBlock-457785578.json` | `a40291059eed384580ce31844ef7358796ff59d1b7d8d4adcc64dd630523314a` |

The full 761-row public-signature comparison required for this review is in
[`r4-evidence-summaries/R4_SOURCE_TRANSACTION_INDEX_COMPARISON.csv`](r4-evidence-summaries/R4_SOURCE_TRANSACTION_INDEX_COMPARISON.csv)
(SHA-256 `cacf5b3794d655c23a4bf01734a89f1b9bad01491e773cb34ea0b792dfda8577`).
It records slot, signature, source index, finalized array index, equality and
difference, receive sequence/time, preserved-bank and `created_at`
availability, finalized blockhash, version, program class, direct/inner
program metadata, and the available full/successful/non-vote/filtered
ordinals. R4 subscribed with `include_entries=false`; therefore no
entry-local position, cumulative-entry transaction count, or
`starting_transaction_index` was captured. Those fields are explicitly
unavailable, not inferred.

## Exact dependency and field provenance

### Local versions actually used by R4

| Component | Exact version / source |
|---|---|
| `yellowstone-grpc-proto` | `12.5.0`, Cargo source revision `27a47608c14b529717a5fe2f47ee41316f72c520` |
| `prost` | `0.14.3` in `rust-sidecar`; `0.14.4` in the ledger |
| `tonic` | `0.14.6` |
| `agave-transaction-view` | `2.2.1` |
| Matching upstream Yellowstone revision | `27a47608c14b529717a5fe2f47ee41316f72c520` (`yellowstone-grpc-geyser 13.2.5`, built against `agave-geyser-plugin-interface 4.0.0`) |

### Trace

1. The pinned proto declares `SubscribeUpdateTransactionInfo.index` as a
   `uint64` field. It does not label it arrival order.
2. In matching upstream Yellowstone source, the server copies
   `ReplicaTransactionInfoV3.index` directly into its message object, then
   encodes that object directly as protobuf field 5. The matching Agave 4.0.0
   interface defines the V3 field as the transaction's index in the block.
   See [the pinned proto](https://github.com/rpcpool/yellowstone-grpc/blob/27a47608c14b529717a5fe2f47ee41316f72c520/yellowstone-grpc-proto/proto/geyser.proto#L221-L227),
   [the pinned Yellowstone conversion](https://github.com/rpcpool/yellowstone-grpc/blob/27a47608c14b529717a5fe2f47ee41316f72c520/yellowstone-grpc-geyser/src/plugin/message.rs#L293-L320),
   and [Agave's V3 contract](https://github.com/anza-xyz/agave/blob/v4.0.0/geyser-plugin-interface/src/geyser_plugin_interface.rs#L147-L167).
3. The R4 client copies that protobuf value once, without cast, filter-relative
   renumbering, overwrite, or arithmetic:
   `info.index -> SourceObservation::Transaction.source_transaction_index`.
4. `FilteredIdentityManifestWriter.append` converts that `u64` only to its
   decimal string representation and retains it under the legacy JSON key
   `observedArrivalIndex`. `receiveSequence` and `receivedAtUnixNs` are
   separately allocated delivery provenance.
5. The finalized-side code independently enumerates the finalized JSON-RPC
   `transactions` array and records that array position as
   `FinalizedTransactionInput.transaction_index`.

Therefore there is **no EdgeSniffer mapping, cast, overwrite, or
filter-relative indexing bug** between the protobuf field and the persisted
source value. The unfortunate legacy JSON key is misleading, but it did not
create the R4 discrepancy.

The client has no local evidence of Shyft's deployed server revision or any
provider-specific semantic override. The upstream implementation is evidence
of intended Yellowstone behavior, not proof that Shyft used that exact server
path for R4.

## Index comparison

### Aggregate result

| Measure | Value |
|---|---:|
| Finalized relevant identities across five slots | 867 |
| Source identities captured | 761 |
| Same-signature pairs | 761 |
| Source-only signatures | 0 |
| Finalized-only signatures | 106 (all in slot 457785578) |
| Exact source/finalized index equality | 327 / 761 (42.97%) |
| Index mismatches | 434 / 761 (57.03%) |
| Source index range | 1 to 1,330 |
| Finalized array-index range | 1 to 601 |
| Median source minus finalized index | +183 |
| Fixed offset | No |
| Pearson: source index versus finalized index | 0.7366 overall |
| Pearson: source index versus source receive sequence | 0.6996 overall |

The source index is **not** the receive sequence: receive order is non-monotonic
with finalized order in all four populated slots, while source index is
monotonic with finalized order in all four. It is also not a filtered ordinal,
successful ordinal, non-vote ordinal, or a fixed offset from any of those
available orders. It carries an order-like signal, but its scale changes within
and between slots.

| Candidate interpretation | R4 result | Basis |
|---|---|---|
| Raw arrival order | Rejected | `receiveSequence` is separately generated locally; it is non-monotonic in final array order and only has 0.6996 Pearson association with source index. |
| Finalized full executed array index | Rejected as an equality invariant | 434/761 differ; per-slot behavior changes from near equality in 574/575 to zero equal rows in 576/577. |
| Filtered-transaction ordinal | Rejected | Source index is not equal to the retained filtered ordinal and has no fixed offset. |
| Successful/non-vote successful ordinal | Rejected | Source index is not equal to either retained ordinal and has no fixed offset. |
| Entry-local index / cumulative entry count / `starting_transaction_index` | Not testable | R4 did not subscribe to or retain Geyser entry records. |
| Bank-specific ordering | Not testable | R4 did not retain a source bank ID or source blockhash. |
| Fixed offset | Rejected | Differences range from 0 to +777 across same-signature rows. |
| Monotonic transformation / provider-local ordering | Unresolved | Source indices are monotone with final array order per populated slot and correlate strongly in 576/577, but their changing scale has no documented interpretation in the retained evidence. |

| Slot | Source / expected | Equal | Mismatch | Finalized index range | Source index range | Median difference | Correlation to finalized index |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 457785574 | 152 / 152 | 148 | 4 | 18-487 | 18-1,174 | 0 | 0.8517 |
| 457785575 | 185 / 185 | 179 | 6 | 1-527 | 1-1,138 | 0 | 0.8963 |
| 457785576 | 186 / 186 | 0 | 186 | 73-553 | 402-1,330 | +671 | 0.9729 |
| 457785577 | 238 / 238 | 0 | 238 | 12-601 | 32-1,165 | +499.5 | 0.9741 |
| 457785578 | 0 / 106 | n/a | n/a | n/a | n/a | n/a | n/a |

Program classification among the 761 same-signature rows is 615 PumpSwap, 120
Pump, and 26 multi-program. Mismatches are not isolated to a transaction
version or program family: 363/615 PumpSwap, 61/120 Pump, and 10/26 multi-program
rows disagree.

Representative preserved discrepancies:

| Slot | Signature prefix | Source index | Finalized index | Difference | Program |
|---:|---|---:|---:|---:|---|
| 457785574 | `2ZDjkLJJ...` | 1,149 | 471 | +678 | PumpSwap |
| 457785575 | `5cNCvbFC...` | 591 | 512 | +79 | PumpSwap |
| 457785576 | `3Urjt3i6...` | 403 | 74 | +329 | Pump |
| 457785577 | `4KAEeAKh...` | 33 | 13 | +20 | PumpSwap |

The data supports neither a fixed transformation nor interpreting the value as
simple arrival order. In particular, the first two slots mostly agree while the
last two have no equal index at all.

## Bank and block identity limits

Finalized RPC provides one blockhash per target slot. R4 source records preserve
slot, signature, source index, source-frame hash, source instance/epoch, receive
sequence, and local receive timestamp. They do **not** preserve source bank ID,
blockhash, parent, Yellowstone `created_at`, or raw transaction protobuf bytes.

The slot update protobuf can carry a parent, but the R4
`SourceObservation::FinalizedSlot` retained only slot and frame hash. The
transaction protobuf contains no bank/block identifier. Consequently R4 cannot
prove that a source transaction and the RPC response came from the identical
bank/block; nor can it prove a fork explanation. A same signature at a same
slot is a strong identity match, but not a block-identity witness.

The pinned upstream server would broadcast finalized frozen-block transactions
before the finalized slot update on a finalized subscription. R4's behavior is
therefore inconsistent with that exact implementation's expected transaction
index and closure behavior. Whether this reflects a provider-version/semantic
difference, a provider bank-identity path, or another Shyft implementation
detail is not decidable from the retained fields.

## Fifth-slot timeline and completeness

| Slot | Source identities | First / last source transaction UTC | Finality sequence | What is actually preserved |
|---:|---:|---|---:|---|
| 457785574 | 152 | 21:22:01.026196 / 21:22:02.017964 | 1 | local transaction times; no finality timestamp |
| 457785575 | 185 | 21:22:02.025487 / 21:22:03.357401 | 2 | same |
| 457785576 | 186 | 21:22:03.364696 / 21:22:04.563479 | 3 | same |
| 457785577 | 238 | 21:22:04.570396 / 21:22:06.179712 | 4 | same |
| 457785578 | 0 | none before exit | 5 | finalized notification exists; no source transaction identity exists |

The capture did not persist a single cross-type stream sequence or timestamp for
the finality updates. It is therefore impossible to calculate the requested
last-transaction-to-finalized-notification deltas or determine whether normal
transactions arrive before or after finality from R4 alone. The exact range
selection and source-match-evaluation timestamps are likewise not recorded.
The preserved filesystem write times give only bounded later milestones:
`getBlocks` was written at 21:22:06.506 UTC; the five `getBlock` responses
were written from 21:22:07.674 through 21:22:09.475 UTC; checkpoint publication
was 21:22:09.562 UTC; and report publication/process failure was 21:22:09.624
UTC. The elapsed time from slot 578 finality notification to process exit is
therefore unknown, not zero and not estimated.

The code evaluates range readiness in slot order. It encounters the index
mismatch in slot 457785574 first and returns a hard error before it can keep
waiting for the fifth slot's source identities. With the index condition removed
in the offline reproducer, slots 574-577 are `MATCHED` and slot 578 is
`MISSING_FILTERED_IDENTITY`, yielding `AWAITING_SOURCE_IDENTITIES`, not a seal.

Thus slot 578 is normal pending source evidence at failure time. It is neither
evidence that an arbitrary sleep would repair the system nor evidence of
confirmed permanent loss.

## Recommended integrity model: B

**Bind the source to finalized RPC by exact `(slot, signature)` set equality;
use finalized RPC exclusively for canonical transaction order; retain source
index as non-authoritative provenance and anomaly telemetry.**

This keeps Yellowstone independently valuable:

- it is the independent filtered source whose exact signature set demonstrates
  that the selected program stream was seen;
- its hash chain, source epoch, receive sequence, source-frame hash, duplicates,
  extras, and hard source gaps still detect delivery/continuity defects;
- finalized RPC alone supplies immutable block identity, transaction order, and
  instruction-path input;
- source index remains preserved and may be checked only after source
  block/bank identity is separately demonstrated. A mismatch must produce an
  auditable anomaly, not silently become canonical order or terminate a range
  that otherwise has an exact identity set.

This is not a claim that signature equality proves a complete market universe;
it proves only the defined filtered-transaction capture for a finalized slot.

### Deterministic `AWAITING_SOURCE_MATCH` condition

The current R4 source cannot prove a close boundary, so a future design must
require all of the following before sealing a produced slot:

1. finalized `getBlock` bytes and blockhash are durably recorded;
2. the source epoch is continuous and has the target slot's finalized progress;
3. the source has supplied the exact expected `(slot, signature)` set; no
   duplicate or extra identity exists;
4. an **authoritative source closure proof**, bound to that same block identity,
   establishes that no later transaction message for the slot can arrive.

The present transaction-plus-slot subscription provides items 1-3 but does not
persist item 4. A provider-supported finalized block/closure message with
blockhash and documented ordering could supply it. If unavailable, a slot may
remain `AWAITING_SOURCE_MATCH` only until a bounded deadline; deadline expiry is
`INCONCLUSIVE`, never a seal. Time is therefore a resource bound, not evidence.

For zero-relevant slots, an empty source set is valid only after the same
block-bound closure proof. Skipped slots must be explicitly represented by the
finalized `getBlocks` inventory and cannot be inferred from source silence.

## Offline reproduction

The read-only harness
[`tools/r4_source_semantics_reproducer.py`](tools/r4_source_semantics_reproducer.py)
operates on immutable R4 copies. Its current-rule self-test covers exact match,
index mismatch, missing and extra identities. Its separate Model-B prototype
covers exact signature match despite source-index disagreement, missing source
identity, duplicate identity, observable source/RPC bank mismatch, delayed
source closure, explicit skipped slots, and zero-relevant produced slots.

| Run | Result |
|---|---|
| Current R4 rule | Reproduces `FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH` first at slot 457785574; all first four populated slots fail this rule. |
| Same data, source-index condition disabled in the harness | Slots 574-577 match exactly; slot 578 remains `MISSING_FILTERED_IDENTITY`; range is not ready. |
| Synthetic self-test | PASS. Model B accepts a zero-relevant produced slot only after explicit closure; it rejects duplicate identities and an observable block-identity conflict. |

This is an offline reproduction of the R4 decision logic, not a modified or
installed release candidate.

## Known and unknown

**Known:** EdgeSniffer mapped the field faithfully; the first four signature
sets matched exactly; the source index is monotone with finalized order but not
equal to it; slot 578 was pending when a prior hard error ended R4; current
evidence cannot close a source slot deterministically.

**Unknown:** Shyft's deployed Yellowstone server revision and any provider-side
index semantics; whether the source values arose from a different bank/fork;
the true finality-to-transaction timing because R4 discarded cross-type timing;
and the provider-supported closure witness needed for scientific sealing.

## Decision

A production change is warranted, but it must be reviewed first: remove the
unsupported source-index equality requirement as an admission condition,
preserve it as provenance/anomaly data, and add a block-identity/closure proof
before a range can seal. No patch was prepared because the closure contract must
be specified with provider evidence rather than improvised.

**R5 should not be considered until that reviewed contract and its offline
adversarial tests exist.**
