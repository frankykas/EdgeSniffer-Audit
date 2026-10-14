# Sealed Decision Evidence

One immutable package per accepted future decision contains only audit/reproduction necessities:

- decision id, chosen pool/mint, wallet transaction identity and canonical decision sequence;
- source instance, epoch, sequence interval, slots and journal-segment hashes;
- candidate identities, eligibility/exclusion reasons and candidate-state hash;
- exact frozen feature vector values and feature-schema/model/candidate-rule hashes;
- pool identity provenance and wallet-ledger before/after tail references;
- checkpoint reference/hash or semantic event slice hashes needed for reconstruction;
- completeness certificate: contiguous sequence proof, valid feature flags, universe validity, atomic writer success;
- optional raw frames only where specifically referenced for an anomaly or required receipt;
- package hash, previous package hash and writer binary hash.

The package must be independently reproducible from retained semantic segments/checkpoint references while those are in retention. On archive compaction, retain the package plus a compact per-decision semantic slice if global segments are eligible to expire.
