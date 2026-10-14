# Post-Gap Candidate-Universe Recovery

## Rule

No exact replay means no continuous candidate universe across the missing interval. The prior epoch is sealed and the new epoch enters `UNIVERSE_INCOMPLETE`. The new epoch may not admit scientific decisions merely by starting with an empty eligible universe or excluding pre-gap pools as unknown.

## Algorithm

1. Write a blackout interval `[last_proven_sequence, new_epoch_first_sequence)` to the immutable coverage ledger.
2. Start a new source epoch and validate contiguous canonical records from its boundary.
3. Bootstrap only from canonical post-boundary semantic events and authoritative retained evidence. A Pool-account read supplies immutable identity only; it may not supply post-gap reserves, flow, first-public-seen time, or candidate eligibility.
4. Enumerate every pool potentially eligible under the frozen rules at the gap boundary from retained authoritative pre-gap candidate-state/checkpoint evidence. For each such pool, assign exactly one status: `RESTORED_COMPLETE`, `PROVABLY_INELIGIBLE`, or `UNRESOLVED`.
5. `RESTORED_COMPLETE` requires retained authoritative evidence sufficient to reconstruct candidate eligibility and every required feature/state through the decision boundary. `PROVABLY_INELIGIBLE` requires frozen-rule evidence that the pool is no longer eligible, without using future outcomes. `UNRESOLVED` keeps the entire universe incomplete.
6. A newly observed post-boundary pool becomes a candidate only after authoritative identity, positive supported state, resolved causal ordering, and all rolling/prior-state requirements are populated from post-boundary records. This does not cure an unresolved older pool.
7. Require clean 5/20/60-second rolling history for all rolling fields. Require complete lifetime provenance for pool-age/no-prior-buy fields. Mark each field validity independently.
8. The universe becomes complete only if one of the following is sealed: (a) exact replay proves continuity across the gap; (b) every potentially eligible pre-gap pool is `RESTORED_COMPLETE` or `PROVABLY_INELIGIBLE`; or (c) a retained authoritative candidate-state proof shows all pools whose eligibility/state depends on the missing interval are `PROVABLY_INELIGIBLE`, leaving a fully post-boundary universe.
9. A wallet action is admissible only when the full-universe proof, chosen pool, and every candidate included in its universe have valid required fields, and the coverage ledger shows no blackout/recovery/warm-up interval at that exact decision boundary.

## Completeness certificate

For each accepted decision, seal the new epoch id, bootstrap start sequence, potential-pre-gap-pool inventory hash, per-pool recovery-status hash, full-universe proof type/hash, eligible-pool registry hash, per-feature validity state, and source segment hashes. There is no mechanism to reintroduce or suppress pre-gap pools by convenience or arrival-time inference.
