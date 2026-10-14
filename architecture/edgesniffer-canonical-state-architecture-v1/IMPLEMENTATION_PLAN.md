# Phased Implementation Plan (No Deployment Authorized)

1. **Offline schema/writer:** add canonical semantic record, strict causal-order resolver, journal segments, hash-chain manifest, recovery scanner and immutable failure receipt.
2. **State migration:** extract existing observer state into a canonical-engine crate; preserve decoder and Pool resolver semantics.
3. **Wallet/candidate binding:** make wallet index and candidate universe consume canonical sequence only; remove observer direct gRPC ownership.
4. **Checkpointing and coverage:** atomic checkpoint/restore with per-feature validity, deterministic state serialization, candidate-universe recovery and immutable coverage ledger.
5. **Historical equivalence:** execute the full old/new plan twice and after restart; resolve every difference.
6. **Storage benchmark:** measure compact journal and decision packages on calm/burst retained data; set actual quotas.
7. **Shadow live:** subscribe once through the new receiver, run new state engine without scientific admission, compare state hashes with the legacy observer where their intervals overlap.
8. **Failure injection:** validate lag, provider disagreement, restart, corruption, disk pressure and capacity behavior.
9. **Controlled cutover:** disable the legacy observer only after shadow acceptance; run new source/state pipeline without holdout until stability criteria are met.
10. **Authorization gate:** only then can a separately reviewed future protocol consider a fresh boundary.

Each stage emits a receipt with code/config hashes, tests, measured resource use and rollback point. A stage does not imply authorization for the next.
