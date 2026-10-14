# Canonical Semantic Journal

## Identity and order

One canonical journal record is emitted only by the canonical gRPC receiver. A record has monotonically increasing `canonical_sequence`; sequence gaps are illegal. The semantic deduplication identity is:

`chain_id | slot | transaction_signature | transaction_index_or_unknown | outer_instruction_index | inner_instruction_path | program_id | pool_pubkey | semantic_kind | semantic_ordinal`

For transaction records without stable instruction index, retain `event_payload_hash` and a deterministic ordinal after sorting canonical decoded events by instruction path, program event order, pool, side and payload hash. Same identity with different payload is a contradiction and seals the epoch.

## Record envelope (canonical JSON -> zstd framed segment)

```json
{
  "schema":"EDGESNIFFER_CANONICAL_SEMANTIC_EVENT_V1",
  "canonical_sequence": 0,
  "source_instance_id":"...", "epoch_id":0,
  "observed_at_unix_ns":0, "provider_received_at_unix_ns":0,
  "slot":0, "signature":"...", "transaction_index":null,
  "instruction_path":"outer/inner", "semantic_ordinal":0,
  "event_identity":"sha256...", "raw_frame_sha256":"optional",
  "kind":"PUMPSWAP_SWAP|POOL_DISCOVERED|WALLET_ACTION|...",
  "pool":"...", "base_mint":"...", "quote_mint":"...",
  "side":"BUY|SELL", "actor":"...",
  "pool_base_raw":0, "pool_quote_raw":0, "quote_delta_raw":0,
  "fee_tuple":[0,0,0,0,0], "layout":"...",
  "pool_identity_ref":"sha256...", "parser_version":"sha256...",
  "previous_record_sha256":"...", "record_sha256":"..."
}
```

The final schema has nullable fields only where a semantic type cannot own them. It never silently substitutes a guessed mint. Pool identity is a separate immutable cache record resolved from the canonical `event.pool`, owner and supported account layout.

## Segments and durability

Append records to fixed-size/time-bounded local segments on the kEFi hot volume. Each segment has a header (source instance, epoch, first sequence), record hash chain, compressed payload SHA-256, footer (last sequence, record count, byte count, segment SHA-256) and atomic publish. A journal manifest hash-chains sealed segments. `fdatasync` occurs at a bounded group-commit interval; publication is recoverable by scanning a partial segment and verifying sequence/hash continuity.

Full raw provider frames stay in bounded RAM and optional short-TTL diagnostics. `raw_frame_sha256` permits evidence binding without making raw payload a permanent dependency.
