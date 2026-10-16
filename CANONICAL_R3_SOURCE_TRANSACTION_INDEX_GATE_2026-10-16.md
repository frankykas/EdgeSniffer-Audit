# Canonical R3 Source Transaction-Index Gate

Base review: `6c4f5af400710153b87b1668542105eb758631a5`.

## Result

The explicit Yellowstone source transaction-index consistency gate has been
restored. Finalized RPC remains the only authority for canonical ordering:

```text
slot -> finalized transaction index -> instruction path -> semantic ordinal
```

Yellowstone `SubscribeUpdateTransactionInfo.index` is retained as an
independent consistency assertion. A source/finalized mismatch rejects the
slot with `FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH`; it never changes the
canonical order. Receive sequence and receive timestamp remain the delivery
provenance, so reverse stream delivery remains valid.

## Narrow implementation change

- Renamed the in-memory Rust field to `source_transaction_index`.
- Kept its serialized key as `observedArrivalIndex` with
  `#[serde(rename = "observedArrivalIndex", alias = "sourceTransactionIndex")]`.
  Existing immutable manifests therefore deserialize unchanged and preserve
  their previous canonical JSON/hash input for the same records.
- The finalized reconciler now first verifies exact relevant `(slot,
  signature)` membership, then requires every matching Yellowstone record to
  contain a parseable source index equal to the finalized transaction index.
- Existing source records missing the index fail closed with
  `FILTERED_SOURCE_TRANSACTION_INDEX_MISSING`; malformed values fail with
  `FILTERED_SOURCE_TRANSACTION_INDEX_INVALID`.
- Exact identity-set mismatches still retain their existing
  missing/extra/conflict dispositions before the index cross-check runs.

No R2/R3 relevance predicate, receipt-emission behavior, source-completeness
gate, or quarantined-creation protection was changed.

## Focused verification

| Requirement | Result | Evidence |
| --- | --- | --- |
| Reverse delivery is accepted | VERIFIED | `complete_interval_reconciles_when_arrival_is_reverse_of_block_order`: source index 2 is received before index 0; reconciliation matches. |
| Equal source/finalized indexes pass | VERIFIED | Same reverse-delivery regression uses exact source/finalized index equality. |
| Index mismatch fails closed | VERIFIED | `source_transaction_index_mismatch_fails_closed` returns `REJECTED` with `FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH`. |
| Missing index fails closed | VERIFIED | `missing_source_transaction_index_fails_closed_after_identity_membership_checks` returns `REJECTED` with `FILTERED_SOURCE_TRANSACTION_INDEX_MISSING`. |
| Existing relevance/receipt correction remains covered | VERIFIED | Canary integration suite includes `identity_receipts_share_exact_reconciliation_relevance_and_version_rules`. |
| Quarantined creation remains ineligible | VERIFIED | `forward_new_pool_cohort_v1::tests::quarantined_creation_receipt_cannot_be_admitted`. |

Executed with the installed Rust 1.99.0 toolchain:

```text
cargo test --release --lib filtered_identity_manifest_v1 -- --nocapture
14 passed, 0 failed

cargo test --release --lib forward_new_pool_cohort_v1 -- --nocapture
1 passed, 0 failed

cargo test --release --lib
49 passed, 0 failed

cargo test --release --bin kefi_finalized_rpc_reconciliation_canary_v1 -- --nocapture
17 passed, 0 failed

cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1
passed
```

The only build diagnostic was the existing unused `first_seen_ms` field in the
dispatcher; it does not affect this gate.

## R3 Release Candidate

New uninstalled binary SHA-256:

```text
96887daf4a76dd7d4cc693076812be930135966c4515d5ef585784ac6500c799
```

The installed binary was not changed and remains:

```text
b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54
```

## Proposed R3 Install and Rollback (Not Executed)

```bash
set -euo pipefail
src=/root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
candidate_sha=96887daf4a76dd7d4cc693076812be930135966c4515d5ef585784ac6500c799
backup=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r3-source-index

test "$(sha256sum "$src" | awk '{print $1}')" = "$candidate_sha"
test "$(sha256sum "$dst" | awk '{print $1}')" = "b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54"
cp --preserve=mode,timestamps "$dst" "$backup"
install -m 0755 "$src" "$dst.new"
test "$(sha256sum "$dst.new" | awk '{print $1}')" = "$candidate_sha"
mv -f "$dst.new" "$dst"
```

Rollback, if an approved R3 has not modified the previous binary:

```bash
set -euo pipefail
mv -f /usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r3-source-index \
  /usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
```

No installation, R3 execution, live RPC, service action, V8 action,
dust-service action, transaction submission, trading, or prior-evidence
modification occurred in this change.
