# Shyft Complete-Block Inventory Capability Gate

**Base reviewed:** `288300aed29d165ff03d16f42e31ef6947e8394c`  
**Mode:** read-only provider-capability investigation.  
**Live impact:** none. No subscription was opened, no RPC request was made, no
service was restarted, and no evidence/cohort/trading path was changed.

## Decision

`PROVIDER_CAPABILITY_UNVERIFIED`

The installed Yellowstone protobuf can represent a block containing a slot,
block identity, executed-transaction count, transaction signatures, and
transaction indexes. That is *necessary* for a complete slot inventory. It is
not sufficient: the exact Shyft `blocks` request used by the current canary is
account-filtered, and the public documentation says that Shyft requires at
least one block filter and streams only blocks matching the filter. The docs do
not state whether `block.transactions` on such a matched block is the complete
slot transaction vector or only matching transactions. No retained evidence
proves it either.

Do not authorize Stage-1 capture from this result. Storage reserve, exact
decoder-fixture parity, and complete eligible-pool-universe bootstrap remain
separate gates even if the provider answers this question positively.

## Numbered Completion Checklist

| # | Requirement | Status | Evidence / conclusion |
|---:|---|---|---|
| 1 | Inspect sanitized subscription config, protobuf usage, client settings, filters and limits | VERIFIED | `sanitized-code/canonical-state-v1/kefi_canonical_canary_v1.rs` was inspected; the exact request and bounds are summarized below. |
| 2 | Determine whether `SubscribeUpdateBlock` supplies every identity/index in a slot rather than only filtered output | BLOCKED | The protobuf shape can carry identities/indexes, but Shyft's public filter documentation does not guarantee complete-vector semantics for an account-filtered block. |
| 3 | Distinguish block inventory, filtered blocks, transaction subscriptions, block metadata, and finalized RPC | VERIFIED | Separate capability table below. |
| 4 | Investigate decoding-memory constraints and bounded-receiver safety | VERIFIED | Current tonic/prost handling is whole-message decode; current bounded receiver is not proven safe for arbitrary required blocks. |
| 5 | Inspect docs, retained evidence, and configuration without paid requests or a live stream | VERIFIED | Official docs, source, prior provider/RPC/canary reports, and existing retained-evidence findings reviewed. No network data request was made. |
| 6 | Define smallest independent finalized-RPC conformance test and bounds | VERIFIED AS PLAN | Eight-slot / eight-request conformance plan below. It is not run by this gate. |
| 7 | Specify skipped-slot, fork, version, filter, reconnect, and missing-block treatment | VERIFIED AS PLAN | Fail-closed dispositions below. |
| 8 | Evaluate filtered-identity plus finalized-RPC fallback | VERIFIED | This is technically valid for *finalized offline scientific verification* if exact reconciliation succeeds; it does not supply prospective low-latency completeness. |
| 9 | Identify Shyft confirmation needed | VERIFIED | Exact written questions below. |
| 10 | Audit AOM-V0 proof-bundle dependencies versus self-consistent hashes | VERIFIED | Required independent evidence and remaining block are documented below. |
| 11 | Audit original assignment line by line / no scope expansion | VERIFIED | This report is the only change. No collector, canary, provider call, deployment, cohort, or architecture redesign was performed. |

## Exact Existing Subscription Shape

The standalone canary source builds one `SubscribeRequestFilterBlocks` entry,
`supportedPumpBlocks`, with:

```text
account_include = [Pump program, Pump CPI router program, PumpSwap program]
include_transactions = true
include_accounts = false
include_entries = false
commitment = confirmed
transactions / slots / blocks_meta / entry = empty
```

The gRPC client uses a 10-second connection timeout,
`max_decoding_message_size(128 MiB)`, and a 4,096-update in-process receive
queue. It does not configure an application RSS cap, a streaming protobuf
decoder, or a per-block byte cap below the gRPC message ceiling. Its current
block path also performs decoder and Pool-RPC work after decode; that is
explicitly unsuitable for a compact-inventory proof path.

The compiled protobuf fields cited by the prior provider audit are:

```text
SubscribeUpdateBlock:
  slot, blockhash, executed_transaction_count, transactions[]
SubscribeUpdateTransactionInfo:
  signature, transaction, meta, index
```

That establishes **payload capability**, not **delivery completeness**. In
particular, the source comment describing a received block as a "complete
block" is an unverified assumption for this filtered request and must not be
used as scientific evidence.

## What Each Surface Can and Cannot Prove

| Surface | What it provides in the inspected configuration/documentation | What it cannot establish by itself |
|---|---|---|
| `SubscribeUpdateBlock` with `account_include` filter | A block update for a block involving a requested account; with `include_transactions=true`, a transaction vector and provider indexes are available in the message. Shyft requires a block filter. | That `transactions[]` contains **every** transaction in the slot, including unrelated transactions needed to prove that none of the relevant ones were omitted. |
| Unfiltered complete-block subscription | Potentially the right inventory surface if Shyft supports an admissible non-selective filter and documents full-vector semantics. | Availability, billing, message-size ceiling, and bounded decoding are unproven. Shyft's requirement for at least one filter means this cannot be assumed to exist. |
| Filtered transaction subscription | Prompt, matching transaction records and full-meta data for subscribed program/account filters. | A closed per-slot complete relevant set, canonical transaction order, or absence of omitted matching transactions. Arrival order is not canonical order. |
| `blocks_meta` | Block-level metadata without full block data. | Transaction identity inventory and transaction indexes; it cannot close the candidate/event set. |
| Finalized JSON-RPC `getBlock(..., transactionDetails=full)` | A finalized block identity and ordered full transaction data, including the data needed to derive a program/account relevance set. The prior bounded test succeeded. | Low-latency prospective signals; it is a later independent verifier and may be rate-/credit-limited. |

Official Shyft documentation says a block request needs at least one filter and
that only blocks matching the filter are streamed. It documents
`accountInclude` as a way to select blocks involving an address; it does not
define whether the returned transaction vector is slot-complete under that
filter. [Streaming Blocks & BlocksMeta](https://docs.shyft.to/solana-yellowstone-grpc/docs/streaming-blocks-and-blocksmeta)

Shyft documents `getBlock` with `finalized` commitment and `full` transaction
details. [Shyft `getBlock`](https://docs.shyft.to/solana/rpc-calls/http/getblock)

## Memory / Receiver Finding

`max_decoding_message_size(128 MiB)` only limits or rejects an incoming
message. Tonic/prost first deserializes the complete protobuf message into an
in-memory Rust value; iterating its transaction vector afterward is not
wire-streaming. This is confirmed by the current implementation and the
earlier producer canary, where complete block decode combined with Pool RPC
work approached the 1 GiB process cap before useful output was published.

Therefore:

- the existing receiver is **not proven safe** for all block payloads needed
  for a complete inventory;
- an oversize message must create `BLOCK_PROOF_OVERSIZE`, a coverage blackout,
  not a partial proof;
- removing Pool RPC/semantic decode from Stage 1 can lower retained working
  state, but cannot remove the whole-message allocation;
- only a separately measured, strict-message-cap probe can establish that a
  specific provider configuration is bounded enough for a controlled test.

## Retained-Evidence Findings

The prior bounded finalized-RPC conformance check established that the
configured Shyft regional RPC returned finalized full blocks for three sampled
slots and could derive a Pump/PumpSwap program/account-relevant superset from
actual transaction data. It did **not** validate Yellowstone filtered-set
completeness because V8 retained selected candidates, not every filtered
Yellowstone identity.

The AOM-V0 admission inventory found 1,601 legacy manifests with
`exactReplayComplete=false`; none has independently reconciled finalized-block
coverage plus a complete candidate-universe proof. Those historical manifests
cannot retroactively prove the block-filter semantics at issue here.

## Smallest Bounded Conformance Test (Not Run)

This test answers only whether the **exact selected Shyft block request** is
complete for a small sample. It does not authorize scientific capture.

### Fixed bounds

| Control | Frozen test limit |
|---|---:|
| Candidate slots | 8 consecutive finalized slots observed on one uninterrupted test connection |
| Finalized RPC requests | 8 `getBlock(finalized, full, maxSupportedTransactionVersion=1)` calls; no retry that would exceed 8 |
| gRPC block message ceiling | 32 MiB per `SubscribeUpdateBlock` |
| Process RSS ceiling | 512 MiB; breach is an explicit failure |
| Wall-clock runtime | 5 minutes |
| gRPC queue | 16 messages; backpressure/overflow is failure |
| Durable output cap | 32 MiB, compact identities/proofs only; no raw block spool |
| Protected free-space reserve | 60 GiB; preflight and each append must remain above it |
| RPC concurrency / rate | 1 concurrent request, maximum 1 request/second |

The test stores only canonicalized compact records: slot, block hash, expected
count, `(transaction_index, signature)` pairs, derived relevance bits,
connection/epoch receipt metadata, and hashes. It does not persist full block
responses. Full responses are parsed in memory one at a time and discarded
after the compact proof has been written.

### Procedure and pass condition

1. Open one isolated test subscription using the exact current filtered block
   request. Record an explicit source instance and epoch.
2. Accept eight distinct slots only if each block update fits the 32 MiB cap,
   reports a non-conflicting slot/blockhash/count, and no stream end, error,
   reconnect, or queue overflow occurs.
3. After each slot reaches finalized status, issue its one bounded full
   `getBlock` request. Require an unambiguous finalized block identity and a
   transaction vector supported by the configured transaction version.
4. Derive the finalized block's complete relevant set from actual account-key,
   loaded-address, and compiled-instruction program references. Signatures
   alone are never relevance evidence.
5. Compare all delivered block identities/indexes to the finalized block
   inventory, then compare the derived relevant set to the captured filtered
   set. Require exact equality after duplicate/conflict checks.
6. Produce one compact receipt per slot and one run receipt. Any failure
   yields `UNVERIFIED`; it never retries by silently replacing a slot.

An all-eight-slot pass is a bounded **empirical conformance result**, not a
general provider guarantee. A permanent scientific design still needs Shyft's
written semantics and a separate candidate-universe bootstrap proof.

### Required fail-closed cases

| Case | Required treatment |
|---|---|
| Skipped slot | Only accept `FINALIZED_SKIPPED` after finalized RPC independently returns the authoritative skipped/absent disposition and no captured block/transaction identity conflicts. Otherwise fail. |
| Fork / different blockhash for a slot | Reject the slot and whole test interval; do not choose one arrival. |
| Unsupported transaction version | Reject the slot. Do not compare a partial finalized inventory. |
| Filtered vector missing / extra / duplicate identity | Reject the slot and test interval. |
| Reconnect, stream error, receive error, queue overflow, source epoch change | End the interval; no cross-epoch bridge or retry. |
| Missing or unavailable finalized block | Reject the slot. A later slot cannot cover it. |
| Oversize gRPC message / RSS breach / output quota / reserve breach | Stop before additional state; record a non-scientific failed conformance run. |

## Fallback Architecture

If Shyft cannot provide a demonstrably complete block vector under safe size
bounds, the only currently credible fallback is:

```text
prospective filtered Yellowstone transactions
  -> append every received filtered identity before selection
  -> compact source/epoch manifest
  -> finalized full getBlock, one bounded slot at a time
  -> derive finalized relevant set from actual message/account/instruction data
  -> exact set reconciliation
  -> only then release a finalized scientific interval
```

Prospective filtered events may remain useful **operational signals**. They
are not complete scientific coverage until the finalized reconciliation
succeeds. The fallback has bounded RPC budgets and must fail closed on a
backlog, provider error, version gap, source discontinuity, or mismatch. It
does not bridge a gap, does not make `getBlock` a low-latency feed, and does
not establish the historical candidate universe by itself.

RabbitStream, ShredStream, and WebSocket feeds remain optional latency
witnesses only; they cannot replace execution metadata, finalized block
identity, and exact reconciliation in the frozen Canonical State v1 contract.

## Questions Shyft Must Answer Explicitly

1. For `SubscribeRequestFilterBlocks.accountInclude=[...]` and
   `includeTransactions=true`, does an emitted `SubscribeUpdateBlock` contain
   the **entire slot's** transaction vector with canonical transaction indexes,
   or only transactions matching the filter?
2. Is there a supported request shape that streams every finalized block's
   inventory while satisfying Shyft's mandatory-filter rule? If so, provide
   the exact filter semantics and an example.
3. Are `transaction.index` values the finalized canonical block positions?
   What does Shyft emit when a fork/reorg changes a prior confirmed block?
4. What maximum block-message bytes, transaction count, subscription rate, and
   concurrent block streams apply to this plan? Are message limits hard or
   negotiated?
5. Does the subscription include versioned transactions, loaded addresses,
   failed transactions, and inner-instruction metadata without omission under
   the selected filter? Which transaction versions are supported?
6. What replay/from-slot guarantees, retention horizon, reconnect semantics,
   and billing/credit treatment apply to block subscriptions and finalized
   `getBlock` calls?

No answer may be inferred from a successful connection or a few matching
transactions. Written provider confirmation must be preserved with any future
conformance receipt.

## AOM-V0 Proof-Bundle Audit

`AOM_V0_IMMUTABLE_PROOF_BUNDLE_V1` verifies that artifacts are hash-bound and
bound to a partition, source instance, epoch, and boundary. That prevents
tampering or accidental cross-binding. It does **not** independently make a
claim true merely because internally consistent files hash correctly.

Before real-data AOM admission, each proof must be independently established:

| Required proof | Independent underlying evidence |
|---|---|
| Finalized transaction coverage | Finalized block identity/inventory for every claimed slot, explicit skipped-slot dispositions, captured filtered-identity manifest, and exact block-derived relevant-set reconciliation. |
| Canonical causal order | Complete transaction message plus execution metadata, resolved outer/inner instruction paths, finalized transaction indexes, and explicit `ORDER_AMBIGUOUS` rejection. |
| Valid pre-decision state | A causal checkpoint derived only from reconciled events preceding the boundary, with feature validity/missingness preserved. |
| Complete eligible-pool universe | A complete-universe recovery certificate proving every potentially eligible pre-boundary pool is restored or provably ineligible. A complete transaction interval does not imply this. |
| Boundary binding | Exact public decision time/checkpoint and source/epoch bindings matching the above evidence. |

Hashing these artifacts records their identity. It cannot turn a provider's
unproven filtered delivery into complete coverage, reconstruct missing events,
or eliminate selection/wallet leakage. Therefore the AOM gate remains blocked
until the provider/conformance gate **and** the separate decoder/state/universe
gates pass.

## Exact Next Action

Send the six questions above to Shyft support. In parallel, keep the bounded
eight-slot conformance-test plan frozen but **do not run it** until the
operator explicitly authorizes its billed/request-limited execution and the
60 GiB reserve can be independently preflighted. A written answer that
filtered blocks are not slot-complete immediately selects the finalized-RPC
reconciliation fallback; an ambiguous answer leaves this verdict unchanged.
