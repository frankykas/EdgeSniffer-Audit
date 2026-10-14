# Canonical Semantic Journal

## Identity and order

One canonical journal record is emitted only by the canonical gRPC receiver. A record has monotonically increasing `canonical_sequence`; sequence gaps are illegal. `canonical_sequence` records durable ingestion order only. The semantic deduplication identity is:

`chain_id | slot | transaction_signature | transaction_index_or_unknown | outer_instruction_index | inner_instruction_path | program_id | pool_pubkey | semantic_kind | semantic_ordinal`

For transaction records without stable instruction index, retain `event_payload_hash` and a deterministic ordinal after sorting canonical decoded events by instruction path, program event order, pool, side and payload hash. Same identity with different payload is a contradiction and seals the epoch.

## Strict Solana causal-ordering contract

Scientific pre-action ordering uses `(slot, transaction_index, outer_instruction_index, inner_instruction_path, semantic_ordinal)`, derived from authoritative transaction metadata. Arrival time is metadata only and never breaks a causal tie. A record lacking a required transaction index or instruction path is retained as a diagnostic semantic record but cannot be used to establish a before/after relationship. If the selected-wallet action and any state-affecting event share a slot and their relative transaction/instruction order cannot be resolved, every affected candidate and decision is rejected with `ORDER_AMBIGUOUS`. No timestamp, provider order, lexical signature order, or sequence number may repair that ambiguity.

## Byte-level deterministic serialization

The hash input for every semantic record, receipt, checkpoint manifest and decision package is the UTF-8 byte sequence produced by **RFC 8785 JSON Canonicalization Scheme (JCS)** over the schema-versioned object. SHA-256 is taken over those uncompressed canonical bytes. Zstandard is a storage transport only: compression parameters, frame boundaries and compressed bytes never define semantic identity.

Conforming writers must use JSON strings for every Solana public key, signature, hash and identifier; JSON integers only for exact values within the RFC 8785 interoperable integer range; decimal strings for `u64`, lamports, raw token quantities, sequence numbers and nanoseconds outside that range; JSON booleans for flags; explicit JSON `null` for absent optional fields; no non-finite numbers; no omitted field in place of a defined nullable field; and no duplicate keys. The schema/version, parser version and field names are part of the canonical object. Arrays use frozen schema order; maps are canonicalized by JCS member ordering. A schema change requires a new schema identifier and cannot rewrite prior hashes.

## Record envelope (JCS bytes -> zstd framed segment)

```json
{
  "schema":"EDGESNIFFER_CANONICAL_SEMANTIC_EVENT_V1",
  "canonical_sequence": 0,
  "source_instance_id":"...", "epoch_id":0,
  "observed_at_unix_ns":0, "provider_received_at_unix_ns":0,
  "slot":0, "signature":"...", "transaction_index":null,
  "instruction_path":"outer/inner", "semantic_ordinal":0,
  "causal_order_status":"RESOLVED|ORDER_AMBIGUOUS|NOT_REQUIRED",
  "event_identity":"sha256...", "raw_frame_sha256":"optional",
  "kind":"PUMPSWAP_SWAP|POOL_DISCOVERED|WALLET_ACTION|...",
  "pool":"...", "base_mint":"...", "quote_mint":"...",
  "side":"BUY|SELL", "actor":"...",
  "pool_base_raw":0, "pool_quote_raw":0, "quote_delta_raw":0,
  "fee_tuple":[0,0,0,0,0], "layout":"...",
  "pool_identity_ref":"sha256...", "parser_version":"sha256...",
  "previous_record_sha256":"...", "record_sha256":"sha256(JCS UTF-8 bytes excluding record_sha256)"
}
```

The final schema has nullable fields only where a semantic type cannot own them. It never silently substitutes a guessed mint. Pool identity is a separate immutable cache record resolved from the canonical `event.pool`, owner and supported account layout.

## Segments and durability

Append records to fixed-size/time-bounded local segments on the kEFi hot volume. Each segment has a header (source instance, epoch, first sequence), record hash chain, compressed-payload SHA-256 for transport integrity, footer (last sequence, record count, canonical-uncompressed byte count, compressed byte count, segment SHA-256) and atomic publish. A journal manifest hash-chains the JCS-derived record/segment identities. `fdatasync` occurs at a bounded group-commit interval; publication is recoverable by decompressing a partial segment and verifying canonical sequence/hash continuity.

Full raw provider frames stay in bounded RAM and optional short-TTL diagnostics. `raw_frame_sha256` permits evidence binding without making raw payload a permanent dependency.
