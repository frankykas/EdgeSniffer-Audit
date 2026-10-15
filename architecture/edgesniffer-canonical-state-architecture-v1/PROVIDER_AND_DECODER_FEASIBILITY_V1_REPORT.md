# Canonical State v1 Provider and Decoder Feasibility

**Status:** focused read-only feasibility investigation.  No live service,
canary, cohort, holdout, model, observer, 2CQ component or historical evidence
was changed.

## Evidence Consulted

1. Shyft Yellowstone blocks / blocks-meta documentation, retrieved 2026-10-14:
   <https://docs.shyft.to/solana-yellowstone-grpc/docs/streaming-blocks-and-blocksmeta>
2. Shyft Yellowstone transaction-filter documentation, retrieved 2026-10-14:
   <https://docs.shyft.to/solana-yellowstone-grpc/docs/transaction-streaming>
3. Shyft RabbitStream overview/comparison, retrieved 2026-10-14:
   <https://docs.shyft.to/solana-shredstreaming/rabbitstream-overview> and
   <https://docs.shyft.to/solana-shredstreaming/yellowstone-vs-rabbitstream-comparison>
4. Jito ShredStream documentation and `mev-protos` `shredstream.proto`,
   retrieved 2026-10-14:
   <https://docs.jito.wtf/lowlatencytxnfeed/> and
   <https://github.com/jito-labs/mev-protos/blob/master/shredstream.proto>
5. Installed Yellowstone generated protobuf definitions and the existing
   PumpSwap decoder, inspected in `/root/bot/EdgeSniffer` without modification.

## Checklist

| Requirement | Status | Verified conclusion |
|---|---|---|
| 1. Compact authoritative block inventory from Shyft | PARTIALLY_FEASIBLE | `SubscribeUpdateBlock` carries slot, blockhash, `executed_transaction_count`, and every transaction's signature/index. It is authoritative enough to construct an exact inventory **if the received block transaction vector is complete**. |
| 1a. Identify all Pump/PumpSwap-relevant transactions without pool RPC | FEASIBLE | Full transaction messages carry static account keys, compiled outer instructions, and loaded addresses in meta. The frozen relevance predicate can be program/account-key membership for Pump, router and PumpSwap. No Pool account RPC is required for that predicate. |
| 1b. Provider guarantee that filtered block payload is a full inventory | BLOCKED | Shyft documents that block filters select blocks involving included accounts and that `includeTransactions` includes transactions, but the docs reviewed do not explicitly promise whether a matched block returns every transaction or only matching transactions. The prior full-block canary payload was large, but that is not a contractual proof. Obtain written provider confirmation or run a bounded provider conformance fixture before scientific use. |
| 2. True streaming protobuf decode | BLOCKED in current client stack | Tonic/prost deserializes one `SubscribeUpdateBlock` message to an in-memory struct before application code sees it. Iterating `block.transactions` is bounded *post-decode*, not wire-streaming. `max_decoding_message_size` caps/rejects a large message; it does not stream it. |
| 2a. Bounded processing after decode | FEASIBLE | Stage 1 can immediately reduce an accepted block to identities/relevance bits, discard full transaction objects, and defer all decoder/RPC work. This bounds retained application state, but not protobuf message allocation. |
| 2b. Controlled-message fallback | FEASIBLE only as fail-closed probe | Set a strict message cap and reject/receipt oversized blocks. This is safe but cannot establish continuous coverage unless the provider supplies a compact inventory API. It is not a complete production solution. |
| 3. Relevant-set derivation | FEASIBLE conditional on 1b | Scan complete block transaction account keys plus compiled instruction program indices and loaded addresses. Produce `{slot,index,signature,relevant}` before any semantic decoder or Pool RPC. Exact equality with stage-2 filtered identities proves no relevant transaction was omitted. |
| 4. Exact instruction path from full-meta | FEASIBLE with decoder work | Yellowstone full-meta contains message outer instructions; meta `inner_instructions` has outer index, ordered inner instruction vectors and optional stack height; logs are ordered. Existing decoder currently throws this structure away and only returns log indexes. |
| 4a. Existing decoder ready today | NO | `decode_events(meta)` has no transaction message and labels every canonical event `ORDER_AMBIGUOUS`. It cannot bind a log `Program data` payload to a unique outer/inner invocation path. |
| 4b. Required decoder change | VERIFIED DESIGN | Change API to accept `(message, meta)`. Build loaded account-key table; enumerate `(outer_index)` and `(outer_index, inner_ordinal, stack_height)` instruction candidates; parse log invocation/return stack; bind each PumpSwap event to exactly one compatible instruction based on program, invocation nesting, event sequence, and account/pool identity. Zero or multiple matches => `ORDER_AMBIGUOUS`, fail closed. |
| 5. RabbitStream | NOT_CANONICAL | Shyft documents RabbitStream as pre-execution: it has transaction intent but no execution meta, logs, inner instructions, or final status. It cannot establish exact semantic events, final state, or block completeness. |
| 5a. AllenHark/Jito ShredStream | NOT_CANONICAL | Jito emits serialized entries/shreds. They can reconstruct transaction bytes/order before execution, but do not provide execution meta/logs/status and require complete shred recovery/fork handling. They cannot replace Yellowstone for current PumpSwap semantic evidence. |
| 5b. Shyft WebSocket | NOT_A_SOLUTION | `blockSubscribe` can deliver a getBlock-style full block but remains a full JSON block object; it offers no compact inventory contract or byte-streaming decode. Slot/account/log subscriptions have weaker completeness. |
| Simplest valid solution | CONDITIONAL | Yellowstone complete block inventory + Yellowstone filtered full-meta transactions, with Stage 1 proof and Stage 2 exact reconciliation, **only after** Shyft confirms/tests full inventory semantics under the selected block filter. |
| Fallback if Shyft cannot provide that evidence | BLOCKED / FAIL_CLOSED | Do not use RabbitStream, ShredStream or WebSockets to fill the proof gap. Retain operational capture only. Obtain a provider/API that supplies a bounded complete block inventory, or run a self-operated/authoritative Solana replay source. |

## Actual Payload Findings

The installed Yellowstone protobuf has:

- `SubscribeUpdateBlock { slot, blockhash, executed_transaction_count,
  transactions[] }`;
- `SubscribeUpdateTransactionInfo { signature, transaction, meta, index }`;
- `Transaction.message.instructions[]` for outer compiled instructions;
- `TransactionStatusMeta.inner_instructions[]`, each with its parent outer
  `index`, ordered inner instructions, and optional `stack_height`.

This is sufficient raw material for both transaction-index order and a
fail-closed instruction-path binding.  It does **not** by itself solve the
provider delivery contract or application-level decoded-message size.

## Memory and Completeness Analysis

### Yellowstone blocks

The existing canary demonstrated that complete block messages can be large
enough to approach the 1 GiB process cap when the consumer starts decoding and
resolving the entire block.  The problem was compounded by doing Pool RPC work
inside the block path.  Separating Stage 1 removes that work, but prost still
constructs the entire block before the application can compact it.  There is
no true field-by-field stream exposed by the current tonic/prost client.

The practical safe design is therefore:

1. strict gRPC max-message ceiling;
2. immediate Stage-1 compact extraction with zero Pool RPC and no PumpSwap
   event parsing;
3. durable compact proof and relevant-set manifest;
4. independently received filtered full-meta transaction records;
5. bounded-concurrency Stage-2 decoder/Pool resolver;
6. exact set equality before slot release.

An oversized block is an explicit `BLOCK_PROOF_OVERSIZE` coverage blackout,
not a reason to flush by timeout or accept a partial slot.

### RabbitStream and ShredStream

Both are useful as optional latency/receipt witnesses.  Neither provides the
execution metadata required by the frozen candidate state/F01-F13 semantics.
They must never fill a missing Stage-2 event or establish universe completeness.

## Recommended Next Step

**Do not start another canary yet.** First obtain a small, non-scientific
provider conformance fixture/confirmation answering one binary question:

> For a Shyft `blocks` subscription filtered on Pump/PumpSwap with
> `includeTransactions=true`, does each emitted `SubscribeUpdateBlock` contain
> the complete transaction inventory and canonical indexes for that whole slot,
> rather than only matching transactions?

If yes, implement the bounded Stage-1 adapter with an intentionally low
message ceiling and test it against that fixture.  If no or unavailable, the
scientifically valid fallback is to remain fail-closed until an authoritative
compact inventory source is available.  RabbitStream, ShredStream and Shyft
WebSockets do not close the gap.

