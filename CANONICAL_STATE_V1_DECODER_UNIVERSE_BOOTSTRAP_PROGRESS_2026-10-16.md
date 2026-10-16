# Canonical State V1: Decoder and Universe Bootstrap Progress

## Implemented

* Added `pool_universe_bootstrap_v1` to the exact-evidence ledger.
* The bootstrap is immutable and hash-bound.  It explicitly represents
  `Known(true)`, `Known(false)`, and `Unknown` lifetime `ever_observed_buy`
  state instead of coercing unknown history to false.
* `UNIVERSE_COMPLETE` is possible only with a `FULL_FINALIZED_HISTORY_REPLAY`
  proof covering the declared lower sequence boundary and known lifetime state
  for every pool in the snapshot.  Program-account snapshots and live filtered
  observation are intentionally rejected as insufficient.
* The path decoder's durable `event_id` mapping from the immediately prior
  change is retained; focused decoder tests still pass.

## Verified tests

* `cargo test --lib pool_universe_bootstrap_v1`: 2 passed.
* `cargo test --lib pump_instruction_path_decoder_v1`: 5 passed.

## Exact current limitation

The actual finalized-RPC producer has not yet been changed to construct
`FullTransactionInput` from the finalized `getBlock` JSON, invoke
`decode_instruction_paths`, and persist a per-slot path receipt.  It still
uses `decode_events` solely as a legacy event parser/count.  Therefore the
answer to “does the live producer now use the authoritative decoder?” is **no**.

There is also no available retained source that proves a contiguous complete
historical eligible-pool universe including lifetime buy state.  A current
PumpSwap `getProgramAccounts` scan can identify current Pool accounts but
cannot establish historical eligibility or `ever_observed_buy`; it can only
form a partial, explicitly incomplete bootstrap.  This new implementation
preserves that distinction and prevents a false `UNIVERSE_COMPLETE` receipt.

## Smallest remaining implementation step

Implement the finalized-RPC JSON adapter in the real producer, then persist
quarantined path receipts for every reconciled event.  In parallel the only
route to `UNIVERSE_COMPLETE` is an independently complete finalized historical
replay from a defined lower boundary (or a future capture with a frozen finite
eligibility horizon that demonstrably ages every pre-boundary unknown pool out).
Neither condition exists yet, so no scientific interval canary is authorized.
