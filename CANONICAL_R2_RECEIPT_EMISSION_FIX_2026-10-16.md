# Canonical State V1 R2 Receipt-Emission Fix

Base audit commit: `362d53c84d801b83ecf9ac847d6a2709ea49db04`.

## Scope and result

This is a review-only implementation correction. No binary was installed, no
live RPC request was made, and no R3 diagnostic was run.

The R2 failure was reproduced from code inspection: finalized reconciliation
uses program/account relevance, while `identity_receipts` previously emitted a
receipt for every successful finalized transaction with any account key. This
made a reconciled relevant set of 90 incompatible with 281 emitted receipt
records.

The correction introduces two shared methods on
`FinalizedTransactionInput` in `filtered_identity_manifest_v1`:

- `is_relevant_to(canonical_program_ids)`
- `require_supported_version()`

`reconcile_finalized_block` and the real producer's `identity_receipts` now
call those same methods. The receipt writer cannot independently reinterpret
relevance or transaction-version support.

## Forward-cohort fail-closed correction

Review found a second, real consumer defect. `CreationReceipt::validate()`
correctly permits `CREATION_QUARANTINED` as an audit record, but
`admit_creation` did not explicitly reject that disposition before producing a
forward-cohort pool admission. It now rejects every disposition other than
`CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED` with
`FORWARD_CREATION_QUARANTINED`.

This does not open, release, or alter any scientific cohort.

## Verification

Existing installed Rust toolchain used: `/root/.cargo/bin` (`cargo 1.99.0`,
`rustc 1.99.0`). No toolchain or package installation occurred.

| Command | Result |
| --- | --- |
| `cargo test --release --lib -- --nocapture` in `rust-exact-ledger` | 46 passed, 0 failed |
| `cargo test --release --lib forward_new_pool_cohort_v1 -- --nocapture` | 1 passed, 0 failed |
| `cargo test --release --bin kefi_finalized_rpc_reconciliation_canary_v1 -- --nocapture` | 17 passed, 0 failed |
| `cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1` | passed |

The repository-wide ledger test invocation remains blocked by unrelated legacy
V3 executables that do not compile against their current `EvidenceMetadata`
definition. The modified ledger library itself fully compiled and passed all
46 library tests; the real canary binary compiled and passed its 17 tests.

Focused new regression coverage proves:

- a mixed finalized block emits an identity receipt only for the successful
  Pump/PumpSwap-relevant transaction;
- unrelated successful and failed relevant transactions cannot enter the
  identity-receipt bundle;
- receipt `(signature, transaction_index)` identities exactly equal the
  reconciler's shared relevance set;
- the mixed bundle validates without `SLOT_IDENTITY_RECEIPT_CONFLICT`;
- unsupported version input is rejected by both reconciliation and receipt
  emission under the same shared supported-version rule;
- `CREATION_QUARANTINED` cannot become a forward-cohort pool admission;
- all R2 source-completeness and five-slot reservation tests remain passing.

## New review candidate

Release binary SHA-256:

```text
005351595f48d9f38633191a86ae8add45f835c42805ce361bfb4688ea8e297c
```

The installed canary remains R2:

```text
b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54
```

## Proposed R3 installation and rollback

These commands are review material only. They were not executed.

```bash
set -euo pipefail
src=/root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
new_sha=005351595f48d9f38633191a86ae8add45f835c42805ce361bfb4688ea8e297c
old_sha=b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54
backup="${dst}.pre-r3-receipt-emission-$(date -u +%Y%m%dT%H%M%SZ)"

test "$(sha256sum "$src" | awk '{print $1}')" = "$new_sha"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
test ! -e "$backup"
cp --preserve=mode,ownership,timestamps "$dst" "$backup"
install -m 0755 "$src" "${dst}.new"
test "$(sha256sum "${dst}.new" | awk '{print $1}')" = "$new_sha"
mv -f "${dst}.new" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$new_sha"
printf 'ROLLBACK_BACKUP=%s\n' "$backup"
```

Rollback, separately approval-gated:

```bash
set -euo pipefail
backup=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r3-receipt-emission-REPLACE_WITH_RECORDED_TIMESTAMP
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
old_sha=b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54

test "$(sha256sum "$backup" | awk '{print $1}')" = "$old_sha"
install -m 0755 "$backup" "${dst}.rollback"
mv -f "${dst}.rollback" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
```

## Requirement checklist

| Requirement | Status | Evidence |
| --- | --- | --- |
| Share reconciliation/receipt relevance and version rules | VERIFIED | Shared `FinalizedTransactionInput` methods |
| Mixed block regression coverage | VERIFIED | New actual-canary regression; 17/17 passed |
| Exact receipt/reconciler signature-index equality | VERIFIED | New regression assertion |
| R2 source-completeness behavior unchanged | VERIFIED | Existing five-slot tests remain green |
| Quarantined creation blocked by forward consumer | VERIFIED | New adversarial ledger regression |
| New release candidate built | VERIFIED | SHA above |
| Install/live RPC/R3/services/V8/dust/trading/evidence changes | NOT PERFORMED | Outside authorized scope |

The implementation is ready for independent review and a separately authorized
R3 installation/run. R1 and R2 evidence remains untouched.
