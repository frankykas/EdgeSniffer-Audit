# Checkpoint Schema

Checkpoint frequency is determined by measured semantic-event rate and a recovery budget: start with 5-minute checkpoints or 64 MiB of changed canonical state, whichever occurs first; revise only after the storage benchmark shows actual replay cost. This is an initial engineering cap, not a scientific parameter.

```text
checkpoint_id, schema_version, created_at
source_instance_id, epoch_id, canonical_sequence, slot
sealed_segment_manifest_hash, journal_tail_hash
pool_identity_registry_hash
active_pool_state: first_seen, reserves, fee/layout, prior-buy, rolling deque
candidate_state: eligibility and explicit exclusions
wallet_ledger_sequence, wallet_ledger_tail_hash
feature_validity: per-pool horizons/lifetime-state validity
engine_binary_hash, parser_schema_hash, checkpoint_payload_hash
```

Write compressed state to a temporary file, `fsync`, write checksum/metadata, `fsync`, atomically rename, then append a checkpoint receipt to the journal manifest. A restart may restore only when source instance/epoch, journal tail and checkpoint hash match. Otherwise it seals the epoch and starts a new one.

Immediate validity after restoration is allowed only for state whose complete inputs are in the checkpoint: F01–F03, F05–F13 and candidate eligibility may resume if exact rolling deques/lifetime provenance are restored. If any required state is absent/corrupt, mark affected pools invalid; require 5/20/60-second clean warm-up for rolling fields and exclude unprovable lifetime fields until rebuilt.
