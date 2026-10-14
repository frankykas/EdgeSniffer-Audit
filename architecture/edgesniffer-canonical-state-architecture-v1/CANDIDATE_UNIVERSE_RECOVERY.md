# Post-Gap Candidate-Universe Recovery

## Rule

No exact replay means no continuous candidate universe across the missing interval. The prior epoch is sealed. The new epoch starts with an empty **scientifically eligible** universe even if a local cache/checkpoint contains old pools.

## Algorithm

1. Write a blackout interval `[last_proven_sequence, new_epoch_first_sequence)` to the immutable coverage ledger.
2. Start a new source epoch and validate contiguous canonical records from its boundary.
3. Bootstrap only from canonical post-boundary semantic events and authoritative Pool-account identity reads. A Pool-account read supplies immutable identity only; it may not supply post-gap reserves, flow, first-public-seen time, or candidate eligibility.
4. Treat every pre-gap pool as `EXISTING_POOL_HISTORY_UNKNOWN`. It may remain operationally tracked, but is scientifically ineligible until a frozen rule establishes post-boundary public state. For the current F01-F13 contract, it is excluded because pool age/no-prior-buy and prior-state provenance cannot be reconstructed without exact replay.
5. A newly observed post-boundary pool becomes a candidate only after authoritative identity, positive supported state, resolved causal ordering, and all rolling/prior-state requirements are populated from post-boundary records.
6. Require clean 5/20/60-second rolling history for all rolling fields. Require post-boundary first-seen and prior-buy provenance for lifetime fields. Mark each field validity independently.
7. A wallet action is admissible only when its chosen pool and every candidate included in its universe have valid required fields, and the coverage ledger shows no blackout/recovery/warm-up interval at that exact decision boundary.

## Completeness certificate

For each accepted decision, seal the new epoch id, bootstrap start sequence, eligible-pool registry hash, excluded-existing-pool count and reason, per-feature validity state, and source segment hashes. There is no mechanism to reintroduce pre-gap pools by convenience or arrival-time inference.
