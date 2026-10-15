# Canonical State V1 - Instruction-Path Decoder Status

Filtered-identity reconciliation answers only this question:

`Did the provider deliver every Pump/PumpSwap-relevant transaction in a finalized block?`

It does **not** answer:

`Which exact outer instruction, inner instruction path, and semantic event produced a state mutation?`

## Offline implementation completed

`sanitized-code/canonical-state-v1/pump_instruction_path_decoder_v1.rs`
now implements the following offline primitive:

1. Accept the complete transaction message and metadata together, not logs
   alone.
2. Build the full account-key space from static keys plus loaded writable and
   readonly addresses.
3. Preserve the outer compiled-instruction index. For every `innerInstructions`
   group, preserve the parent outer index and ordered inner position; use stack
   height when available only as additional provenance, not as a substitute for
   position.
4. Map Pump/PumpSwap invocation/log nesting to the outer/inner instruction
   path. A decoder must reject an ambiguous mapping rather than choosing the
   first matching log/event.
5. Emit each semantic event with the frozen order tuple:
   `slot -> finalized transaction index -> outer/inner instruction path -> semantic ordinal`.
6. Require all semantic events from a transaction to have strictly ordered,
   non-duplicated paths. Unsupported layouts, missing inner instruction data,
   loaded-address resolution failure, and conflicting log paths become
   `ORDER_AMBIGUOUS` / excluded evidence.
7. Test direct Pump, direct PumpSwap, router CPI, multiple PumpSwap invocations,
   multi-mint routed transactions, loaded-address transactions, failed
   transactions, and intentionally ambiguous logs.

It requires a unique message-plus-metadata invocation match. Logs only
corroborate the active program at the event's recorded log position; they are
never used to create a path. Ambiguous, malformed, failed, or incomplete
records are rejected before scientific-state release.

## Remaining integration work

1. Build an offline adapter from the retained Yellowstone full-meta protobuf
   transaction/message/meta types into the decoder input types.
2. Bind the existing PumpSwap event decoder's side/discriminator evidence to
   `SemanticEventInput` and preserve its raw event provenance.
3. Validate the adapter against retained full Pump/PumpSwap fixtures. The
   current retained fixture listed in the source tree is a 2CQ protobuf, not
   an eligible Pump/PumpSwap full-transaction fixture, so this item is
   currently blocked by fixture availability rather than inferred complete.
4. Only after that adapter and a finalized complete-interval proof exist may a
   caller mark an event `RESOLVED`; candidate-universe completeness remains a
   separate required proof.

## Explicit non-claims

- A Yellowstone arrival index is not canonical transaction order.
- A block signature position does not by itself identify an instruction path.
- Pool identity resolution must not reorder events or fill an unknown path.
- Passing the filtered-identity reconciler does not authorize semantic state,
  feature calculation, scientific admission, or a holdout.

## Gate

The decoder primitive is **ready for offline adapter integration**, but
semantic ordering remains **BLOCKED for a live canary** until every emitted
Pump/PumpSwap event has a verified instruction path against retained/full-meta
fixtures and a complete captured interval plus complete candidate-universe
proof exist.
