# Canonical Producer Canary V1 Report

## Scope

This was an isolated, capture-only producer exercise.  It did not feed a
scientific observer, a holdout, a research admission path, PRE_H3, 2CQ, or
execution.  The active V8 capture service was neither restarted nor replaced.

## Implemented safeguards

- `universe_complete_at(epoch, sequence)` is historical: it evaluates only
  immutable coverage/proof records at or before the queried sequence.  A later
  gap, recovery interval, or proof cannot rewrite a formerly valid boundary.
- Regression: complete -> accepted boundary -> later incomplete -> re-query
  earlier boundary remains complete.
- Canonical records use JCS bytes and self-hash exclusion; journal restart and
  open-segment recovery are covered by regression tests.
- The isolated service has its own systemd unit, binary, mount assertion and
  root on `/mnt/edgesniffer-kefi-hot`, a 240 GiB protected-free-space reserve,
  a 4 GiB temporary raw-fixture quota, a 6 GiB total quota, a 45-minute wall
  clock cap, 1 GiB memory cap, and idle I/O priority.
- The raw validation fixture receives an immutable TTL record.  It is a
  bounded migration artifact, not a raw firehose.

## Causal-ordering result

The first canary used Yellowstone `SLOT_COMPLETED` as a closure signal.  It
received a slot-complete update before a related filtered transaction.  The
canary sealed itself with `LATE_TRANSACTION_AFTER_SLOT_COMPLETION`; it did not
accept or emit a scientific decision.  This proves transport arrival order is
not usable as a Solana causal-order closure rule for this provider stream.

The next implementation subscribed to Yellowstone block messages carrying
transaction indexes.  It established that a block update can contain the
complete transaction inventory, but the provider emits a memory-heavy complete
block payload.  Resolving every newly encountered pool in the first block
pushed the isolated process toward its 1 GiB cap before useful progress could
be published.  It was stopped manually, fail-closed, before a semantic stream
or any scientific state was accepted.

Both canary roots are preserved audit evidence only:

- `canonical-state-v1-canary-20261014T230000Z`
- `canonical-state-v1-canary-20261014T233000Z`
- `canonical-state-v1-canary-20261014T233200Z`

No root is a scientific cohort and none may be reused for admission.

## Current hard blocker

The current Shyft subscription surface has not yet demonstrated a bounded,
authoritative way to obtain *both*:

1. filtered full-meta Pump/PumpSwap transactions needed by the decoder; and
2. a complete, compact per-slot transaction-order closure proof.

`SLOT_COMPLETED` is disproven as proof #2.  Full `SubscribeUpdateBlock`
provides transaction indexes but is too large to decode/pool-resolve inside the
bounded producer as written.  The implementation intentionally labels events
without authoritative instruction paths `ORDER_AMBIGUOUS`; those records cannot
mutate scientific state or establish a complete candidate universe.

Therefore this canary has **not** established a complete validation interval.
Legacy/new semantic equivalence, feature parity, scientific bootstrap and the
integrated capture fault matrix remain blocked.  No result may be inferred from
the capture attempts.

## Verified tests

```
cargo test canonical_state_v1 --lib        8 passed
cargo test --bin kefi_canonical_canary_v1  1 passed
```

The canonical suite covers reverse arrival order, JCS self-hash vectors,
historical coverage stability, coverage/proof non-overlap, journal
restart/open-segment recovery, and corrupt journal/checkpoint rejection.
The producer test proves the subscription requests complete blocks rather than
using slot-arrival closure.

## Required next engineering gate

Before another live canary, implement a bounded two-stage block verifier:

1. persist a quota-capped complete-block ordering proof without RPC pool
   resolution in the receive/closure path;
2. separately extract only filtered Pump/PumpSwap transaction records in
   transaction-index order and resolve pool identity with bounded concurrency;
3. prove that every admitted filtered transaction appears in its retained block
   proof and that no omitted matching transaction exists;
4. retain `ORDER_AMBIGUOUS` and prevent scientific state mutation whenever
   instruction-path order is unavailable.

This is a producer-mechanics blocker, not a reason to weaken scientific
ordering or declare a post-start universe complete.

