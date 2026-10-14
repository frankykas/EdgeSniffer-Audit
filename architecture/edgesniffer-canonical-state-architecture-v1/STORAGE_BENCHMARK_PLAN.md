# Storage Benchmark Plan

The prior 6 GiB ring breach is excluded as a storage-rate measurement. It measured RAM ring occupancy during dense retention, not sustained disk ingress.

## Offline benchmark using retained evidence

1. Select sealed historical Pump/PumpSwap partitions from the large evidence ledger without changing them.
2. Run the proposed semantic encoder twice; prove identical event identities, segment hashes and output bytes.
3. Record input frames, decoded semantic events, zstd bytes, bytes/event, events/hour, compressed GiB/million events, checkpoint size, decision package size, CPU, peak RSS, write bandwidth and replay lag.
4. Use at least one calm and one observed-burst window. Report p50/p95/max separately; only derive 24-hour projections from sustained observed intervals.

## Current measured facts

The audited V8 root was roughly 202 MB and did not contain an all-event spool. V8 health had cumulative frame counters but no retained per-minute byte series. Therefore journal GiB/hour, GiB/day, bytes per semantic event and peak semantic write bandwidth are **not yet measured**. The design blocks capacity sizing and deployment until this benchmark exists.
