# EdgeSniffer Canonical State Architecture v1

## Decision

Build one **canonical semantic journal** from a single controlled Shyft gRPC full-meta receiver. The canonical state engine, wallet-action classifier, candidate-universe builder, finalizer and research snapshot writer consume that journal only. There is no direct external subscription inside a scientific consumer.

Canonical sequence is a durable ingestion sequence, but **not by itself a scientific causal-order claim**. Scientific decisions use the Solana causal-ordering contract: slot, resolved transaction index, resolved instruction path, and deterministic semantic ordinal. A same-slot relationship that cannot be resolved from authoritative transaction metadata is `ORDER_AMBIGUOUS` and is excluded rather than guessed from gRPC arrival time.

This is not a claim that gRPC is the lowest-latency feed. It is the currently proven kEFi source of complete transaction metadata, PumpSwap decode inputs, `fromSlot` replay integration, and confirmed full-meta semantics. Its known silent-lag risk is addressed by durable source epochs, freshness detection, explicit replay verification and fail-closed writer behavior—not by pretending another subscription shares its continuity.

```text
             ShredStream ----------------> provisional execution observation
                 |                                  | (never research evidence)
                 |                                  v
Shyft gRPC --> canonical receiver --> semantic journal --> canonical state engine
   full meta       one sequence/epoch       sealed segments       |                                                        wallet index | candidate universe
                                                              \     | /
                                                               sealed decision evidence
Shyft RabbitStream / WebSocket --> cross-check telemetry / bootstrap hints only
```

Fast and canonical paths may observe the same chain, but only canonical journal order creates science. A fast-path transaction is reconciled by canonical identity or marked provisional/discarded; it may never create an independent research record.

Every future epoch additionally writes to an immutable scientific coverage ledger. It records valid time, blackout/recovery/warm-up intervals, accepted decisions, excluded decisions, and explicit exclusion reasons. This makes observability bias measurable rather than silently hidden.

After an unrecoverable gap, admission stays closed as `UNIVERSE_INCOMPLETE`. Marking pre-gap pools unknown and excluding them is never sufficient: an accepted decision requires proof that the **complete frozen candidate universe** is known at its boundary.

## Non-goals

- No implementation or deployment is authorized by this design.
- No V6/V7/V8 evidence is reclassified.
- No new holdout, model change, or archive/release operation is implied.
- No external feed is assumed equivalent without a source-specific validation record.
