# Immutable Scientific Coverage Ledger

Each source epoch has an append-only, hash-chained coverage ledger. It is separate from mutable health telemetry and is sealed alongside journal/checkpoint receipts.

```json
{
  "schema":"EDGESNIFFER_SCIENTIFIC_COVERAGE_LEDGER_V1",
  "ledger_sequence":0,
  "epoch_id":0,
  "kind":"VALID_INTERVAL|BLACKOUT|RECOVERY|WARMUP|ACCEPTED_DECISION|EXCLUDED_DECISION",
  "start_canonical_sequence":0,
  "end_canonical_sequence":0,
  "start_slot":0,
  "end_slot":0,
  "start_time_unix_ns":0,
  "end_time_unix_ns":0,
  "decision_id":null,
  "reason_code":"SOURCE_LAG|ORDER_AMBIGUOUS|FEATURE_WARMUP|EXISTING_POOL_HISTORY_UNKNOWN|...",
  "details_hash":"sha256...",
  "previous_record_sha256":"sha256...",
  "record_sha256":"sha256..."
}
```

Intervals cannot overlap ambiguously. A valid interval is closed before a blackout/recovery/warm-up interval opens. Accepted/excluded decision records refer to the interval and canonical sequence at which the decision was evaluated. A cohort report must calculate its exposure and missing-observation time exclusively from this ledger; it may not infer continuity from process uptime or health counters.

This ledger is a future-protocol component only. It does not alter, repair, or reuse V6 or historical cohorts.
