# Canonical State V1 - Remaining Instruction-Path Decoder Work

Filtered-identity reconciliation answers only this question:

`Did the provider deliver every Pump/PumpSwap-relevant transaction in a finalized block?`

It does **not** answer:

`Which exact outer instruction, inner instruction path, and semantic event produced a state mutation?`

## Required implementation changes

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

## Explicit non-claims

- A Yellowstone arrival index is not canonical transaction order.
- A block signature position does not by itself identify an instruction path.
- Pool identity resolution must not reorder events or fill an unknown path.
- Passing the filtered-identity reconciler does not authorize semantic state,
  feature calculation, scientific admission, or a holdout.

## Gate

Semantic ordering remains **BLOCKED** until every emitted Pump/PumpSwap event
has a verified instruction path or the transaction is excluded with an
immutable failure receipt.
