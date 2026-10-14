# Old-vs-New Equivalence Plan

Run offline against identical sealed historical input. For every retained supported transaction, compare:

1. decoded PumpSwap semantic events and deterministic event identity;
2. authoritative pool identity/cache records;
3. direct wallet action classification and wallet-ledger tail;
4. active eligible pool universe and every explicit exclusion;
5. F01–F13 values, validity flags and feature-schema order;
6. pre-decision candidate snapshots and evidence package hashes where serialization is deterministic.

Allowed differences: transport receipt timestamps, process ids, temporary file names and intentionally new canonical sequence field representation. No difference is allowed in semantic identity, ordering after deterministic canonical sort, pool/mint identity, eligibility, frozen feature values, candidate membership, selected decision identity, or fail-closed disposition.

Run twice plus restart-from-checkpoint replay. Any difference produces a compact counterexample fixture and blocks deployment. The large Pump evidence ledger is the primary input; no fresh cohort or outcome inspection is needed.
