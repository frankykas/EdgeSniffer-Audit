# Design Evidence Index

Generated: 2026-10-14 architecture-provenance repair.

This index excludes its own full-file hash to avoid self-reference. `ARCHITECTURE_MANIFEST.sha256` covers every current bundle file except the manifest itself, including this index.

## Current architecture file hashes
- `CANDIDATE_UNIVERSE_RECOVERY.md`: `dc7aa23c759f31234a065961111e97bd34177f12675b131d3cd3aaa03cc7328c`
- `CANONICAL_EVENT_SCHEMA.md`: `5bca5f7109976c91770b3e2d3784722d761d1aab07702b2b0e6fc9afe00cde3b`
- `CHECKPOINT_SCHEMA.md`: `d07269163ed54c26c85060b14cf76add271e7905fd279027249fcebf3311a7bd`
- `COMPONENT_REUSE_MATRIX.md`: `9971480b130a8fdb7b541619dfd2d14c8466422e053c13e4b95f51cf7a52552d`
- `CONTINUITY_AND_RECOVERY.md`: `8a39d9a936ca0108541881af1041a919f0ca6f03db2a15f93191e5bb18bdb654`
- `DECISION_EVIDENCE_SCHEMA.md`: `e0f6f7f175082b9f0f5e9e33384a651c2bec1b385f037d9e47ef21f29cd734f7`
- `EXECUTION_FAST_PATH.md`: `33e78c9a18fd208c342060f54f6f0c7c5d5c584b1823cfd71bbcd637bc7f8b18`
- `FAILURE_TEST_MATRIX.md`: `35516359e663bda410f3456c56aa6d78b814326246b94b364181fe01234b8be8`
- `IMPLEMENTATION_PLAN.md`: `e05d5ab69e8287a7b2ea5769bcf78b2032ef8bede3d13d51b46623a471ebc23f`
- `MIGRATION_ROLLBACK_PLAN.md`: `762aaedbc6abf719c241a1c8babcd6656be07cad425c19705eccd3555c554555`
- `OLD_NEW_EQUIVALENCE_PLAN.md`: `5a156f958fd01e68f71d7c4aae1863f9c2ce6ca0e49fc8babe63134158325531`
- `SCIENTIFIC_COVERAGE_LEDGER.md`: `d14e5fdddf1cb1bd6834d62bcd5afd7a2c4e34528c004fd5717c8283af70c648`
- `SOURCE_ROLE_MATRIX.md`: `2037d249fd164827e3ed0251d482c28bcfcacc20bdf4c9dbac1947b9a8d8fd5d`
- `STATE_SCHEMA.md`: `1f65308d8af92321906b4083e3d535f2be725e256cd121dd71067eeb537d2a7a`
- `STORAGE_BENCHMARK_PLAN.md`: `3d93eee97144f1c483a3b1bf3dc3dec9bf4d052ca4ecec3f1f9268c875a3c10f`
- `STORAGE_POLICY.md`: `7bf8ef00d4feb89c7394e0f0ff74491aebea7dc8becefc4b8d46a2913a6ddf75`
- `TARGET_ARCHITECTURE.md`: `ce081bdfc86bb52a01024ce5b68fc2001a2dc434b52fd42552dd4db0df4ef603`

## Revision notes
- Strict causal ordering, complete-universe recovery, scientific coverage ledger, raw teacher-transaction retention, and RFC 8785/JCS hashing are design-only requirements.
- No production source, historical evidence, model, deployment, or service state is represented by these file hashes.
