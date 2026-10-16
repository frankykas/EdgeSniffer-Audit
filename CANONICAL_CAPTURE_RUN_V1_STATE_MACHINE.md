# Canonical Capture Run V1 State Machine

## States

```text
PRECHECK
  -> CAPTURING_SOURCE
  -> RANGE_SELECTED
  -> FINALIZED_INVENTORY_FETCHED
  -> AWAITING_SOURCE_MATCH
  -> RECEIPTS_STAGED
  -> RECEIPTS_COMMITTED
  -> VERIFIED_DIAGNOSTIC

any nonterminal state -> INCONCLUSIVE | QUARANTINED | VOIDED
```

## Invariants

1. Source finality is a durable source observation, not finalized-RPC proof.
2. The contiguous 3–5 slot range is selected exclusively from durable source
   finality records. No dispatcher reconciliation is required before selection.
3. Finalized `getBlocks` then one `getBlock` per produced slot create the
   immutable expected relevant identity set. Each raw provider response is
   reserved, written, hashed, then parsed.
4. `AWAITING_SOURCE_MATCH` may wait for source identities but must not consume
   another finalized-RPC request merely because an expected identity is late.
5. A source gap, source-instance change, epoch change, corrupt checkpoint,
   invalid receipt bundle, resource breach, or missed source deadline cannot
   produce a committed range.
6. A restart never bridges a nonterminal root. Startup atomically records
   `VOIDED` and requires a fresh output root.
7. A committed slot bundle validates the same policy hash and exact expected /
   emitted `(slot, signature, transactionIndex)` set digest before a cohort
   consumer can read it.
8. Canonical semantic records may be materialized after a receipt bundle
   commits, but carry `UNIVERSE_INCOMPLETE`; no scientific decision is released
   by this diagnostic path.

## Checkpoint Fields

- source instance and continuity epoch
- policy version and SHA-256
- source finality records
- filtered-identity manifest tail and cursor
- selected range and finalized inventory SHA-256
- expected identity set
- raw RPC evidence descriptors
- RPC/disk/RSS/queue accounting
- receipt staging / commit state
- terminal reason and self-hash

The checkpoint uses canonical JCS bytes and atomic replace. Its hash excludes
only its self-hash field.
