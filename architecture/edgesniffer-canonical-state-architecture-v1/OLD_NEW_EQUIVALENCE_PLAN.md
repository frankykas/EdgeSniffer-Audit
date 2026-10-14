# Old-vs-New Equivalence Plan

Run offline against identical sealed historical input. For every retained supported transaction, compare:

1. decoded PumpSwap semantic events and deterministic event identity;
2. authoritative pool identity/cache records;
3. direct wallet action classification and wallet-ledger tail;
4. active eligible pool universe and every explicit exclusion;
5. F01–F13 values, validity flags and feature-schema order;
6. pre-decision candidate snapshots and evidence package hashes where serialization is deterministic.
7. causal-order status and the exact exclusion set for same-slot ambiguity;
8. coverage-ledger intervals and accepted/excluded decision identities.

Allowed differences: transport receipt timestamps, process ids, temporary file names and intentionally new canonical sequence field representation. No difference is allowed in semantic identity, resolved Solana causal ordering, pool/mint identity, eligibility, frozen feature values, candidate membership, selected decision identity, coverage disposition, or fail-closed disposition. Newly excluded records are permitted only where the old input cannot prove same-slot causal ordering; each must have an explicit `ORDER_AMBIGUOUS` counterexample fixture.

Run twice plus restart-from-checkpoint replay. Any difference produces a compact counterexample fixture and blocks deployment. The large Pump evidence ledger is the primary input; no fresh cohort or outcome inspection is needed.
